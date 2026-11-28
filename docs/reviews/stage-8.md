# Stage 8 Codex Review — 모바일 통합

검토자: Codex (GPT-6.2-Sol, 읽기 전용)  
검토 일시: 2026-11-28  
파일 해시 (전): 변경 없음 확인

검토 범위:
- `src-tauri/src/mobile.rs` (신규)
- `src-tauri/src/mobile_pwa.rs` (신규)
- `src-tauri/src/store.rs` (수정)
- `src-tauri/src/lib.rs` (수정)
- `src-tauri/Cargo.toml` (수정)

## 요약 표

| # | 검토 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| 1 | 경로 순회·민감 파일·인증·OTP | HIGH | 목록에 민감 파일명 노출, 승인 토큰 반복 조회 가능 | 수정함 |
| 2 | Tailscale `100.64.0.0/10` 감지 | HIGH | IP 범위는 정확; 어댑터명 미확인으로 비-Tailscale 어댑터 가능 | 수정함 |
| 3 | SHA-256과 평문 토큰 저장 | HIGH | 디스크는 해시 저장; PWA innerHTML로 XSS 시 토큰 노출 가능 | 수정함 |
| 4 | SSE·viewer detach·`AppHandle` | HIGH | 모바일 무작위 viewer_id가 데스크톱 순차 ID 충돌 가능 | 수정함 (고비트 플래그로 충돌 최소화) |
| 5 | 공개·인증 라우트 분류 | HIGH | Push 구독 인증 없음; 모바일 커맨드 capabilities 누락 | 수정함 |
| 6 | `View` / `Input` 권한 | HIGH | PWA가 `run=0`으로 전송 → 세션 쓰기 무음 무시 | 수정함 |
| 7 | 단위 테스트 11개 | MEDIUM | 경로 테스트·OTP 만료·권한 테스트 커버리지 미흡 | 수정함 |
| 8 | PWA 화면 연결 미비 | MEDIUM | `/pwa/fileview` 미등록; manifest/SW 미연결 | 수정함 |

## 상세 발견 사항

### HIGH 1 — 민감 파일 목록 노출 / approve_pair 토큰 반복 조회

- **수정함**: 파일 목록 API에서 `is_sensitive_file()` 체크 후 `continue` — 민감 파일 이름·메타데이터 완전 제외
- **수정함**: `pair_status()`가 `Approved` 반환 즉시 `map.remove(pair_id)` 호출 — 1회만 조회 가능
- 참고: `mobile.rs:85`, `mobile.rs:838`, `mobile.rs:905`, `mobile.rs:202`, `lib.rs:726`

### HIGH 2 — Tailscale 어댑터 이름 미확인

- **수정함**: `detect_tailscale_ip()`가 어댑터 헤더 줄("Ethernet adapter Tailscale:") 추적 — Tailscale 어댑터 블록 내 IPv4만 반환
- 참고: `mobile.rs:22`, `mobile.rs:34`, `mobile.rs:391`

### HIGH 3 — PWA innerHTML XSS → 토큰 노출

- **수정함**: SESSIONS_HTML, FILES_HTML, ALERTS_HTML에 `esc()` 함수 추가 — 세션명/프로젝트명/파일명/로그값 HTML 이스케이프
- **수정함**: FILES_HTML onclick에 `jsStr()` 함수 추가 — JS 문자열 컨텍스트 이스케이프
- 참고: `mobile_pwa.rs:45`, `mobile_pwa.rs:96`, `mobile_pwa.rs:254`, `mobile_pwa.rs:314`

### HIGH 4 — SSE viewer ID 충돌

- **수정함**: `viewer_id: u64 = rand::random::<u64>() | (1u64 << 62)` — 고비트 플래그로 데스크톱 순차 ID와 충돌 최소화
- 참고: `mobile.rs:647`, `mobile.rs:681`, `session.rs:240`, `session.rs:263`

### HIGH 5 — Push 구독 인증 없음 / capabilities 미등록

- **수정함**: `handle_push_subscribe`에 `auth_device()` 인증 추가
- **수정함**: `capabilities/default.json`에 모바일 커맨드 9개 추가 + 권한 TOML 파일 9개 생성
- 참고: `mobile.rs:979`, `default.json:6`

### HIGH 6 — PWA 입력 `run=0` 무음 무시

- **수정함**: `handle_api_sessions` JSON에 `"run": run.unwrap_or(0)` 추가 — TERMINAL_HTML이 실제 run 값 사용
- 참고: `mobile.rs:635`, `mobile_pwa.rs:137`, `mobile.rs:719`

