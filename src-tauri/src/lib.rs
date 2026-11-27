mod agent;
mod layout;
mod ring;
mod session;
mod shell;
mod store;
mod windows;
mod workspace;

use agent::Agent;
use layout::Node;
use session::{AttachInfo, Sessions};
use shell::ShellInfo;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use store::{Kind, State as StoredState};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};
use workspace::Workspace;

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
fn session_start(
    app: AppHandle,
    workspace: Shared<'_>,
    sessions: State<'_, Sessions>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<SessionStarted, String> {
    let os_build = windows::build_number();
    // 이미 띄운 셸에 다시 붙을 때는 폴더나 셸 설치를 확인하지 않는다.
    if let Some(run) = sessions.running(&id) {
        return Ok(SessionStarted { run, os_build });
    }
    // 폴더 확인은 응답 없는 네트워크 경로에서 오래 걸릴 수 있어 잠금 밖에서 한다.
    let launch = lock(&workspace)?.launch(&id)?;
    let (command, typed) = shell::command(&launch.shell, &launch.cwd, launch.startup.as_deref())?;
    // 삭제와 겹쳐 기록 없는 셸이 남지 않도록 기록을 다시 확인하고 시작이 끝날 때까지 잠근다.
    let mut ws = lock(&workspace)?;
    ws.launch(&id)?;
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
    ws.launched(&id);
    Ok(SessionStarted { run, os_build })
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

pub fn run() {
    windows::restrict_dll_search();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Sessions::default())
        .setup(|app| {
            // LMUX_DATA_DIR은 확인 스크립트가 실제 데이터와 분리할 때 쓴다.
            let dir = match std::env::var_os("LMUX_DATA_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            app.manage(Mutex::new(Workspace::open(&dir)));
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
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<Sessions>().kill_all();
            }
        });
}
