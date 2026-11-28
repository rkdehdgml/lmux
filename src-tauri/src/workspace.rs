//! 저장한 기록으로 세션을 어떻게 띄울지 정한다.

use crate::agent::{Agent, command_line};
use crate::hooks;
use crate::shell;
use crate::store::{SessionRecord, Store};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// cmd의 명령 줄 한도(8191자) 안에 들어가는 길이.
const MAX_PROMPT: usize = 4000;

#[derive(Debug)]
pub struct Launch {
    pub shell: String,
    pub cwd: String,
    /// 셸이 뜬 뒤 입력할 명령 줄.
    pub startup: Option<String>,
}

pub struct Workspace {
    pub store: Store,
    pub warning: Option<String>,
    /// 아직 시작하지 않은 세션의 첫 지시. 저장하지 않는다.
    prompts: HashMap<String, String>,
    /// 훅 설정 파일을 보관하는 디렉터리.
    pub hooks_dir: PathBuf,
    /// 세션별 훅 토큰. HookServer에 등록된 것과 일치해야 한다.
    tokens: HashMap<String, String>,
}

impl Workspace {
    pub fn open(dir: &Path) -> Workspace {
        let (store, warning) = Store::open(dir);
        let hooks_dir = dir.join("hooks");
        // 이전 실행에서 남은 설정 파일을 정리한다.
        hooks::remove_all(&hooks_dir);
        Workspace {
            store,
            warning,
            prompts: HashMap::new(),
            hooks_dir,
            tokens: HashMap::new(),
        }
    }

    pub fn create_session(
        &mut self,
        project_id: &str,
        name: &str,
        shell_id: &str,
        agent: Agent,
        custom_command: &str,
        prompt: &str,
    ) -> Result<SessionRecord, String> {
        shell::kind(shell_id)?;
        if prompt.chars().count() > MAX_PROMPT {
            return Err(format!("첫 지시는 {MAX_PROMPT}자까지 넣을 수 있어요"));
        }
        let session = self
            .store
            .add_session(project_id, name, shell_id, agent, custom_command)?;
        if !prompt.trim().is_empty() {
            self.prompts.insert(session.id.clone(), prompt.into());
        }
        Ok(session)
    }

    pub fn launch(&self, id: &str) -> Result<Launch, String> {
        if self.store.used_elsewhere() {
            // 두 창이 같은 폴더에서 에이전트를 겹쳐 띄우지 않게 한다.
            return Err("다른 Lmux 창에서 사용 중이에요. 그 창에서 세션을 열어 주세요".into());
        }
        let state = self.store.state();
        let session = state
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or("세션이 없어요")?;
        let project = state
            .projects
            .iter()
            .find(|p| p.id == session.project_id)
            .ok_or("프로젝트가 없어요")?;
        let prompt = self.prompts.get(id).map_or("", String::as_str);
        let is_wsl = session.shell.starts_with("wsl:");
        let settings_path = self.tokens.get(id).map(|_| {
            let win = self
                .hooks_dir
                .join(format!("{id}.json"))
                .to_string_lossy()
                .into_owned();
            if is_wsl {
                windows_to_wsl_path(&win)
            } else {
                win
            }
        });
        Ok(Launch {
            shell: session.shell.clone(),
            cwd: project.path.clone(),
            startup: command_line(
                shell::kind(&session.shell)?,
                session.agent,
                &session.custom_command,
                prompt,
                settings_path.as_deref(),
            ),
        })
    }

    /// Claude 세션의 훅 설정 파일을 만들고 토큰을 반환한다.
    /// 반환값: (settings 파일 경로, 토큰)
    pub fn write_hook_settings(
        &mut self,
        session_id: &str,
        port: u16,
        is_wsl: bool,
    ) -> std::io::Result<(PathBuf, String)> {
        let token = generate_token();
        hooks::write(&self.hooks_dir, session_id, port, &token, is_wsl)?;
        self.tokens.insert(session_id.to_string(), token.clone());
        Ok((self.hooks_dir.join(format!("{session_id}.json")), token))
    }