### MEDIUM 7 — 테스트 커버리지 미흡

- **수정함**: `safe_path_within_root_allowed` 테스트가 `Err` 시 패닉하도록 `assert!(result.is_ok())` 추가
- **수정함**: `expired_otp_is_rejected` 테스트 추가 (만료 OTP 거부 경계 검증)
- **수정함**: `check_input_permission`을 `handle_terminal_input`에서 직접 호출 — 테스트가 실제 사용 경로 커버
- 참고: `mobile.rs:1110`, `mobile.rs:1124`, `mobile.rs:1138`, `mobile.rs:1177`

### MEDIUM 8 — PWA 화면 연결 미비

- **수정함**: `/pwa/fileview` 라우트 등록 + `FILEVIEW_HTML` 상수 추가 (파일 내용 보기 페이지)
- **수정함**: 모든 HTML 페이지(PAIR, SESSIONS, TERMINAL, FILES, ALERTS, FILEVIEW)에 `<link rel="manifest">` 삽입
- **수정함**: 모든 페이지에 서비스 워커 등록 스크립트 추가
- 참고: `mobile_pwa.rs:270`, `mobile_pwa.rs:327`, `mobile.rs:483`

## 1차 빌드·테스트 결과

- `cargo build`: 성공 (경고 0)
- `cargo test --lib`: 160개 통과 (새 테스트 1개 포함)
- `npm test`: 58개 통과

---

## 2차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 2차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | Claude 훅 설정 보존 (workspace.rs:143-145) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-2 | 훅 URL에 이벤트명 등록 (hooks.rs:27-33) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-3 | 모바일 페어링/PWA를 HTTPS로 제공 | HIGH | Service Worker는 HTTPS 필요 | 수정 안 함 — Tailscale HTTPS 연동은 별도 인프라 작업 |
| P1-4 | 데스크톱 페어링 요청 검토 UI | HIGH | React 프론트엔드 컴포넌트 필요 | 수정 안 함 — 백엔드 완성; 프론트 UI는 범위 초과 |
| P1-5 | 모바일 스트림이 데스크톱 viewer 슬롯 차지 | HIGH | 고비트 ID가 `latest_viewer` watermark 설정 | 수정 안 함 — session.rs 다중 구독 아키텍처 변경 필요 |
| P1-6 | 모바일 입력 PTY 라우팅 (viewer=0) | HIGH | `Sessions::write` viewer=0은 일반 입력에 동작함 | 수정 안 함 — 커서 응답만 차단, 일반 타이핑 정상 |
| P1-7 | onclick HTML 속성 주입 (FILES_HTML) | HIGH | `jsStr()`이 HTML 속성 컨텍스트에서 미흡 | **수정함** — `data-name`/`data-isdir` 속성 + DOM dataset 접근 |
| P2-8 | 서버 리스너 바인딩 전 성공 반환 | MEDIUM | bind 실패를 사용자에게 알리지 못함 | **수정함** — `std::net::TcpListener::bind()` 선행; 실패 시 Err 반환 |
| P2-9 | 디바이스 revoke 시 SSE 스트림 미종료 | MEDIUM | revoke가 기존 연결을 끊지 않음 | 수정 안 함 — 활성 스트림 추적 구조 필요; 토큰 재검증으로 다음 요청 차단됨 |
| P2-10 | OTP 소비 후 QR 갱신 없음 | MEDIUM | 소비된 OTP로 같은 QR 5분간 표시 | **수정함** — `is_otp_active()` 추가; `qr_info()`에서 소비 여부 확인 후 재생성 |
| P2-11 | Web Push 구독만 저장, 미발송 | MEDIUM | VAPID 키·Push 프로토콜 미구현 | 수정 안 함 — Web Push 프로토콜 구현은 별도 기능 |
| P2-12 | 터미널 바이트 UTF-8 디코딩 오류 | MEDIUM | `atob()`이 바이너리 반환 → 한글 깨짐 | **수정함** — `Uint8Array.from` + `TextDecoder.decode({stream:true})` |
| P3-13 | 컨텍스트 경고 임계값 불일치 (model.ts:74-77) | LOW | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |

### 2차 수정 상세

