#![allow(dead_code)] // Task 5에서 hook_server/workspace에 연결 시 제거
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub enum Hook {
    Prompt,
    ToolUse,
    Permission { request: String },
    Stop,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    pub used: u64,
    pub size: u64,
    pub estimated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Live {
    Working,
    Waiting { request: String },
    Done,
}

#[derive(Deserialize)]
struct HookBody {
    hook_event_name: Option<String>,
    tool_use_id: Option<String>,
    tool_name: Option<String>,
}

pub fn parse(event: &str, body: &[u8]) -> Result<Hook, String> {
    match event {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => match event {
            "UserPromptSubmit" => Ok(Hook::Prompt),
            _ => Ok(Hook::ToolUse),
        },
        "PermissionRequest" => {
            let b: HookBody =
                serde_json::from_slice(body).map_err(|e| format!("parse error: {e}"))?;
            let request = b
                .tool_use_id
                .or(b.tool_name)
                .unwrap_or_else(|| "unknown".to_string());
            Ok(Hook::Permission { request })
        }
        "Stop" => Ok(Hook::Stop),
        "SessionEnd" | "SubagentStop" => Ok(Hook::End),
        other => Err(format!("unknown event: {other}")),
    }
}

/// PostToolUse 이벤트 body에서 수정된 파일 경로를 추출한다.
/// Write/Edit/MultiEdit/NotebookEdit 도구만 처리하고, 나머지는 빈 Vec을 반환한다.
pub fn extract_files(hook: &str, body: &[u8]) -> Vec<String> {
    if hook != "PostToolUse" {
        return vec![];
    }
    let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) else {
        return vec![];
    };
    let tool_name = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    let input = &val["tool_input"];
    match tool_name {
        "Write" | "Edit" | "NotebookEdit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                vec![path.to_string()]
            } else {
                vec![]
            }
        }
        "MultiEdit" => input["edits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e.get("path").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect(),
        _ => vec![],
    }
}

pub fn parse_status(_body: &[u8]) -> Option<Context> {
    // statusLine은 Claude Code 2.1.332+에서만 지원.
    // 현재는 transcript 방식(context.rs)을 사용하므로 미구현.
    None
}