    /// 훅 설정 파일과 토큰을 삭제한다.
    pub fn remove_hook_settings(&mut self, session_id: &str) {
        self.tokens.remove(session_id);
        hooks::remove(&self.hooks_dir, session_id);
    }

    /// 앱 종료 시 모든 훅 설정 파일을 삭제한다.
    pub fn remove_all_hook_settings(&mut self) {
        self.tokens.clear();
        hooks::remove_all(&self.hooks_dir);
    }

    /// 셸을 띄웠으면 첫 지시를 버린다. 다시 시작할 때는 에이전트만 실행한다.
    pub fn launched(&mut self, id: &str) {
        self.forget(id);
    }

    pub fn forget(&mut self, id: &str) {
        self.prompts.remove(id);
        self.remove_hook_settings(id);
    }
}

/// Windows 절대 경로를 WSL의 POSIX 마운트 경로로 변환한다.
/// `C:\Users\foo` → `/mnt/c/Users/foo`. 인식할 수 없는 형식이면 원본을 그대로 돌려준다.
fn windows_to_wsl_path(win_path: &str) -> String {
    let p = win_path.replace('\\', "/");
    if p.len() >= 3 {
        let drive = p.chars().next().unwrap().to_ascii_lowercase();
        if drive.is_ascii_alphabetic()
            && p.chars().nth(1) == Some(':')
            && p.chars().nth(2) == Some('/')
        {
            return format!("/mnt/{}/{}", drive, &p[3..]);
        }
    }
    win_path.to_string()
}