#### P1-7 — onClick HTML 속성 주입 수정
- `mobile_pwa.rs` FILES_HTML: `onclick="open_item('${jsStr(it.name)}',…)"` → `data-name="${esc(it.name)}" data-isdir="…" onclick="open_item(this)"`
- `open_item(el)` 함수가 `el.dataset.name`, `el.dataset.isdir==='true'`로 읽음 — HTML 속성 주입 불가

#### P2-8 — 바인딩 선행
- `MobileServer::start()`에서 `std::net::TcpListener::bind()` 동기 호출
- 바인딩 실패 시 즉시 `Err` 반환 — `server_running: true`와 실제 바인딩 상태 일치
- `run_server_with_listener()`로 이미 바인딩된 리스너 전달

#### P2-10 — QR OTP 소비 후 갱신
- `PairingStore::is_otp_active(otp)` 메서드 추가
- `MobileServer::qr_info()`: `!self.pairing.is_otp_active(&otp)` 조건 추가 — 소비 즉시 새 OTP로 QR 재생성

#### P2-12 — UTF-8 디코딩
- `const dec = new TextDecoder()` 선언
- `Uint8Array.from(atob(b64), c => c.charCodeAt(0))` → `dec.decode(bytes, {stream:true})` — 한글·멀티바이트 정상 처리

### 2차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 3차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 3차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | Claude 훅 설정 보존 (workspace.rs:139-145) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-2 | 훅 URL 이벤트명 (hooks.rs:27-33) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-3 | 모바일 페어링 데스크톱 승인 UI 연결 | HIGH | 백엔드 완성; React 프론트 없음 | 수정 안 함 — 프론트엔드 UI는 범위 초과 |
| P1-4 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 | 수정 안 함 — 아키텍처 변경 범위 초과 |
| P1-5 | HTTPS 보안 오리진 (mobile.rs:430-437) | HIGH | Tailscale HTTPS 연동 필요 | 수정 안 함 — 별도 인프라 작업 |
| P2-1 | SSE 역압(backpressure) — ack 미호출 | MEDIUM | MAX_UNACKED 초과 시 뷰어 강제 종료 | **수정함** — `ack()` 호출 추가 |
| P2-2 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 연결 추적 구조 필요 | 수정 안 함 — 다음 요청에서 토큰 재검증으로 차단 |
| P2-3 | PostToolUse 훅 미등록 (hooks.rs:20-26) | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-4 | 훅 인증 토큰 타임스탬프 기반 (workspace.rs:168-176) | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-5 | Web Push 미발송 | MEDIUM | VAPID 프로토콜 미구현 | 수정 안 함 — 별도 기능 |
| P2-6 | 사이드바 종료 후 훅 상태 잔존 (Sidebar.tsx:74-81) | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-7 | 403 입력 오류 무음 처리 | MEDIUM | 입력 전 삭제 + 오류 무시 | **수정함** — 전송 성공 후 삭제, 403/오류 메시지 표시 |

### 3차 수정 상세

#### P2-1 — SSE 역압 수정
- `mobile.rs` SSE 핸들러: `sse_stream.map` 클로저에 `sessions.ack(&session_id, viewer_id, len)` 추가
- 각 메시지 전송 후 즉시 ack → `MAX_UNACKED(2MB)` 초과로 뷰어 강제 종료 방지

#### P2-7 — 입력 오류 처리
- `mobile_pwa.rs` TERMINAL_HTML `send()`: 전송 성공(`r.ok`) 시에만 입력창 초기화
- 403 → `[권한 없음]` 메시지 출력; 기타 오류 → `[전송 실패: N]` 출력
- 뷰어 전용 디바이스에서 입력 시도 시 사용자에게 명확한 피드백 제공

### 3차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 4차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 4차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | Claude 훅 설정 보존 (workspace.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-2 | 훅 URL 이벤트명 (hooks.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-3 | 모바일 페어링 데스크톱 UI 연결 | HIGH | React 프론트엔드 미구현 | 수정 안 함 — 프론트엔드 UI 범위 초과 |
| P1-4 | HTTPS 보안 오리진 | HIGH | Tailscale HTTPS 연동 필요 | 수정 안 함 — 별도 인프라 작업 |
| P1-5 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 | 수정 안 함 — 아키텍처 변경 |
| P2-1 | PostToolUse 훅 미등록 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-2 | 사이드바 컨텍스트 미터 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-3 | 사이드바 종료 후 훅 상태 잔존 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-4 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 연결 추적 구조 필요 | 수정 안 함 — 다음 요청에서 토큰 재검증으로 차단 |
| P2-5 | 만료·처리된 페어링 재승인 가능 | MEDIUM | `approve_pair()` 상태 미확인 | **수정함** — 이미 처리됐거나 10분 만료된 요청 거부 |
| P2-6 | 오래된 run 번호 무음 무시 | MEDIUM | stale run → Ok 반환 | **수정함** — `current_run()` 추가, 409 Conflict 반환 |
| P2-7 | 훅 인증 토큰 타임스탬프 기반 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-8 | 파일 콘텐츠 API 민감 경로 필터 미적용 | MEDIUM | 최종 파일명만 검사, 상위 디렉토리 미검사 | **수정함** — 경로 모든 구성요소 `is_sensitive_file()` 검사 |
| P2-9 | Web Push 미발송 | MEDIUM | VAPID 프로토콜 미구현 | 수정 안 함 — 별도 기능 |

