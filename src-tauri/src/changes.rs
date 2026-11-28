#![allow(dead_code)] // Task 4에서 Tauri 명령에 연결 시 제거
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::Mutex;

const MAX_DIFF_BYTES: usize = 200 * 1024; // 200 KB

/// SVN `svn status` 한 줄 결과.
#[derive(Debug, PartialEq)]
pub struct SvnEntry {
    pub rel_path: String,
    pub status: char,
}

/// Git `git status --porcelain` 한 줄 결과.
#[derive(Debug, PartialEq)]
pub struct GitEntry {
    pub rel_path: String,
    pub status: char,
}

/// Git 워크트리 항목.
#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeInfo {
    pub path: String,
    pub branch: String,
    pub is_locked: bool,
}

// ─────────────────────── SVN 명령 ───────────────────────

/// `svn status` 출력(텍스트)을 파싱한다.
pub fn parse_svn_status(output: &[u8]) -> Vec<SvnEntry> {
    output
        .split(|&b| b == b'\n')
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }
            let ch = *line.first()? as char;
            let path = std::str::from_utf8(line.get(1..)?).ok()?.trim().to_string();
            if path.is_empty() {
                return None;
            }
            Some(SvnEntry {
                status: ch,
                rel_path: path,
            })
        })
        .collect()
}

pub async fn svn_status(project_path: &str) -> anyhow::Result<Vec<SvnEntry>> {
    let output = Command::new("svn")
        .args(["status"])
        .current_dir(project_path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "svn CLI를 찾지 못했어요. TortoiseSVN의 커맨드라인 도구를 설치해 주세요."
                )
            } else {
                anyhow::anyhow!("svn 실행 오류: {e}")
            }
        })?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "svn status 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(parse_svn_status(&output.stdout))
}

pub async fn svn_diff_file(project_path: &str, rel_path: &str) -> anyhow::Result<String> {
    let output = Command::new("svn")
        .args(["diff", rel_path])
        .current_dir(project_path)
        .output()
        .map_err(|e| anyhow::anyhow!("svn 실행 오류: {e}"))?;
    let mut s = String::from_utf8_lossy(&output.stdout).into_owned();
    if s.len() > MAX_DIFF_BYTES {
        s.truncate(MAX_DIFF_BYTES);
        s.push_str("\n... (diff truncated) ...");
    }
    Ok(s)
}