/// 세션별로 유일한 훅 토큰을 생성한다.
/// 형식: 초(8) + 나노초(8) + 카운터(4) = 20 hex 문자 (~80비트 예측 불가).
fn generate_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{secs:08x}{nanos:08x}{seq:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;

    fn workspace(name: &str) -> (Workspace, String, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("lmux-ws-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let mut ws = Workspace::open(&dir.join("data"));
        let project = ws
            .store
            .add_project(
                "p",
                store::inspect(work.to_str().unwrap()).unwrap(),
                "cmd",
                Agent::None,
            )
            .unwrap();
        (ws, project.id, dir)
    }

    #[test]
    fn uses_the_first_prompt_only_until_the_first_start() {
        let (mut ws, project, dir) = workspace("prompt");
        let session = ws
            .create_session(&project, "n", "gitbash", Agent::Claude, "", "고쳐 줘")
            .unwrap();
        let first = ws.launch(&session.id).unwrap();
        assert_eq!(first.shell, "gitbash");
        assert_eq!(first.cwd, dir.join("work").to_str().unwrap());
        assert_eq!(first.startup.as_deref(), Some("claude '고쳐 줘'"));
        // 시작에 실패하면 다음 시도에 다시 쓴다.
        assert_eq!(ws.launch(&session.id).unwrap().startup, first.startup);
        ws.launched(&session.id);
        assert_eq!(
            ws.launch(&session.id).unwrap().startup.as_deref(),
            Some("claude")
        );
        let saved = std::fs::read_to_string(dir.join("data/state.json")).unwrap();
        assert!(!saved.contains("고쳐 줘"), "첫 지시는 저장하지 않는다");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_second_instance_does_not_start_sessions() {
        let (mut first, project, dir) = workspace("second");
        let session = first
            .create_session(&project, "n", "cmd", Agent::None, "", "")
            .unwrap();
        let second = Workspace::open(&dir.join("data"));
        assert!(
            second
                .launch(&session.id)
                .unwrap_err()
                .contains("다른 Lmux")
        );
        assert!(first.launch(&session.id).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn forgets_the_prompt_of_a_removed_session() {
        let (mut ws, project, dir) = workspace("forget");
        let session = ws
            .create_session(&project, "n", "gitbash", Agent::Claude, "", "일")
            .unwrap();
        ws.forget(&session.id);
        assert_eq!(
            ws.launch(&session.id).unwrap().startup.as_deref(),
            Some("claude")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_unknown_shells_and_oversized_prompts() {
        let (mut ws, project, dir) = workspace("reject");
        assert!(
            ws.create_session(&project, "", "zsh", Agent::None, "", "")
                .is_err()
        );
        let long = "가".repeat(MAX_PROMPT + 1);
        assert!(
            ws.create_session(&project, "", "cmd", Agent::Claude, "", &long)
                .is_err()
        );
        assert!(ws.store.state().sessions.is_empty());
        assert!(ws.launch("s999").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hook_settings_created_and_deleted_with_session() {
        let (mut ws, project, dir) = workspace("hook-lifecycle");
        let session = ws
            .create_session(&project, "n", "powershell", Agent::Claude, "", "")
            .unwrap();

        // 토큰 등록 전에는 --settings 없음
        let before = ws.launch(&session.id).unwrap();
        assert!(
            !before
                .startup
                .as_deref()
                .unwrap_or("")
                .contains("--settings"),
            "before: {:?}",
            before.startup
        );

        // 토큰 등록 후: 파일 생성 + --settings 추가됨
        let (path, _) = ws.write_hook_settings(&session.id, 9900, false).unwrap();
        assert!(path.exists(), "settings file must exist");
        let after = ws.launch(&session.id).unwrap();
        assert!(
            after
                .startup
                .as_deref()
                .unwrap_or("")
                .contains("--settings"),
            "after: {:?}",
            after.startup
        );

        // forget() 호출 시 파일 삭제
        ws.forget(&session.id);
        assert!(!path.exists(), "settings file must be deleted after forget");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hook_settings_deleted_on_app_exit() {
        let (mut ws, project, dir) = workspace("hook-exit");
        let session = ws
            .create_session(&project, "n", "powershell", Agent::Claude, "", "")
            .unwrap();
        let (path, _) = ws.write_hook_settings(&session.id, 9901, false).unwrap();
        assert!(path.exists());

        ws.remove_all_hook_settings();
        assert!(!path.exists(), "settings file must be deleted on exit");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn startup_includes_settings_in_launch() {
        let (mut ws, project, dir) = workspace("hook-startup");
        let session = ws
            .create_session(&project, "n", "gitbash", Agent::Claude, "", "fix it")
            .unwrap();
        ws.write_hook_settings(&session.id, 9902, false).unwrap();

        let launch = ws.launch(&session.id).unwrap();
        let startup = launch.startup.unwrap();
        assert!(startup.contains("--settings"), "startup: {startup}");
        assert!(startup.contains("fix it"), "startup: {startup}");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn token_has_sufficient_length_for_entropy() {
        // 20 hex chars (secs + subsec_nanos + counter) — fails with the old 12-char format
        for _ in 0..5 {
            let token = generate_token();
            assert!(
                token.len() >= 20,
                "token too short for adequate entropy: {token} (len {})",
                token.len()
            );
        }
    }

    #[test]
    fn wsl_session_gets_posix_settings_path() {
        let (mut ws, project, dir) = workspace("wsl-settings");
        let session = ws
            .create_session(&project, "n", "wsl:Ubuntu", Agent::Claude, "", "")
            .unwrap();
        ws.write_hook_settings(&session.id, 9903, true).unwrap();
        let launch = ws.launch(&session.id).unwrap();
        let startup = launch.startup.unwrap();
        // WSL session must get a POSIX /mnt/... path, not a Windows backslash path
        assert!(
            !startup.contains('\\'),
            "WSL settings path must not contain backslashes: {startup}"
        );
        assert!(
            startup.contains("/mnt/"),
            "WSL settings path must be POSIX /mnt/ path: {startup}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