pub fn next(_now: Option<Live>, hook: Hook) -> Option<Live> {
    match hook {
        Hook::End => None,
        Hook::Prompt | Hook::ToolUse => Some(Live::Working),
        Hook::Permission { request } => Some(Live::Waiting { request }),
        Hook::Stop => Some(Live::Done),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse() ---

    #[test]
    fn parses_user_prompt_submit() {
        let body = br#"{"hook_event_name":"UserPromptSubmit","session_id":"x"}"#;
        assert_eq!(parse("UserPromptSubmit", body).unwrap(), Hook::Prompt);
    }

    #[test]
    fn parses_pre_tool_use_as_tool_use() {
        let body = br#"{"hook_event_name":"PreToolUse","tool_use_id":"tu_1","tool_name":"Bash"}"#;
        assert_eq!(parse("PreToolUse", body).unwrap(), Hook::ToolUse);
    }

    #[test]
    fn parses_post_tool_use_as_tool_use() {
        let body = br#"{"hook_event_name":"PostToolUse","tool_use_id":"tu_1"}"#;
        assert_eq!(parse("PostToolUse", body).unwrap(), Hook::ToolUse);
    }

    #[test]
    fn parses_permission_request_uses_tool_use_id() {
        let body =
            br#"{"hook_event_name":"PermissionRequest","tool_use_id":"tu_42","tool_name":"Bash"}"#;
        assert_eq!(
            parse("PermissionRequest", body).unwrap(),
            Hook::Permission {
                request: "tu_42".to_string()
            }
        );
    }

    #[test]
    fn parses_permission_request_falls_back_to_tool_name() {
        let body = br#"{"hook_event_name":"PermissionRequest","tool_name":"Read"}"#;
        assert_eq!(
            parse("PermissionRequest", body).unwrap(),
            Hook::Permission {
                request: "Read".to_string()
            }
        );
    }

    #[test]
    fn parses_permission_request_unknown_when_no_id() {
        let body = br#"{"hook_event_name":"PermissionRequest"}"#;
        assert_eq!(
            parse("PermissionRequest", body).unwrap(),
            Hook::Permission {
                request: "unknown".to_string()
            }
        );
    }

    #[test]
    fn parses_stop() {
        let body = br#"{"hook_event_name":"Stop"}"#;
        assert_eq!(parse("Stop", body).unwrap(), Hook::Stop);
    }

    #[test]
    fn parses_session_end_as_end() {
        let body = br#"{"hook_event_name":"SessionEnd"}"#;
        assert_eq!(parse("SessionEnd", body).unwrap(), Hook::End);
    }

    #[test]
    fn parses_subagent_stop_as_end() {
        let body = br#"{"hook_event_name":"SubagentStop"}"#;
        assert_eq!(parse("SubagentStop", body).unwrap(), Hook::End);
    }

    #[test]
    fn rejects_unknown_event() {
        let body = br#"{}"#;
        assert!(parse("Unknown", body).is_err());
    }

    #[test]
    fn rejects_broken_json_for_permission() {
        assert!(parse("PermissionRequest", b"not-json").is_err());
    }

    // --- next() ---

    #[test]
    fn prompt_moves_to_working() {
        assert_eq!(next(None, Hook::Prompt), Some(Live::Working));
    }

    #[test]
    fn tool_use_moves_to_working() {
        assert_eq!(
            next(Some(Live::Working), Hook::ToolUse),
            Some(Live::Working)
        );
    }

    #[test]
    fn permission_moves_to_waiting() {
        assert_eq!(
            next(
                Some(Live::Working),
                Hook::Permission {
                    request: "tu_1".to_string()
                }
            ),
            Some(Live::Waiting {
                request: "tu_1".to_string()
            })
        );
    }

    #[test]
    fn stop_moves_to_done() {
        assert_eq!(next(Some(Live::Working), Hook::Stop), Some(Live::Done));
    }

    #[test]
    fn end_returns_none() {
        assert_eq!(next(Some(Live::Done), Hook::End), None);
    }

    // --- extract_files() ---

    #[test]
    fn extracts_write_path() {
        let body = br#"{"tool_name":"Write","tool_input":{"file_path":"/a/b.rs"}}"#;
        assert_eq!(extract_files("PostToolUse", body), vec!["/a/b.rs"]);
    }

    #[test]
    fn extract_files_bash_no_panic() {
        let body = br#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
        assert!(extract_files("PostToolUse", body).is_empty());
    }

    #[test]
    fn extract_files_non_post_tool_use_empty() {
        let body = br#"{"tool_name":"Write","tool_input":{"file_path":"/a/b.rs"}}"#;
        assert!(extract_files("PreToolUse", body).is_empty());
    }

    #[test]
    fn extracts_edit_path() {
        let body = br#"{"tool_name":"Edit","tool_input":{"file_path":"/src/main.rs"}}"#;
        assert_eq!(extract_files("PostToolUse", body), vec!["/src/main.rs"]);
    }

    #[test]
    fn extracts_multi_edit_paths() {
        let body = br#"{"tool_name":"MultiEdit","tool_input":{"edits":[{"path":"/a.rs"},{"path":"/b.rs"}]}}"#;
        let paths = extract_files("PostToolUse", body);
        assert_eq!(paths, vec!["/a.rs", "/b.rs"]);
    }

    #[test]
    fn pre_tool_use_after_stop_returns_working() {
        // Stop 뒤 PreToolUse가 오면 Working으로 돌아간다 (계획서 명시)
        assert_eq!(next(Some(Live::Done), Hook::ToolUse), Some(Live::Working));
    }

    #[test]
    fn state_transition_full_cycle() {
        let s = next(None, Hook::Prompt);
        assert_eq!(s, Some(Live::Working));
        let s = next(
            s,
            Hook::Permission {
                request: "tu_1".to_string(),
            },
        );
        assert_eq!(
            s,
            Some(Live::Waiting {
                request: "tu_1".to_string()
            })
        );
        let s = next(s, Hook::ToolUse);
        assert_eq!(s, Some(Live::Working));
        let s = next(s, Hook::Stop);
        assert_eq!(s, Some(Live::Done));
        let s = next(s, Hook::End);
        assert_eq!(s, None);
    }
}
