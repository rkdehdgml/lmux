#![allow(dead_code)] // Task 5에서 연결 시 제거
use std::io;
use std::path::{Path, PathBuf};

/// 세션별 Claude --settings JSON을 생성한다.
/// 훅 명령은 항상 정상 종료(|| true)하고 timeout: 5초.
pub fn settings_json(port: u16, session: &str, token: &str, wsl: bool) -> String {
    let url_base = format!("http://127.0.0.1:{port}/event/{session}/{token}");
    let curl = if wsl {
        // WSL: curl.exe 우선, 없으면 curl
        format!(
            "curl.exe -s -m 3 -X POST --data-binary @- '{url_base}/${{CLAUDE_HOOK_EVENT}}' 2>/dev/null || curl -s -m 3 -X POST --data-binary @- '{url_base}/${{CLAUDE_HOOK_EVENT}}' 2>/dev/null || true"
        )
    } else {
        format!(
            "curl.exe -s -m 3 -X POST --data-binary @- \"{url_base}/%CLAUDE_HOOK_EVENT%\" || true"
        )
    };

    let events = [
        "UserPromptSubmit",
        "PreToolUse",
        "PermissionRequest",
        "Stop",
        "SessionEnd",
    ];
    let hooks_json: String = events
        .iter()
        .map(|e| {
            format!(
                r#""{e}":[{{"hooks":[{{"type":"command","command":{cmd},"timeout":5}}]}}]"#,
                cmd = serde_json::to_string(&curl).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(r#"{{"hooks":{{{hooks_json}}}}}"#)
}

pub fn write(dir: &Path, session: &str, port: u16, token: &str, wsl: bool) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{session}.json"));
    let json = settings_json(port, session, token, wsl);
    std::fs::write(&path, json)?;
    Ok(path)
}

pub fn remove(dir: &Path, session: &str) {
    let path = dir.join(format!("{session}.json"));
    let _ = std::fs::remove_file(path);
}

pub fn remove_all(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generated_json_is_valid() {
        let json = settings_json(9000, "sess-1", "tok-abc", false);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed["hooks"].is_object());
    }

    #[test]
    fn url_contains_session_and_token_only() {
        let json = settings_json(9000, "sess-XYZ", "tok-123", false);
        assert!(json.contains("sess-XYZ"));
        assert!(json.contains("tok-123"));
        // 외부 입력(경로·프롬프트)이 URL에 없음
        assert!(!json.contains("project"));
    }

    #[test]
    fn all_five_hook_events_present() {
        let json = settings_json(9000, "s", "t", false);
        for event in &[
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "Stop",
            "SessionEnd",
        ] {
            assert!(json.contains(event), "missing event: {event}");
        }
    }

    #[test]
    fn hook_command_always_exits_zero() {
        let json = settings_json(9000, "s", "t", false);
        // Windows: "|| true" 또는 명시적 종료 코드 0
        assert!(json.contains("|| true"), "hook must always exit 0");
    }

    #[test]
    fn hook_has_timeout_5() {
        let json = settings_json(9000, "s", "t", false);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let hooks = parsed["hooks"]["Stop"][0]["hooks"][0].clone();
        assert_eq!(hooks["timeout"], 5);
    }

    #[test]
    fn wsl_uses_curl_exe_first() {
        let json = settings_json(9000, "s", "t", true);
        assert!(json.contains("curl.exe"));
    }

    #[test]
    fn write_creates_file_and_remove_deletes_it() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "sess-1", 9000, "tok", false).unwrap();
        assert!(path.exists());
        remove(dir.path(), "sess-1");
        assert!(!path.exists());
    }

    #[test]
    fn remove_all_deletes_all_json_files() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "sess-1", 9000, "tok1", false).unwrap();
        write(dir.path(), "sess-2", 9000, "tok2", false).unwrap();
        // 비-json 파일은 그대로여야 함
        std::fs::write(dir.path().join("other.txt"), "x").unwrap();
        remove_all(dir.path());
        assert!(!dir.path().join("sess-1.json").exists());
        assert!(!dir.path().join("sess-2.json").exists());
        assert!(dir.path().join("other.txt").exists());
    }

    #[test]
    fn remove_nonexistent_file_is_noop() {
        let dir = TempDir::new().unwrap();
        // 존재하지 않는 세션 삭제 — 오류 없이 종료
        remove(dir.path(), "no-such-session");
    }

    #[test]
    fn remove_all_on_missing_dir_is_noop() {
        remove_all(Path::new("C:/nonexistent/path/xyz"));
    }
}