pub async fn svn_update(project_path: &str) -> anyhow::Result<String> {
    let output = Command::new("svn")
        .args(["update"])
        .current_dir(project_path)
        .output()
        .map_err(|e| anyhow::anyhow!("svn 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "svn update 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn svn_commit(
    project_path: &str,
    files: &[String],
    message: &str,
) -> anyhow::Result<String> {
    let mut args = vec!["commit", "-m", message];
    for f in files {
        args.push(f.as_str());
    }
    let output = Command::new("svn")
        .args(&args)
        .current_dir(project_path)
        .output()
        .map_err(|e| anyhow::anyhow!("svn 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "svn commit 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn svn_revert(project_path: &str, files: &[String]) -> anyhow::Result<String> {
    let mut args = vec!["revert"];
    for f in files {
        args.push(f.as_str());
    }
    let output = Command::new("svn")
        .args(&args)
        .current_dir(project_path)
        .output()
        .map_err(|e| anyhow::anyhow!("svn 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "svn revert 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ─────────────────────── Git 명령 ───────────────────────

/// `git status --porcelain` 출력을 파싱한다.
pub fn parse_git_status(output: &[u8]) -> Vec<GitEntry> {
    output
        .split(|&b| b == b'\n')
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let xy = std::str::from_utf8(&line[..2]).ok()?;
            let status_char = xy.chars().find(|c| !c.is_whitespace()).unwrap_or('?');
            let path = std::str::from_utf8(line.get(3..)?).ok()?.trim().to_string();
            if path.is_empty() {
                return None;
            }
            Some(GitEntry {
                status: status_char,
                rel_path: path,
            })
        })
        .collect()
}

/// `git worktree list --porcelain` 출력을 파싱한다.
pub fn parse_git_worktree_list(output: &[u8]) -> Vec<GitWorktreeInfo> {
    let text = String::from_utf8_lossy(output);
    let mut result = vec![];
    let mut current_path = String::new();
    let mut current_branch = String::new();
    let mut is_locked = false;

    for line in text.lines() {
        if line.starts_with("worktree ") {
            if !current_path.is_empty() {
                result.push(GitWorktreeInfo {
                    path: current_path.clone(),
                    branch: current_branch.clone(),
                    is_locked,
                });
            }
            current_path = line.trim_start_matches("worktree ").to_string();
            current_branch = String::new();
            is_locked = false;
        } else if line.starts_with("branch ") {
            current_branch = line
                .trim_start_matches("branch refs/heads/")
                .trim_start_matches("branch ")
                .to_string();
        } else if line.starts_with("locked") {
            is_locked = true;
        }
    }
    if !current_path.is_empty() {
        result.push(GitWorktreeInfo {
            path: current_path,
            branch: current_branch,
            is_locked,
        });
    }
    result
}

pub async fn git_status(project_path: &str) -> anyhow::Result<Vec<GitEntry>> {
    let output = Command::new("git")
        .args(["-C", project_path, "status", "--porcelain"])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git status 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(parse_git_status(&output.stdout))
}

pub async fn git_diff_file(project_path: &str, rel_path: &str) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["-C", project_path, "diff", rel_path])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    let mut s = String::from_utf8_lossy(&output.stdout).into_owned();
    if s.len() > MAX_DIFF_BYTES {
        s.truncate(MAX_DIFF_BYTES);
        s.push_str("\n... (diff truncated) ...");
    }
    Ok(s)
}

pub async fn git_commit(
    project_path: &str,
    files: &[String],
    message: &str,
) -> anyhow::Result<String> {
    // 먼저 지정 파일을 스테이징
    let mut add_args = vec!["-C", project_path, "add", "--"];
    for f in files {
        add_args.push(f.as_str());
    }
    let add_out = Command::new("git")
        .args(&add_args)
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !add_out.status.success() {
        return Err(anyhow::anyhow!(
            "git add 오류: {}",
            String::from_utf8_lossy(&add_out.stderr)
        ));
    }
    let output = Command::new("git")
        .args(["-C", project_path, "commit", "-m", message])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git commit 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn git_worktree_list(project_path: &str) -> anyhow::Result<Vec<GitWorktreeInfo>> {
    let output = Command::new("git")
        .args(["-C", project_path, "worktree", "list", "--porcelain"])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git worktree list 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(parse_git_worktree_list(&output.stdout))
}

pub async fn git_worktree_add(
    project_path: &str,
    branch: &str,
    worktree_path: &str,
) -> anyhow::Result<String> {
    // 경로 이미 존재 여부 확인
    if std::path::Path::new(worktree_path).exists() {
        return Err(anyhow::anyhow!("이미 있는 경로예요: {worktree_path}"));
    }
    let output = Command::new("git")
        .args(["-C", project_path, "worktree", "add", worktree_path, branch])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git worktree add 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn git_worktree_remove(
    project_path: &str,
    worktree_path: &str,
) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args([
            "-C",
            project_path,
            "worktree",
            "remove",
            "--force",
            worktree_path,
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("git 실행 오류: {e}"))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git worktree remove 오류: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

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

    /// 해당 경로를 수정한 모든 세션 목록을 반환한다.
    pub fn sessions_for_file(&self, path: &str) -> Vec<String> {
        self.files
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, files)| files.contains(path))
            .map(|(s, _)| s.clone())
            .collect()
    }

    /// 주어진 프로젝트 경로에 속한 세션 목록을 반환한다.
    pub fn sessions_in_project(&self, project_path: &str) -> Vec<String> {
        self.session_projects
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p)| p.as_str() == project_path)
            .map(|(s, _)| s.clone())
            .collect()
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

    // --- SVN ---

    #[test]
    fn parses_svn_status_output() {
        let result = parse_svn_status(b"? unversioned.txt\nM  modified.java\n");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].status, '?');
        assert_eq!(result[0].rel_path, "unversioned.txt");
        assert_eq!(result[1].status, 'M');
        assert_eq!(result[1].rel_path, "modified.java");
    }

    #[test]
    fn svn_status_ignores_blank_lines() {
        let result = parse_svn_status(b"\nM  foo.rs\n\n");
        assert_eq!(result.len(), 1);
    }

    // --- Git ---

    #[test]
    fn parses_git_status_output() {
        let out = b" M src/lib.rs\n?? new_file.txt\n";
        let entries = parse_git_status(out);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].status, 'M');
        assert_eq!(entries[0].rel_path, "src/lib.rs");
        assert_eq!(entries[1].rel_path, "new_file.txt");
    }

    #[test]
    fn parses_git_worktree_list_output() {
        let list_output = b"worktree /a/main\nHEAD abc123\nbranch refs/heads/main\n\nworktree /a/feat\nHEAD def456\nbranch refs/heads/feat\n";
        let list = parse_git_worktree_list(list_output);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].branch, "main");
        assert_eq!(list[1].path, "/a/feat");
    }

    // --- FileTracker ---

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
