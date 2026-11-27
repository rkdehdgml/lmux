use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// 세션별 수정 파일 추적기. 충돌 감지(2개 이상 세션이 같은 파일 수정)에 사용된다.
#[derive(Default)]
pub struct FileTracker {
    files: Mutex<HashMap<String, HashSet<String>>>,
    /// session_id → project_path (훅 핸들러가 workspace 없이 충돌을 판단할 수 있도록)
    session_projects: Mutex<HashMap<String, String>>,
}

impl FileTracker {
    /// 세션이 시작될 때 세션과 프로젝트 경로를 연결한다.
    pub fn register_session(&self, session: &str, project_path: &str) {
        self.session_projects
            .lock()
            .unwrap()
            .insert(session.to_string(), project_path.to_string());
    }

    /// 세션 종료 시 프로젝트 연결을 해제한다.
    pub fn unregister_session(&self, session: &str) {
        self.session_projects.lock().unwrap().remove(session);
    }

    /// 세션이 속한 프로젝트 경로를 반환한다.
    pub fn project_path(&self, session: &str) -> Option<String> {
        self.session_projects.lock().unwrap().get(session).cloned()
    }

    /// 세션이 파일을 수정했음을 기록한다.
    pub fn track(&self, session: &str, path: &str) {
        let mut guard = self.files.lock().unwrap();
        guard
            .entry(session.to_string())
            .or_default()
            .insert(path.to_string());
    }

    /// 세션이 수정한 파일 집합을 반환한다.
    #[allow(dead_code)]
    pub fn files(&self, session: &str) -> HashSet<String> {
        self.files
            .lock()
            .unwrap()
            .get(session)
            .cloned()
            .unwrap_or_default()
    }

    /// 세션 종료 시 추적 정보를 지운다.
    pub fn clear(&self, session: &str) {
        self.files.lock().unwrap().remove(session);
    }

    /// project_path 하위 파일을 수정한 다른 세션이 있으면 충돌 경로 목록을 반환한다.
    pub fn conflicts(&self, session: &str, project_path: &str) -> Vec<String> {
        let guard = self.files.lock().unwrap();
        let my_files = match guard.get(session) {
            Some(f) => f,
            None => return vec![],
        };
        let mut result = vec![];
        for (other_session, other_files) in guard.iter() {
            if other_session == session {
                continue;
            }
            for path in other_files {
                if path.starts_with(project_path) && my_files.contains(path) {
                    result.push(path.clone());
                }
            }
        }
        result.sort();
        result.dedup();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_when_two_sessions_share_file() {
        let ft = FileTracker::default();
        ft.track("s1", "/proj/a.rs");
        ft.track("s2", "/proj/a.rs");
        let c = ft.conflicts("s1", "/proj");
        assert!(c.contains(&"/proj/a.rs".to_string()));
    }

    #[test]
    fn cleared_on_session_end() {
        let ft = FileTracker::default();
        ft.track("s1", "/proj/a.rs");
        ft.clear("s1");
        assert!(ft.files("s1").is_empty());
    }

    #[test]
    fn no_conflict_when_different_files() {
        let ft = FileTracker::default();
        ft.track("s1", "/proj/a.rs");
        ft.track("s2", "/proj/b.rs");
        let c = ft.conflicts("s1", "/proj");
        assert!(c.is_empty());
    }
}
