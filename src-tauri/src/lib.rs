mod account;
mod agent;
mod changes;
mod context;
mod events;
mod hook_server;
mod hooks;
mod layout;
mod notifications;
mod ring;
mod session;
mod shell;
mod store;
mod usage;
mod windows;
mod workspace;

use account::AccountRegistry;
use changes::{FileTracker, GitWorktreeInfo};
use events::{Context, Live};
use hook_server::{HookServer, SessionEvent};
use std::collections::HashMap;

use agent::Agent;
use layout::Node;
use session::{AttachInfo, Sessions};
use shell::ShellInfo;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use store::{Kind, State as StoredState};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};
use workspace::Workspace;

/// 세션별 훅 상태 (실행 번호·상태·컨텍스트).
#[derive(Default)]
pub struct HookStates {
    states: Mutex<HashMap<String, HookEntry>>,
}

struct HookEntry {
    run: u64,
    name: String,
    live: Option<Live>,
    context: Option<Context>,
}

/// Tauri 이벤트 `session-state` 페이로드.
#[derive(serde::Serialize, Clone)]
struct SessionStatePayload {
    id: String,
    run: u64,
    live: Option<LivePayload>,
    context: Option<ContextPayload>,
}

#[derive(serde::Serialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
enum LivePayload {
    Working,
    Waiting { request: String },
    Done,
}

#[derive(serde::Serialize, Clone)]
struct ContextPayload {
    used: u64,
    size: u64,
    estimated: bool,
}

impl From<Live> for LivePayload {
    fn from(l: Live) -> Self {
        match l {
            Live::Working => LivePayload::Working,
            Live::Waiting { request } => LivePayload::Waiting { request },
            Live::Done => LivePayload::Done,
        }
    }
}

impl From<Context> for ContextPayload {
    fn from(c: Context) -> Self {
        ContextPayload {
            used: c.used,
            size: c.size,
            estimated: c.estimated,
        }
    }
}

