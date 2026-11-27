//! 저장한 기록으로 세션을 어떻게 띄울지 정한다.

use crate::agent::{Agent, command_line};
use crate::shell;
use crate::store::{SessionRecord, Store};
use std::collections::HashMap;
use std::path::Path;

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
}

impl Workspace {
    pub fn open(dir: &Path) -> Workspace {
        let (store, warning) = Store::open(dir);
        Workspace {
            store,
            warning,
            prompts: HashMap::new(),
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
        Ok(Launch {
            shell: session.shell.clone(),
            cwd: project.path.clone(),
            startup: command_line(
                shell::kind(&session.shell)?,
                session.agent,
                &session.custom_command,
                prompt,
            ),
        })
    }

    /// 셸을 띄웠으면 첫 지시를 버린다. 다시 시작할 때는 에이전트만 실행한다.
    pub fn launched(&mut self, id: &str) {
        self.forget(id);
    }

    pub fn forget(&mut self, id: &str) {
        self.prompts.remove(id);
    }
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
}
