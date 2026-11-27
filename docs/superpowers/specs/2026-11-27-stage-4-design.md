# 4단계 설계: 훅 상태 · 컨텍스트 · 승인 · 알림

작성 2026-11-27. `/grill-me`로 사용자와 3라운드에 걸쳐 정한 결정을 정리한 문서다. 명세는 `docs/spec.md` 3·4·5절, 시안은 `docs/mockups/Main.dc.html`·`Layout3.dc.html`.
구현은 이 문서와 `docs/superpowers/plans/2026-11-27-stage-4.md`를 보고 집 노트북에서 한다(아직 코드 없음).

## 1. 목표
Claude Code 세션의 **상태(작업 중·승인 대기·완료)**와 **컨텍스트 사용률**을 사이드바와 패널 머리글에 보이고, 승인 요청에는 **단추 줄**로 답하며, 창이 활성이 아닐 때 **Windows 알림**을 보낸다. 사용자 파일은 건드리지 않는다.

## 2. 확정한 결정

| # | 결정 | 이유·한계 |
|---|---|---|
| 1 | Claude로 시작할 때 `--settings <세션별 임시 파일>`로 훅을 주입한다. `~/.claude`와 프로젝트의 `.claude`는 수정하지 않는다 | `claude --help`(2.1.332)에 `--settings <file-or-json>`이 있다. 사용자 설정을 고치는 방식(백업 필요)은 만들지 않는다 |
| 2 | 명령 줄에는 **파일 경로만** 넘긴다. 세션 id와 무작위 토큰은 훅 URL(`/event/<세션>/<토큰>`)에 넣는다. 세션을 지울 때 파일도 지운다 | 셸마다 따옴표 규칙이 달라 JSON을 인수로 넘기지 않는다(기존 `agent.rs`의 인수 처리를 재사용) |
| 3 | 훅 명령은 `curl.exe -s -m 3 -X POST --data-binary @- <URL>`이고 항상 정상 종료한다. 훅 `timeout`은 5초 | Lmux가 꺼져 있거나 느려도 Claude가 멈추지 않게 한다(공식 문서: 훅 실패·타임아웃은 대부분 비차단) |
| 4 | **WSL도 지원**한다. 설정 파일은 Windows 데이터 폴더에 두고 `wslpath`로 바꾼 경로를 넘긴다. WSL의 훅은 Windows `curl.exe`(안 되면 Linux `curl`)로 `127.0.0.1`에 보낸다. `transcript_path`는 `\\wsl$\<배포판>\…`로 읽는다. NAT 모드라 안 닿으면 그 세션만 "정보 없음"과 안내 | 이 PC는 `networkingMode=mirrored`라 `127.0.0.1`이 닿는다. 서버를 `0.0.0.0`에 열지 않는다 |
| 5 | 서버는 `127.0.0.1` 전용 임의 포트, 토큰 불일치·본문 1MB 초과·세션 없음은 버린다. 토큰은 저장하지 않는다 | 같은 PC의 다른 프로그램이 가짜 이벤트를 보내는 것을 막는다 |
| 6 | 상태: 작업 중 = `UserPromptSubmit`·`PreToolUse`, **승인 대기 = `PermissionRequest`**, 완료 = `Stop`, 종료 = `SessionEnd`. `Notification`은 쓰지 않는다 | `Notification(permission_prompt)`는 약 6초 늦고, `idle_prompt`는 60초 뒤라 입력 대기와 승인 대기를 섞는다 |
| 7 | 컨텍스트: `--settings`에 `statusLine`도 넣어 Claude가 주는 `context_window_size`와 `used_percentage`를 그대로 쓴다. 없으면 transcript의 마지막 응답 usage(입력 + cache_creation + cache_read) ÷ 200K를 **"추정"** 표시로 쓴다. 50% 주황, 90% 빨강 | 지시문의 70%에서 사용자가 50%로 바꿨다. statusLine은 같은 키라 Lmux 세션에서는 사용자 statusLine이 가려진다(사용자는 statusLine을 쓰지 않음, 2026-11-27 확인) |
| 8 | 승인 단추 줄은 `PermissionRequest`일 때만 보이고, 누르면 `1`/`2`/`3` + Enter를 그 터미널에 입력한다. 백엔드가 **요청 번호와 현재 상태를 다시 확인**하고, 이미 바뀌었으면 입력하지 않는다 | 사용자가 직접 답한 뒤 엉뚱한 Enter가 들어가는 것을 막는 것이 핵심 |
| 9 | 알림: 공식 `tauri-plugin-notification`. Lmux 창이 활성이 아닐 때만, 승인 대기와 완료에 보내고 **세션 이름과 상태만** 담는다 | 프롬프트·파일명은 담지 않는다(8단계 Web Push 기본값과 같은 원칙) |
| 10 | 4단계에 넣지 않는 것: 같은 파일 충돌 경고(6단계), Codex 훅, 알림 클릭으로 창 전환, 알림 설정, 상태 저장 | 훅이 없는 에이전트는 "정보 없음"(상태는 지금처럼 "실행 중"만) |
| 11 | 구현의 **첫 작업은 실제 Claude 한 번 실행**해 훅·statusLine 입력을 덤프하고 아래 가정을 확인한다(사용자 확인 후, 임시 폴더) | 문서로 확정되지 않은 가정이 있다 |