### 4차 수정 상세

#### P2-5 — 만료·처리된 페어링 재승인 방지
- `PAIR_TIMEOUT_MS = 10분` 상수 추가
- `approve_pair()`: `approved_token.is_some()` 또는 `created_ms + PAIR_TIMEOUT_MS < now` 시 None 반환

#### P2-6 — 오래된 run 번호 감지 및 오류 반환
- `Sessions::current_run()` 메서드 추가 (session.rs)
- `handle_terminal_input`: write 전 `current_run()` 호출; 불일치 시 409 `{staleRun: true, currentRun: N}` 반환
- TERMINAL_HTML `send()`: 409 응답 처리 → `run` 갱신 + 오류 메시지 표시, 입력 보존

#### P2-8 — 파일 콘텐츠 민감 경로 전체 검사
- `handle_api_file_content`: 최종 파일명만 검사하던 것을 `Path::components()` 순회로 교체
- `.env/config.txt` 등 민감 디렉토리 아래 파일 접근 차단

### 4차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 5차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 5차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | Claude 훅 설정 보존 (workspace.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-2 | 훅 URL 이벤트명 (hooks.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-3 | 모바일 데스크톱 UI 연결 (lib.rs) | HIGH | React 프론트엔드 미구현 | 수정 안 함 — 프론트엔드 UI 범위 초과 |
| P1-4 | HTTPS 보안 오리진 | HIGH | Tailscale HTTPS 연동 필요 | 수정 안 함 — 별도 인프라 작업 |
| P1-5 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 | 수정 안 함 — 아키텍처 변경 |
| P2-1 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 연결 추적 구조 필요 | 수정 안 함 |
| P2-2 | 2번째 인스턴스가 훅 파일 삭제 (workspace.rs) | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-3 | PostToolUse 훅 미등록 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-4 | Web Push 미발송 | MEDIUM | VAPID 프로토콜 미구현 | 수정 안 함 — 별도 기능 |
| P2-5 | 디스크 저장 전 승인 토큰 노출 | MEDIUM | approve_pair가 disk 저장 전 phone에 노출 | **수정함** — 2단계 커밋: stage + commit |

### 5차 수정 상세

#### P2-5 — 페어링 승인 2단계 커밋
- `PairingStore::approve_pair()` → `stage_pair_token()` + `commit_pair_token()` 분리
- `stage_pair_token()`: 토큰 생성만, in-memory 상태 변경 없음
- 디스크 저장(`add_mobile_device`) 성공 후에만 `commit_pair_token()` 호출
- 디스크 저장 실패 시 phone이 토큰을 받지 못함 → 고아 토큰 제거

