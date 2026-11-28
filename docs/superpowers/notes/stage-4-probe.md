# 4단계 가정 확인 결과 (2026-11-27)

Claude Code 2.1.178에서 `--settings` 훅으로 1회 실행. 필드명과 숫자만 기록.

## 확인된 훅 이벤트 필드

### UserPromptSubmit
```json
{
  "session_id": "<uuid>",
  "transcript_path": "C:\\Users\\...\\<uuid>.jsonl",
  "cwd": "<string>",
  "permission_mode": "default",
  "hook_event_name": "UserPromptSubmit",
  "prompt": "<string>"
}
```

### PreToolUse
```json
{
  "session_id": "<uuid>",
  "transcript_path": "...",
  "cwd": "...",
  "permission_mode": "default",
  "effort": {"level": "high"},
  "hook_event_name": "PreToolUse",
  "tool_name": "Bash",
  "tool_input": {"command": "...", "description": "..."},
  "tool_use_id": "toolu_<id>"
}
```

### PostToolUse
PreToolUse 필드 전부 + :
```json
{
  "tool_response": {
    "stdout": "...",
    "stderr": "...",
    "interrupted": false,
    "isImage": false,
    "noOutputExpected": false
  },
  "duration_ms": 1039
}
```

### Stop
```json
{
  "session_id": "<uuid>",
  "transcript_path": "...",
  "cwd": "...",
  "permission_mode": "default",
  "effort": {"level": "high"},
  "hook_event_name": "Stop",
  "stop_hook_active": false,
  "last_assistant_message": "...",
  "background_tasks": [],
  "session_crons": []
}
```

## PermissionRequest
`-p` 모드에서는 Bash가 자동 승인되어 훅이 실행되지 않음. 인터랙티브 모드에서만 발생.
**가정 A4**: PreToolUse와 동일 구조로 `tool_use_id` + `tool_name` 존재로 가정 (구현에서 검증).

## statusLine 가정 결과 (A2·A3)
**NOT CONFIRMED**: `statusLine` 키를 `--settings` 파일에 넣으면 모든 훅 로딩이 무효화됨 (Claude Code 2.1.178).
원본 개발 환경(2.1.332)에서는 작동할 가능성 있음.

**결정**: transcript 방식(A2 대체안)을 기본으로 구현. 2.1.332 이상에서 statusLine이 작동하면 나중에 추가.

## A5 (1/2/3 + Enter 승인)
`-p` 모드 비해당. 인터랙티브 모드에서 가정이 맞는 것으로 처리 (Claude Code의 표준 동작).

## 훅 JSON 필드 요약 (구현에 쓰는 것)
| 필드 | 이벤트 | 용도 |
|------|--------|------|
| `session_id` | 전체 | URL 토큰 대응 |
| `transcript_path` | 전체 | 컨텍스트 계산 |
| `hook_event_name` | 전체 | 이벤트 분류 |
| `tool_use_id` | PreToolUse / PermissionRequest | 오래된 클릭 방지 |
| `tool_name` | PreToolUse / PermissionRequest | 승인 UI 표시 |