## 3. 확인되지 않은 가정 (계획서 Task 0에서 검증)
- A1: `--settings`의 훅이 사용자 훅과 **합쳐져** 함께 실행된다(공식 문서에 명시 문장 없음, 추측).
- A2: `--settings`로 넘긴 `statusLine`이 동작한다(추측).
- A3: 1M 컨텍스트 모델의 `context_window_size`가 실제 1,000,000으로 온다(문서에 없음).
- A4: `PermissionRequest` 입력에 승인 요청을 구분하는 값(요청 번호 또는 도구 이름·입력)이 있다. 단추의 오래된 클릭 방지에 쓴다.
- A5: 승인 화면에서 `1`/`2`/`3` + Enter가 실제 선택으로 동작한다.
- A6: WSL(미러 모드)에서 `curl.exe`와 `wslpath` 경로가 동작한다(WSL에는 `claude`가 없어 가짜로만 확인 가능).
- 틀리면 대체안: A2·A3이 틀리면 transcript 방식만 쓴다. A4가 틀리면 도착 시각과 상태 변화로 오래된 클릭을 거른다.

## 4. 구성

```
Claude ──훅(curl.exe)──▶ [127.0.0.1:포트] hook_server ──▶ events(상태 계산)
statusLine ──────────────▶                                   │
                                      Tauri 이벤트 session-state ──▶ 화면(App → 사이드바·머리글·단추 줄)
                                                         └──▶ notification(창이 활성이 아닐 때)
화면 단추 ──session_approve(id, run, request)──▶ 상태 재확인 ──▶ session_write("1\r")
```

- `src-tauri/src/hooks.rs`(신규): 세션별 설정 파일 생성(훅·statusLine·URL), WSL 경로 변환, 정리.
- `src-tauri/src/events.rs`(신규): 훅 본문 해석과 상태 계산(순수 함수, 테스트 중심).
- `src-tauri/src/hook_server.rs`(신규): 127.0.0.1 서버(토큰·크기 제한·세션 확인).
- `src-tauri/src/context.rs`(신규): transcript 끝에서부터 읽어 마지막 응답 usage 계산(서브에이전트 항목 `isSidechain` 제외).
- 기존 수정: `agent.rs`(Claude일 때 `--settings` 인수), `workspace.rs`/`lib.rs`(설정 파일 만들기·정리, 새 명령), `session.rs`(새 실행에서 상태 초기화), `build.rs`·`capabilities/default.json`(명령 추가 규칙).
- 프론트: `model.ts`(상태 확장), `SessionPanel.tsx`(상태·링·단추 줄), `Sidebar.tsx`(점·%·미니 막대), `api.ts`, `styles.css`.

### 상태 모델
`idle`(시작 전) → `running`(셸만 있음, 훅 정보 없음) → 훅이 오면 `working` / `waiting`(요청 번호 포함) / `done`. 세션이 다시 시작되면(실행 번호가 바뀌면) 훅 상태와 컨텍스트를 지운다. 늦게 도착한 이전 실행의 이벤트는 버린다(URL의 토큰은 실행마다 새로 만든다).

## 4-1. 화면
- 사이드바: 상태 점(작업 중 파랑, 승인 대기 주황, 완료 초록), 컨텍스트 %와 미니 막대. 훅이 없으면 "정보 없음"이 아니라 지금처럼 상태 글자만 보인다.
- 패널 머리글: 상태 글자, 컨텍스트 링 + `168K/200K` 숫자. 추정이면 "추정" 표시. 승인 대기 패널은 주황 테두리, 아래에 `1 · 예`, `2 · 세션 동안 허용`, `3 · 아니요` 단추 줄.
- 색: `src/styles.css`의 `--working`·`--waiting`·`--done`, 컨텍스트는 50% 이상 `--waiting`, 90% 이상 `--error`.

## 5. 보안·개인정보
- 서버는 `127.0.0.1`만, 세션별 토큰, 1MB 제한. 알림에는 내용을 담지 않는다.
- 설정 파일에는 토큰이 들어가므로 앱 데이터 폴더(사용자 전용)에 만들고 세션 삭제·앱 종료 때 지운다.
- 훅 명령에 외부 입력(프로젝트 경로, 프롬프트)을 넣지 않는다. URL과 상수만 넣는다.
- 폴더 신뢰 화면 통과 전에는 훅이 실행되지 않는다(공식 문서). 이 경우 상태는 "실행 중"으로만 보인다.

## 6. 알려진 한계
- Lmux 세션 안에서 사용자 statusLine이 가려진다.
- Codex·셸만 세션은 상태·컨텍스트가 없다.
- NAT 모드 WSL은 4단계에서 지원하지 않는다.
- 앱이 꺼진 동안의 이벤트는 사라지고, 앱을 켜도 이전 상태를 복원하지 않는다(세션 복원은 7단계).