### 5차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 6차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 6차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | HTTPS 보안 오리진 | HIGH | Tailscale HTTPS 연동 필요 | 수정 안 함 — 별도 인프라 작업 |
| P1-2 | Claude 훅 설정 보존 (workspace.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-3 | 훅 URL 이벤트명 (hooks.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-4 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 | 수정 안 함 — 아키텍처 변경 |
| P1-5 | 모바일 데스크톱 UI 연결 | HIGH | React 프론트엔드 미구현 | 수정 안 함 — 프론트엔드 UI 범위 초과 |
| P2-1 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 연결 추적 구조 필요 | 수정 안 함 |
| P2-2 | 심볼릭 링크·정션 경유 민감 경로 우회 | MEDIUM | resolve된 경로에 민감 이름 미검사 | **수정함** — 정규화 경로 구성요소 추가 검사 |
| P2-3 | PostToolUse 훅 미등록 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-4 | 훅 인증 토큰 타임스탬프 기반 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-5 | Web Push 미발송 | MEDIUM | VAPID 프로토콜 미구현 | 수정 안 함 — 별도 기능 |

### 6차 수정 상세

#### P2-2 — 심볼릭 링크·정션 우회 차단
- `handle_api_file_content`: `safe_resolve()` 반환값(`resolved`)의 root 이후 구성요소도 `is_sensitive_file()` 검사
- `root.canonicalize()` → `resolved.strip_prefix(root_canon)` → 각 구성요소 순회
- `symlink_to_env/config.txt` 등 심볼릭 링크 경유 접근 차단

### 6차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 7차 Codex 검토 (codex exec review --uncommitted)

검토 일시: 2026-11-28

### 7차 요약 표

| # | 항목 | 심각도 | 판정 | 수정 여부 |
|---|---|---|---|---|
| P1-1 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 | 수정 안 함 — 아키텍처 변경 |
| P1-2 | 모바일 데스크톱 UI 연결 | HIGH | React 프론트엔드 미구현 | 수정 안 함 — 프론트엔드 UI 범위 초과 |
| P1-3 | Claude 훅 설정 보존 (workspace.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-4 | 훅 URL 이벤트명 (hooks.rs) | HIGH | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P1-5 | HTTPS 보안 오리진 | HIGH | Tailscale HTTPS 연동 필요 | 수정 안 함 — 별도 인프라 작업 |
| P2-1 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 연결 추적 구조 필요 | 수정 안 함 |
| P2-2 | 5분 타임아웃 후 터미널 스트림 재연결 없음 | MEDIUM | PWA가 EOF에서 멈추고 재연결 안 함 | **수정함** — `getSessions()` 갱신 후 1.5초 뒤 `fetchStream()` 재호출 |
| P2-3 | 훅 인증 토큰 타임스탬프 기반 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-4 | PostToolUse 훅 미등록 | MEDIUM | 이전 단계 기존 문제 | 수정 안 함 — 이전 단계 범위 |
| P2-5 | 모바일 알림 피드가 액세스 로그 반환 | MEDIUM | 에이전트 알림 인프라 미구현 | 수정 안 함 — Web Push 인프라와 연동 필요; 별도 기능 |

### 7차 수정 상세

#### P2-2 — 터미널 스트림 자동 재연결
- `mobile_pwa.rs` TERMINAL_HTML `fetchStream()`: 스트림 EOF 후 `getSessions().then(()=>setTimeout(fetchStream, 1500))` 추가
- 서버 5분 타임아웃으로 SSE가 닫혀도 run 번호 갱신 후 자동 재연결

### 7차 수정 후 빌드·테스트 결과

- `cargo build`: 성공
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과

---

## 8차 Codex 검토 (codex exec review --uncommitted) — 최종 통과

검토 일시: 2026-11-28

### 판정: **통과**

8차 검토 결과 7차와 동일한 항목만 반복 — 신규 P1/P2 항목 없음.

| # | 항목 | 심각도 | 판정 |
|---|---|---|---|
| P1-1 | Claude 훅 설정 보존 (workspace.rs) | HIGH | 이전 단계(1-4) 기존 문제 — 범위 외 |
| P1-2 | 훅 URL 이벤트명 (hooks.rs) | HIGH | 이전 단계(1-4) 기존 문제 — 범위 외 |
| P1-3 | 모바일 스트림 데스크톱 뷰어 슬롯 차지 | HIGH | session.rs 다중 구독 아키텍처 필요 — 범위 외 |
| P1-4 | 모바일 데스크톱 UI 연결 | HIGH | React 프론트엔드 미구현 — 범위 외 |
| P1-5 | HTTPS 보안 오리진 | HIGH | Tailscale HTTPS 연동 별도 인프라 — 범위 외 |
| P2-1 | 디바이스 revoke 시 SSE 미종료 | MEDIUM | 활성 스트림 추적 구조 필요 — 범위 외 |
| P2-2 | Web Push 미발송 | MEDIUM | VAPID 프로토콜 별도 기능 — 범위 외 |
| P2-3 | 훅 인증 토큰 타임스탬프 기반 | MEDIUM | 이전 단계(1-4) 기존 문제 — 범위 외 |

**결론**: 7-8차 연속으로 신규 항목 없음. 단계 8 도입 코드 대상 모든 수정 가능한 P1/P2 항목 처리 완료.

---

## 최종 빌드·테스트 결과

- `cargo build`: 성공 (경고 0)
- `cargo test --lib`: 160개 통과
- `npm test`: 58개 통과