#[derive(serde::Serialize, Clone)]
struct ConflictDetected {
    session_id: String,
    files: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
struct SessionExit {
    id: String,
    run: u64,
    code: u32,
}

#[derive(serde::Serialize)]
struct SessionStarted {
    run: u64,
    os_build: u32,
}

#[derive(serde::Serialize)]
struct AppState {
    state: StoredState,
    warning: Option<String>,
}

#[derive(serde::Serialize)]
struct Detected {
    kind: Kind,
    name: String,
}

#[derive(serde::Serialize)]
struct Created {
    id: String,
    state: StoredState,
}

type Shared<'a> = State<'a, Mutex<Workspace>>;

fn lock<'a>(workspace: &'a Shared<'_>) -> Result<MutexGuard<'a, Workspace>, String> {
    workspace
        .lock()
        .map_err(|_| "저장 상태를 쓸 수 없어요. 앱을 다시 시작해 주세요".to_string())
}

// 새 명령을 추가하면 build.rs의 목록과 capabilities/default.json에도 추가한다.

#[tauri::command(async)]
fn app_state(workspace: Shared<'_>) -> Result<AppState, String> {
    let ws = lock(&workspace)?;
    Ok(AppState {
        state: ws.store.state().clone(),
        warning: ws.warning.clone(),
    })
}

#[tauri::command(async)]
fn shells_list(include_wsl: bool) -> Vec<ShellInfo> {
    shell::list(include_wsl)
}

#[tauri::command(async)]
fn project_detect(path: String) -> Result<Detected, String> {
    let folder = store::inspect(&path)?;
    Ok(Detected {
        kind: folder.kind,
        name: Path::new(&folder.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

#[tauri::command(async)]
fn project_add(
    workspace: Shared<'_>,
    name: String,
    path: String,
    shell: String,
    agent: Agent,
) -> Result<Created, String> {
    shell::kind(&shell)?;
    let folder = store::inspect(&path)?;
    let mut ws = lock(&workspace)?;
    let project = ws.store.add_project(&name, folder, &shell, agent)?;
    Ok(Created {
        id: project.id,
        state: ws.store.state().clone(),
    })
}

/// 등록과 세션 기록만 지우고 실행 중인 셸을 끝낸다. 폴더의 파일은 건드리지 않는다.
#[tauri::command(async)]
fn project_remove(
    workspace: Shared<'_>,
    sessions: State<'_, Sessions>,
    id: String,
) -> Result<StoredState, String> {
    let mut ws = lock(&workspace)?;
    for session in ws.store.remove_project(&id)? {
        ws.forget(&session);
        sessions.kill(&session);
    }
    Ok(ws.store.state().clone())
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
fn session_create(
    workspace: Shared<'_>,
    project_id: String,
    name: String,
    shell: String,
    agent: Agent,
    custom_command: String,
    prompt: String,
) -> Result<Created, String> {
    let mut ws = lock(&workspace)?;
    let session = ws.create_session(&project_id, &name, &shell, agent, &custom_command, &prompt)?;
    Ok(Created {
        id: session.id,
        state: ws.store.state().clone(),
    })
}

#[tauri::command(async)]
fn session_delete(
    workspace: Shared<'_>,
    sessions: State<'_, Sessions>,
    id: String,
) -> Result<StoredState, String> {
    let mut ws = lock(&workspace)?;
    ws.store.remove_session(&id)?;
    ws.forget(&id);
    sessions.kill(&id);
    Ok(ws.store.state().clone())
}

/// 저장한 기록대로 셸을 띄운다. 이미 떠 있으면 그 실행 번호를 돌려준다.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
fn session_start(
    app: AppHandle,
    workspace: Shared<'_>,
    sessions: State<'_, Sessions>,
    hook_server: State<'_, HookServer>,
    hook_states: State<'_, Arc<HookStates>>,
    file_tracker: State<'_, Arc<FileTracker>>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<SessionStarted, String> {
    let os_build = windows::build_number();
    // 이미 띄운 셸에 다시 붙을 때는 폴더나 셸 설치를 확인하지 않는다.
    if let Some(run) = sessions.running(&id) {
        return Ok(SessionStarted { run, os_build });
    }

    // Claude 에이전트이면 훅 설정 파일을 만들고 토큰을 HookServer에 등록한다.
    {
        let mut ws = lock(&workspace)?;
        let state = ws.store.state();
        let is_claude = state
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.agent == agent::Agent::Claude)
            .unwrap_or(false);
        let is_wsl = state
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.shell.starts_with("wsl:"))
            .unwrap_or(false);
        if is_claude && let Ok((_, token)) = ws.write_hook_settings(&id, hook_server.port(), is_wsl)
        {
            hook_server.register(&id, &token);
        }
    }

    // 폴더 확인은 응답 없는 네트워크 경로에서 오래 걸릴 수 있어 잠금 밖에서 한다.
    let launch = lock(&workspace)?.launch(&id)?;
    let (command, typed) = shell::command(&launch.shell, &launch.cwd, launch.startup.as_deref())?;
    // 삭제와 겹쳐 기록 없는 셸이 남지 않도록 기록을 다시 확인하고 시작이 끝날 때까지 잠근다.
    let mut ws = lock(&workspace)?;
    ws.launch(&id)?;

    // 새 실행 시작 시 훅 상태를 초기화한다.
    hook_states.states.lock().unwrap().remove(&id);

    let exit_id = id.clone();
    let run = sessions.start(&id, command, cols, rows, typed, move |run, code| {
        let _ = app.emit(
            "session-exit",
            SessionExit {
                id: exit_id,
                run,
                code,
            },
        );
    })?;
    // 실행 번호가 확정되면 엔트리를 만든다.
    let session_name = ws
        .store
        .state()
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| id.clone());
    hook_states.states.lock().unwrap().insert(
        id.clone(),
        HookEntry {
            run,
            name: session_name,
            live: None,
            context: None,
        },
    );

    // 세션이 속한 프로젝트 경로를 FileTracker에 등록한다.
    if let Some(project_path) = ws
        .store
        .state()
        .sessions
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| {
            ws.store
                .state()
                .projects
                .iter()
                .find(|p| p.id == s.project_id)
                .map(|p| p.path.clone())
        })
    {
        file_tracker.register_session(&id, &project_path);
    }

    ws.launched(&id);
    Ok(SessionStarted { run, os_build })
}

/// 승인 대기 중인 훅 요청에 응답한다.
/// `request`는 `PermissionRequest` 훅의 tool_use_id/tool_name (오래된 클릭 방지),
/// `choice`는 1(예) · 2(세션 동안 허용) · 3(아니요).
#[tauri::command(async)]
fn session_approve(
    sessions: State<'_, Sessions>,
    hook_states: State<'_, Arc<HookStates>>,
    id: String,
    run: u64,
    request: String,
    choice: u8,
) -> Result<(), String> {
    if choice == 0 || choice > 3 {
        return Err("선택 번호는 1·2·3이에요".into());
    }
    let states = hook_states.states.lock().unwrap();
    let entry = states.get(&id).ok_or("세션이 없어요")?;
    if entry.run != run {
        return Err("실행 번호가 맞지 않아요".into());
    }
    match &entry.live {
        Some(Live::Waiting { request: r }) if r == &request => {
            drop(states);
            sessions.write(&id, run, 0, format!("{choice}\r").into_bytes())
        }
        _ => Err("지금은 승인 대기 상태가 아니에요".into()),
    }
}

#[tauri::command(async)]
fn session_attach(
    sessions: State<'_, Sessions>,
    id: String,
    viewer: u64,
    on_output: Channel<InvokeResponseBody>,
) -> Result<AttachInfo, String> {
    sessions.attach(
        &id,
        viewer,
        Box::new(move |data| on_output.send(InvokeResponseBody::Raw(data)).is_ok()),
    )
}

#[tauri::command(async)]
fn session_detach(sessions: State<'_, Sessions>, id: String, viewer: u64) {
    sessions.detach(&id, viewer);
}

#[tauri::command(async)]
fn session_ack(sessions: State<'_, Sessions>, id: String, viewer: u64, bytes: usize) {
    sessions.ack(&id, viewer, bytes);
}

// 입력은 화면이 하나씩 차례로 보낸다. 실제 쓰기는 세션의 전용 스레드가 하므로 막히지 않는다.
#[tauri::command]
fn session_write(
    sessions: State<'_, Sessions>,
    id: String,
    run: u64,
    viewer: u64,
    data: String,
    binary: bool,
) -> Result<(), String> {
    // 바이너리 입력(마우스 보고)은 문자 하나가 바이트 하나다.
    let bytes = if binary {
        data.chars().map(|c| c as u8).collect()
    } else {
        data.into_bytes()
    };
    sessions.write(&id, run, viewer, bytes)
}

// 리사이즈도 화면이 하나씩 보낸다. OS 호출이 늦어도 UI가 멈추지 않게 메인 스레드 밖에서 돈다.
#[tauri::command(async)]
fn session_resize(
    sessions: State<'_, Sessions>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    sessions.resize(&id, cols, rows)
}

#[tauri::command(async)]
fn session_kill(sessions: State<'_, Sessions>, id: String) {
    sessions.kill(&id);
}

#[tauri::command(async)]
fn layout_set(workspace: Shared<'_>, tree: Node) -> Result<(), String> {
    lock(&workspace)?.store.set_layout(tree)
}

#[tauri::command(async)]
fn layout_save(workspace: Shared<'_>, name: String, tree: Node) -> Result<StoredState, String> {
    let mut ws = lock(&workspace)?;
    ws.store.save_layout(&name, tree)?;
    Ok(ws.store.state().clone())
}

#[tauri::command(async)]
fn layout_delete(workspace: Shared<'_>, id: String) -> Result<StoredState, String> {
    let mut ws = lock(&workspace)?;
    ws.store.delete_layout(&id)?;
    Ok(ws.store.state().clone())
}

#[tauri::command(async)]
fn account_list(registry: State<'_, Mutex<AccountRegistry>>) -> Vec<account::AccountSummary> {
    let reg = registry.lock().unwrap();
    reg.all_summaries(usage::now_ms())
}

// ─────────────────────── 변경 사항 명령 ───────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFilePayload {
    rel_path: String,
    status: String,
    session_ids: Vec<String>,
    is_conflict: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangeSummaryPayload {
    kind: String,
    revision: Option<String>,
    branch: Option<String>,
    files: Vec<ChangedFilePayload>,
    add_count: usize,
    del_count: usize,
}

#[tauri::command(async)]
async fn get_changes(
    workspace: Shared<'_>,
    file_tracker: State<'_, Arc<FileTracker>>,
    project_path: String,
) -> Result<ChangeSummaryPayload, String> {
    let kind = {
        let ws = lock(&workspace)?;
        ws.store
            .state()
            .projects
            .iter()
            .find(|p| p.path == project_path)
            .map(|p| p.kind)
            .unwrap_or(Kind::Folder)
    };

    let (entries_svn, entries_git, branch, revision) = match kind {
        Kind::Svn => {
            let entries = changes::svn_status(&project_path)
                .await
                .map_err(|e| e.to_string())?;
            (Some(entries), None, None, None)
        }
        Kind::Git => {
            let entries = changes::git_status(&project_path)
                .await
                .map_err(|e| e.to_string())?;
            let branch = get_git_branch(&project_path);
            (None, Some(entries), branch, None)
        }
        Kind::Folder => (None, None, None, None),
    };

    let mut files = vec![];
    let mut add_count = 0usize;
    let mut del_count = 0usize;

    if let Some(svn) = entries_svn {
        for e in &svn {
            let full_path = format!("{}/{}", project_path, e.rel_path);
            let session_ids = file_tracker.sessions_for_file(&full_path);
            let is_conflict = session_ids.len() > 1;
            if e.status == 'A' {
                add_count += 1;
            }
            if e.status == 'D' {
                del_count += 1;
            }
            files.push(ChangedFilePayload {
                rel_path: e.rel_path.clone(),
                status: e.status.to_string(),
                is_conflict,
                session_ids,
            });
        }
    }

    if let Some(git) = entries_git {
        for e in &git {
            let full_path = format!("{}/{}", project_path, e.rel_path);
            let session_ids = file_tracker.sessions_for_file(&full_path);
            let is_conflict = session_ids.len() > 1;
            if e.status == 'A' {
                add_count += 1;
            }
            if e.status == 'D' {
                del_count += 1;
            }
            files.push(ChangedFilePayload {
                rel_path: e.rel_path.clone(),
                status: e.status.to_string(),
                is_conflict,
                session_ids,
            });
        }
    }

    Ok(ChangeSummaryPayload {
        kind: format!("{kind:?}").to_lowercase(),
        revision,
        branch,
        files,
        add_count,
        del_count,
    })
}

fn get_git_branch(project_path: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["-C", project_path, "branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command(async)]
async fn get_diff(project_path: String, file_path: String, is_git: bool) -> Result<String, String> {
    if is_git {
        changes::git_diff_file(&project_path, &file_path)
            .await
            .map_err(|e| e.to_string())
    } else {
        changes::svn_diff_file(&project_path, &file_path)
            .await
            .map_err(|e| e.to_string())
    }
}

#[tauri::command(async)]
async fn svn_commit_cmd(
    project_path: String,
    files: Vec<String>,
    message: String,
) -> Result<String, String> {
    changes::svn_commit(&project_path, &files, &message)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn svn_revert_cmd(project_path: String, files: Vec<String>) -> Result<String, String> {
    changes::svn_revert(&project_path, &files)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn svn_update_cmd(project_path: String) -> Result<String, String> {
    changes::svn_update(&project_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn git_commit_cmd(
    project_path: String,
    files: Vec<String>,
    message: String,
) -> Result<String, String> {
    changes::git_commit(&project_path, &files, &message)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn git_worktree_list_cmd(project_path: String) -> Result<Vec<GitWorktreeInfo>, String> {
    changes::git_worktree_list(&project_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn git_worktree_add_cmd(
    project_path: String,
    branch: String,
    worktree_path: String,
) -> Result<String, String> {
    changes::git_worktree_add(&project_path, &branch, &worktree_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn git_worktree_remove_cmd(
    project_path: String,
    worktree_path: String,
) -> Result<String, String> {
    changes::git_worktree_remove(&project_path, &worktree_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn get_settings(workspace: Shared<'_>) -> Result<store::Settings, String> {
    Ok(lock(&workspace)?.store.state().settings.clone())
}

#[tauri::command(async)]
fn save_settings(workspace: Shared<'_>, settings: store::Settings) -> Result<(), String> {
    lock(&workspace)?.store.set_settings(settings)
}

#[tauri::command]
fn prevent_sleep(enable: bool) {
    use windows_sys::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };
    unsafe {
        if enable {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        } else {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

#[tauri::command(async)]
fn update_last_running(workspace: Shared<'_>, ids: Vec<String>) -> Result<(), String> {
    lock(&workspace)?.store.update_last_running(ids)
}

pub fn run() {
    windows::restrict_dll_search();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Sessions::default())
        .manage(Arc::new(HookStates::default()))
        .manage(Arc::new(FileTracker::default()))
        .manage(Mutex::new(AccountRegistry::new()))
        .setup(|app| {
            // LMUX_DATA_DIR은 확인 스크립트가 실제 데이터와 분리할 때 쓴다.
            let dir = match std::env::var_os("LMUX_DATA_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            let (settings, last_running) = {
                let ws = Workspace::open(&dir);
                let s = ws.store.state().settings.clone();
                let lr = ws.store.state().last_running.clone();
                app.manage(Mutex::new(ws));
                (s, lr)
            };

            // 세션 복원: restore_sessions가 켜져 있고 마지막 실행 세션이 있으면 이벤트 발송
            if settings.restore_sessions && !last_running.is_empty() {
                let _ = app.emit(
                    "restore-sessions",
                    serde_json::json!({ "sessionIds": last_running }),
                );
            }

            // 훅 수신 서버 시작 및 이벤트 처리.
            // Arc 클론을 클로저에 이동해 HookServer 스레드가 앱 종료 순서와 무관하게
            // 데이터를 안전하게 참조할 수 있도록 한다.
            let hook_app = app.handle().clone();
            let hs_arc = Arc::clone(&*app.state::<Arc<HookStates>>());
            let ft_arc = Arc::clone(&*app.state::<Arc<FileTracker>>());
            let server = HookServer::start(move |ev: SessionEvent| {
                process_hook_event(&hook_app, &*hs_arc, &*ft_arc, ev);
            })
            .expect("훅 수신 서버를 시작하지 못했어요");
            app.manage(server);

            // 30초마다 "usage-tick" 이벤트 발송 (프론트가 account_list를 호출해 갱신)
            let tick_app = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    let _ = tick_app.emit("usage-tick", ());
                }
            });

            // 시스템 트레이 아이콘
            use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            let menu = MenuBuilder::new(app)
                .item(&MenuItemBuilder::with_id("open", "Lmux 열기").build(app)?)
                .item(&PredefinedMenuItem::separator(app)?)
                .item(&MenuItemBuilder::with_id("quit", "종료").build(app)?)
                .build()?;
            TrayIconBuilder::new()
                .tooltip("Lmux")
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|icon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = icon.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_state,
            shells_list,
            project_detect,
            project_add,
            project_remove,
            session_create,
            session_delete,
            session_start,
            session_attach,
            session_detach,
            session_ack,
            session_write,
            session_resize,
            session_kill,
            layout_set,
            layout_save,
            layout_delete,
            session_approve,
            account_list,
            get_changes,
            get_diff,
            svn_commit_cmd,
            svn_revert_cmd,
            svn_update_cmd,
            git_commit_cmd,
            git_worktree_list_cmd,
            git_worktree_add_cmd,
            git_worktree_remove_cmd,
            get_settings,
            save_settings,
            prevent_sleep,
            update_last_running,
        ])
        .on_window_event(|win, ev| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                let minimize = win
                    .app_handle()
                    .state::<Mutex<Workspace>>()
                    .lock()
                    .ok()
                    .map(|ws| ws.store.state().settings.minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    api.prevent_close();
                    let _ = win.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<Sessions>().kill_all();
                if let Ok(mut ws) = app.state::<Mutex<Workspace>>().lock() {
                    ws.remove_all_hook_settings();
                }
            }
        });
}

fn any_session_working(states: &HookStates) -> bool {
    states
        .states
        .lock()
        .unwrap()
        .values()
        .any(|e| matches!(e.live, Some(Live::Working)))
}

/// 훅 이벤트를 받아 세션 상태를 갱신하고 Tauri 이벤트를 발행한다.
fn process_hook_event(
    app: &AppHandle,
    states: &HookStates,
    file_tracker: &FileTracker,
    ev: SessionEvent,
) {
    use tauri_plugin_notification::NotificationExt;

    let hook = match events::parse(&ev.hook, &ev.body) {
        Ok(h) => h,
        Err(_) => return,
    };

    // PostToolUse: 파일 경로 추출·추적, 충돌 감지
    if ev.hook == "PostToolUse" {
        let paths = events::extract_files("PostToolUse", &ev.body);
        for path in &paths {
            file_tracker.track(&ev.session, path);
        }
        if !paths.is_empty()
            && let Some(project_path) = file_tracker.project_path(&ev.session)
        {
            let conflicts = file_tracker.conflicts(&ev.session, &project_path);
            if !conflicts.is_empty() {
                let _ = app.emit(
                    "conflict-detected",
                    ConflictDetected {
                        session_id: ev.session.clone(),
                        files: conflicts,
                    },
                );
            }
        }
    }

    let mut guard = states.states.lock().unwrap();
    let Some(entry) = guard.get_mut(&ev.session) else {
        return; // 등록되지 않은 세션의 늦은 이벤트 무시
    };

    // 이전 live 상태를 가져와 전이 계산
    let prev = entry.live.clone();
    let next_live = events::next(prev.clone(), hook.clone());
    let notify = notifications::should_notify(prev.as_ref(), next_live.as_ref());
    let notify_title = next_live
        .as_ref()
        .map(|l| notifications::notification_title(&entry.name, l));
    entry.live = next_live;

    // 절전 방지: Working 전이 시 활성화, 모든 세션이 Done일 때 해제
    let now_working = matches!(entry.live, Some(Live::Working));
    drop(guard);
    if now_working {
        prevent_sleep(true);
    } else if !any_session_working(states) {
        prevent_sleep(false);
    }

    // 컨텍스트는 Stop 이벤트에서 transcript를 읽어 갱신 (추후 구현; 현재는 유지)
    let guard = states.states.lock().unwrap();
    let Some(entry) = guard.get(&ev.session) else {
        return;
    };
    let payload = SessionStatePayload {
        id: ev.session.clone(),
        run: entry.run,
        live: entry.live.clone().map(LivePayload::from),
        context: entry.context.clone().map(ContextPayload::from),
    };
    drop(guard);

    let _ = app.emit("session-state", payload);

    // 세션 종료 시 파일 추적 정보 삭제
    if matches!(hook, events::Hook::End) {
        file_tracker.clear(&ev.session);
        file_tracker.unregister_session(&ev.session);
    }

    // 창이 활성이 아닐 때만 알림 발송
    if notify && let Some(title) = notify_title {
        let is_focused = app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if !is_focused {
            let _ = app.notification().builder().title(&title).show();
        }
    }
}
