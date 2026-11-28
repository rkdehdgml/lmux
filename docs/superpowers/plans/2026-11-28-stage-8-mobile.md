# Stage 8 Mobile Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tailscale 사설망 전용 모바일 접속 — HTTPS 서버, QR 페어링, 기기 토큰 관리, PWA(세션/터미널/파일/알림), Web Push 알림, 데스크톱 "모바일 연동" 설정 화면.

**Architecture:** Rust 백엔드에 `MobileServer` 스레드를 추가 — Tailscale IP(100.x)에만 바인딩, tailscale cert로 TLS. 클라이언트는 five embedded HTML 페이지로 구성된 PWA. 기존 `Sessions`/`Workspace`를 `Arc`로 공유하고 Tauri 이벤트 채널로 데스크톱 UI에 상태 전달.

**Tech Stack:** Rust (tiny_http + ssl-rustls, sha2, qrcode, web-push), React/TypeScript (SetMobile.tsx), HTML/JS PWA (src-tauri/mobile-pwa/*.html embedded via include_str!)

**Spec:** docs/spec.md §6 단계 8

## Global Constraints

- Tailscale IP만 바인딩 — 0.0.0.0/127.0.0.1 금지. 100.x.x.x 미검출 시 서버 미시작, 오류 반환.
- .env, *.key, *.pem, *.p12, *secret*, *credential*, id_rsa, id_ed25519 패턴 파일 숨김.
- 경로 탈출 방지: canonicalize 후 프로젝트 루트 startsWith 검사, 실패 시 403.
- 새 셸 열기 금지: input 권한 기기도 기존 세션에만 입력 가능.
- 기기 토큰: SHA-256 해시만 store에 저장, 평문 저장 금지.
- QR 한 번만 사용 가능: 사용된 토큰 즉시 무효화.
- cargo check / cargo test / npx vitest run 각 태스크 완료 시 통과 필수.

## Review Focus

- 경로 탈출: `../` 반복, 심볼릭 링크, URL 인코딩 우회 — 프로젝트 루트 밖 접근을 403 반환하는지.
- 미인증 접근: 기기 토큰 없이 /api/* 호출 시 401 반환하는지.
- 입력 권한 미체크: view 권한 기기가 POST /api/terminal/{id}/input 성공하지 않는지.
- OTP 재사용: 같은 QR 토큰으로 두 번 pair 시도 시 두 번째는 403 반환하는지.
- VAPID 미설정: Web Push 미구성 상태에서 push 호출 시 패닉하지 않고 graceful 무시하는지.

---

### Task 1: Tailscale 감지 + HTTP 서버 기반 + TLS

**Files:**
- Create: `src-tauri/src/mobile.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/mobile.rs` (내부 #[cfg(test)])

**Interfaces:**
- Produces:
  - `pub struct MobileServer` — `start(workspace, sessions) -> MobileServer`, `port() -> u16`, `tailscale_ip() -> Option<String>`, `hostname() -> Option<String>`
  - Tauri 커맨드 `mobile_status() -> Result<MobileStatus, String>` (MobileStatus: ip, port, tls_ok, server_running)
  - `fn detect_tailscale_ip() -> Option<std::net::IpAddr>` (pub(crate))
  - `fn tailscale_hostname() -> Option<String>` (pub(crate))

- [ ] **Step 1: 테스트 작성 — Tailscale IP 감지**

```rust
// src-tauri/src/mobile.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tailscale_ip_returns_100x_or_none() {
        // 실제 환경과 무관하게 None 또는 100.x.x.x 반환
        let result = detect_tailscale_ip();
        if let Some(ip) = result {
            let first_octet = match ip {
                std::net::IpAddr::V4(v4) => v4.octets()[0],
                _ => panic!("IPv6 unexpected"),
            };
            assert_eq!(first_octet, 100, "Tailscale IP must start with 100");
        }
    }

    #[test]
    fn filter_tailscale_ip_selects_correct_range() {
        use std::net::Ipv4Addr;
        assert!(is_tailscale_ip(std::net::IpAddr::V4(Ipv4Addr::new(100, 84, 12, 7))));
        assert!(!is_tailscale_ip(std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_tailscale_ip(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p lmux_lib detect_tailscale`
Expected: FAIL with "cannot find function"

- [ ] **Step 3: Cargo.toml에 의존성 추가**

```toml
tiny_http = { version = "0.12", features = ["ssl-rustls"] }
sha2 = "0.10"
hex = "0.4"
qrcode = "0.14"
base64 = { version = "0.22", features = ["std"] }
web-push = "0.10"
rand = "0.8"
```

- [ ] **Step 4: `detect_tailscale_ip` 구현 (`src-tauri/src/mobile.rs`)**

`ipconfig /all` stdout을 파싱해 "100."으로 시작하는 IPv4 주소 반환.
`tailscale_hostname()`은 `tailscale status --json` stdout을 파싱해 `Self.DNSName` 반환 (실패 시 None).

```rust
pub(crate) fn is_tailscale_ip(addr: std::net::IpAddr) -> bool {
    match addr {
        std::net::IpAddr::V4(v4) => v4.octets()[0] == 100,
        _ => false,
    }
}

pub(crate) fn detect_tailscale_ip() -> Option<std::net::IpAddr> {
    // ipconfig /all 파싱
    let out = std::process::Command::new("ipconfig").arg("/all").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.contains("IPv4") {
            if let Some(ip_str) = line.split(':').last().map(|s| s.trim()) {
                if let Ok(addr) = ip_str.parse::<std::net::IpAddr>() {
                    if is_tailscale_ip(addr) { return Some(addr); }
                }
            }
        }
    }
    None
}
```

- [ ] **Step 5: `MobileServer` 구조체 + 시작 로직**

```rust
pub struct MobileServer {
    ip: std::net::IpAddr,
    hostname: Option<String>,
    tls_ok: bool,
}

impl MobileServer {
    pub fn start(
        workspace: std::sync::Arc<std::sync::Mutex<crate::workspace::Workspace>>,
        sessions: std::sync::Arc<crate::session::Sessions>,
    ) -> Result<Self, String> { ... }
    pub fn tailscale_ip(&self) -> String { self.ip.to_string() }
    pub fn hostname(&self) -> Option<String> { self.hostname.clone() }
    pub fn tls_ok(&self) -> bool { self.tls_ok }
}
```

TLS: `tailscale cert <hostname>` 실행 → `<hostname>.crt`, `<hostname>.key` 파일 생성 → rustls로 로드. 실패 시 HTTP로만 시작(tls_ok = false).

- [ ] **Step 6: `lib.rs`에 모듈 추가 + `mobile_status` 커맨드**

```rust
// lib.rs
mod mobile;
// setup()에서:
let mob = mobile::MobileServer::start(...).ok();
app.manage(Mutex::new(mob));
```

```rust
#[tauri::command(async)]
fn mobile_status(mob: State<'_, Mutex<Option<mobile::MobileServer>>>) -> MobileStatusPayload { ... }
```

- [ ] **Step 7: 빌드 통과 확인**

Run: `cargo check`
Expected: 오류 없음

- [ ] **Step 8: 테스트 통과 확인**

Run: `cargo test -p lmux_lib mobile`
Expected: `detect_tailscale_ip_returns_100x_or_none` PASS, `filter_tailscale_ip_selects_correct_range` PASS

- [ ] **Step 9: Commit**

```
git add src-tauri/src/mobile.rs src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat: add mobile server foundation with Tailscale IP detection"
```

---

### Task 2: 페어링 흐름 + 기기 토큰 저장

**Files:**
- Modify: `src-tauri/src/mobile.rs`
- Modify: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/mobile.rs` (추가 테스트)

**Interfaces:**
- Consumes: `MobileServer` (Task 1), `store::Store` (기존)
- Produces:
  - `store::MobileDevice { id, name, token_hash, permission, registered_at_ms, last_seen_ms }`
  - `store::MobilePermission` enum (View / Input)
  - `store::State` 에 `mobile_devices: Vec<MobileDevice>` 추가
  - HTTP 엔드포인트: `POST /pair/begin` → `{pair_id, code_6}`, `POST /pair/confirm` → `{device_token}` (승인 후)
  - Tauri 커맨드: `mobile_qr_info() -> QrInfo`, `mobile_approve_pairing(pair_id: String) -> DeviceInfo`, `mobile_revoke_device(device_id: String)`, `mobile_set_permission(device_id: String, perm: String)`
  - Tauri 이벤트: `"mobile-pair-request" { pair_id, code_6 }` — 프론트가 승인 UI 표시용

- [ ] **Step 1: 테스트 작성 — 토큰 해싱 + OTP 재사용 차단**

```rust
#[test]
fn token_hash_is_deterministic_and_not_plaintext() {
    let token = "test-device-token-abc123";
    let hash1 = hash_token(token);
    let hash2 = hash_token(token);
    assert_eq!(hash1, hash2);
    assert_ne!(hash1, token); // 평문이 아님
    assert_eq!(hash1.len(), 64); // SHA-256 hex
}

#[test]
fn otp_is_consumed_after_first_use() {
    let store = PairingStore::new();
    let otp = store.generate_otp();
    assert!(store.consume_otp(&otp));
    assert!(!store.consume_otp(&otp), "재사용 불가");
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p lmux_lib token_hash`
Expected: FAIL with "cannot find function"

- [ ] **Step 3: `hash_token` + `PairingStore` 구현**

```rust
pub(crate) fn hash_token(token: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

pub(crate) struct PairingStore {
    // pending OTP: token -> (code_6, expires_ms)
    otps: Mutex<HashMap<String, (String, u64)>>,
    // pending pairs: pair_id -> (code_6, approved_device_token_opt)
    pairs: Mutex<HashMap<String, PendingPair>>,
}
```

- [ ] **Step 4: `store.rs`에 `MobileDevice` + `MobilePermission` 추가**

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MobilePermission { View, Input }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MobileDevice {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub permission: MobilePermission,
    pub registered_at_ms: u64,
    pub last_seen_ms: u64,
}
```

`State` 구조체에 `pub mobile_devices: Vec<MobileDevice>` 추가. 기본값 빈 Vec.

- [ ] **Step 5: `GET /pair?token=OTP` → pairing 페이지 (HTML), `POST /pair/begin` 구현**

GET /pair?token=OTP → HTML 페이지 (mobile-pwa의 pair.html, Task 5에서 완성)
POST /pair/begin JSON `{token: OTP, device_name: String}` → validate OTP, generate 6-digit code, emit Tauri event "mobile-pair-request"

- [ ] **Step 6: `mobile_approve_pairing` Tauri 커맨드**

```rust
#[tauri::command(async)]
fn mobile_approve_pairing(
    app: AppHandle,
    mob: State<'_, Mutex<Option<mobile::MobileServer>>>,
    workspace: Shared<'_>,
    pair_id: String,
) -> Result<MobileDevicePayload, String>
```

pair_id 검증 → 새 device_token 생성 → hash → store에 MobileDevice 추가 → pair 상태를 approved로 변경 (모바일이 폴링 중인 pair 채널에서 토큰 반환).

- [ ] **Step 7: `POST /pair/status/{pair_id}` 폴링 엔드포인트**

```
응답: { status: "pending"|"approved"|"rejected", device_token?: string }
```

모바일이 2초마다 폴링, approved 되면 device_token 수신.

- [ ] **Step 8: `mobile_qr_info` 커맨드 — OTP + QR SVG 생성**

```rust
#[tauri::command(async)]
fn mobile_qr_info(mob: State<'_, ...>) -> Result<QrInfoPayload, String>
// QrInfoPayload: { svg: String, url: String, expires_in_secs: u64 }
```

`qrcode::QrCode::new(url)` → SVG string. OTP 5분 유효.

- [ ] **Step 9: 빌드 + 테스트 통과**

Run: `cargo test -p lmux_lib`
Expected: 150+ 테스트 통과

- [ ] **Step 10: Commit**

```
git add src-tauri/src/mobile.rs src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat: mobile pairing flow — OTP, 6-digit confirm, device token storage"
```

---

### Task 3: 세션 목록 + 터미널 스트리밍 API

**Files:**
- Modify: `src-tauri/src/mobile.rs`
- Test: `src-tauri/src/mobile.rs`

**Interfaces:**
- Consumes: `Sessions` (기존), `HookStates` (기존), `Workspace` (기존)
- Produces:
  - `GET /api/sessions` (인증 필요) → `[{id, name, projectName, status, contextPct}]`
  - `GET /api/terminal/{id}/stream` (SSE, 인증 필요) → 터미널 출력 스트림
  - `POST /api/terminal/{id}/input` (인증 필요, input 권한 체크) → 입력 전달
  - `GET /api/sessions/{id}/approve` (input 권한, 훅 대기 중일 때) → 승인 선택

- [ ] **Step 1: 테스트 작성 — 인증 미들웨어 + 입력 권한 체크**

```rust
#[test]
fn auth_rejects_missing_token() {
    let result = verify_device_token("", &[]);
    assert!(result.is_err());
}

#[test]
fn view_permission_cannot_send_input() {
    let device = make_test_device(MobilePermission::View);
    let result = check_input_permission(&device);
    assert!(result.is_err(), "view 권한으로 input 불가");
}

#[test]
fn input_permission_can_send_input() {
    let device = make_test_device(MobilePermission::Input);
    let result = check_input_permission(&device);
    assert!(result.is_ok());
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p lmux_lib auth_rejects`
Expected: FAIL

- [ ] **Step 3: 인증 미들웨어 구현**

Authorization 헤더: `Bearer <device_token>`. device_token을 SHA-256 해시 → store의 MobileDevice와 비교.

```rust
fn authenticate<'a>(
    req: &tiny_http::Request,
    devices: &'a [store::MobileDevice],
) -> Result<&'a store::MobileDevice, tiny_http::Response<std::io::Cursor<Vec<u8>>>> { ... }
```

- [ ] **Step 4: `GET /api/sessions` 구현**

Sessions + Workspace 잠금 → 세션 목록 + hook 상태 조합 → JSON 직렬화.

- [ ] **Step 5: `GET /api/terminal/{id}/stream` SSE 구현**

```
Content-Type: text/event-stream
Transfer-Encoding: chunked
```

1. Ring buffer 스냅샷 먼저 전송 (data: base64\n\n)
2. 채널로 새 출력 받아 전송
3. 연결 끊기면 채널 해제

Sessions의 `attach` 메서드 사용 (viewer=모바일용 특수 ID).

- [ ] **Step 6: `POST /api/terminal/{id}/input` 구현**

Input 권한 체크 → `sessions.write()` 호출.

- [ ] **Step 7: 테스트 + 빌드 통과**

Run: `cargo test -p lmux_lib`
Expected: 모든 테스트 통과

- [ ] **Step 8: Commit**

```
git add src-tauri/src/mobile.rs
git commit -m "feat: mobile session list and terminal streaming API"
```

---

### Task 4: 파일 탐색 API + 경로 탈출 방지

**Files:**
- Modify: `src-tauri/src/mobile.rs`
- Test: `src-tauri/src/mobile.rs`

**Interfaces:**
- Consumes: `Workspace` (프로젝트 목록), 파일시스템
- Produces:
  - `GET /api/files` → `[{id, name, path, kind}]` (등록된 프로젝트 목록)
  - `GET /api/files/{proj_id}?path=subdir` → 디렉토리 내용 목록 `[{name, is_dir, modified_ms, size, hidden}]`
  - `GET /api/files/{proj_id}/content?path=...` → 파일 내용 (최대 100KB)
  - `GET /api/files/{proj_id}/diff?path=...` → 파일 diff (SVN/Git)

- [ ] **Step 1: 테스트 작성 — 경로 탈출 방지 + 민감 파일 숨김**

```rust
#[test]
fn path_traversal_blocked() {
    let root = std::path::PathBuf::from("C:\\projects\\my-app");
    let malicious = "../../Windows/System32/config";
    let result = safe_resolve(&root, malicious);
    assert!(result.is_err(), "경로 탈출은 거부해야 함");
}

#[test]
fn safe_path_allowed() {
    let root = std::path::PathBuf::from("C:\\projects\\my-app");
    let result = safe_resolve(&root, "src/main/java/App.java");
    assert!(result.is_ok());
}

#[test]
fn env_file_is_hidden() {
    assert!(is_sensitive_file(".env"));
    assert!(is_sensitive_file("prod.key"));
    assert!(is_sensitive_file("server.pem"));
    assert!(is_sensitive_file("id_rsa"));
    assert!(!is_sensitive_file("application.yml"));
    assert!(!is_sensitive_file("README.md"));
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p lmux_lib path_traversal`
Expected: FAIL

- [ ] **Step 3: `safe_resolve` + `is_sensitive_file` 구현**

```rust
pub(crate) fn safe_resolve(
    root: &std::path::Path,
    rel: &str,
) -> Result<std::path::PathBuf, &'static str> {
    let joined = root.join(rel);
    let canonical = joined.canonicalize().map_err(|_| "경로 없음")?;
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err("경로 탈출")
    }
}

pub(crate) fn is_sensitive_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(lower.as_str(), ".env" | "id_rsa" | "id_ed25519")
        || lower.ends_with(".key")
        || lower.ends_with(".pem")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("password")
}
```

- [ ] **Step 4: `GET /api/files/{proj_id}?path=` 구현**

`safe_resolve(project_path, path)` → `std::fs::read_dir` → 각 항목의 is_dir, name, size, modified. `is_sensitive_file(name)` → `hidden: true` (이름은 숨기지 않고 내용만 접근 차단).

- [ ] **Step 5: `GET /api/files/{proj_id}/content?path=` 구현**

`safe_resolve` → `is_sensitive_file` → 403. 파일 크기 100KB 초과 시 "파일이 너무 커요" 오류. UTF-8 텍스트 반환.

- [ ] **Step 6: 테스트 + 빌드**

Run: `cargo test -p lmux_lib`
Expected: 모두 통과

- [ ] **Step 7: Commit**

```
git add src-tauri/src/mobile.rs
git commit -m "feat: mobile file browser API with path traversal prevention"
```

---

### Task 5: 모바일 PWA HTML 페이지 (임베디드)

**Files:**
- Create: `src-tauri/mobile-pwa/pair.html`
- Create: `src-tauri/mobile-pwa/sessions.html`
- Create: `src-tauri/mobile-pwa/terminal.html`
- Create: `src-tauri/mobile-pwa/files.html`
- Create: `src-tauri/mobile-pwa/alerts.html`
- Create: `src-tauri/mobile-pwa/manifest.json`
- Create: `src-tauri/mobile-pwa/sw.js`
- Create: `src-tauri/src/mobile_pwa.rs`
- Modify: `src-tauri/src/mobile.rs`

**Interfaces:**
- Consumes: 모든 /api/* 엔드포인트 (Tasks 1-4)
- Produces:
  - HTTP 라우트: `GET /pwa/pair`, `/pwa/sessions`, `/pwa/terminal/{id}`, `/pwa/files`, `/pwa/alerts`
  - `GET /pwa/manifest.json`, `GET /pwa/sw.js`
  - `GET /` → redirect to `/pwa/pair` (미인증) 또는 `/pwa/sessions` (인증됨)

- [ ] **Step 1: `mobile_pwa.rs` 모듈 뼈대**

```rust
// src-tauri/src/mobile_pwa.rs
pub(crate) const PAIR_HTML: &str = include_str!("../mobile-pwa/pair.html");
pub(crate) const SESSIONS_HTML: &str = include_str!("../mobile-pwa/sessions.html");
pub(crate) const TERMINAL_HTML: &str = include_str!("../mobile-pwa/terminal.html");
pub(crate) const FILES_HTML: &str = include_str!("../mobile-pwa/files.html");
pub(crate) const ALERTS_HTML: &str = include_str!("../mobile-pwa/alerts.html");
pub(crate) const MANIFEST_JSON: &str = include_str!("../mobile-pwa/manifest.json");
pub(crate) const SW_JS: &str = include_str!("../mobile-pwa/sw.js");
```

- [ ] **Step 2: `pair.html` — 페어링 화면 (MPair.dc.html 기반)**

MPair.dc.html 색/레이아웃을 HTML/CSS로 구현:
- 상태 단계 표시 (Tailscale 연결됨 → QR 스캔됨 → PC 승인 대기 → 알림)
- 6자리 코드 표시 (`<div class="code">482 913</div>`)
- `/pair/begin` POST 후 code 받아 표시, `/pair/status/{pair_id}` 폴링
- approved 되면 localStorage에 device_token 저장 후 /pwa/sessions로 리다이렉트
- 색: background #0F1115, 강조 #2B5CD6, 텍스트 #E6E8EE

- [ ] **Step 3: `sessions.html` — 세션 목록 (MSessions.dc.html 기반)**

- GET /api/sessions 호출, 프로젝트별 그룹
- 세션별: 이름, 상태 점(작업 중 #6EA8FF / 입력 대기 #F2B54A / 완료 #4CC79A), 컨텍스트 % 링
- 탭 클릭 시 /pwa/terminal/{id} 이동
- 하단 탭 바: 세션/파일/알림

- [ ] **Step 4: `terminal.html` — 원격 터미널 (MTerminal.dc.html 기반)**

- EventSource("/api/terminal/{id}/stream")로 SSE 연결
- 출력: `<pre>` 요소에 base64 디코딩 후 추가
- 입력 대기 중이면 승인 버튼 행(예/세션 동안 허용/아니요) 표시
- 특수 키 툴바: Esc/Tab/Ctrl+C
- 텍스트 입력 + 전송 버튼 (input 권한 없는 기기: disabled)
- 컨텍스트 프로그레스 바 (orange ≥ 70%, red ≥ 90%)

- [ ] **Step 5: `files.html` + `alerts.html`**

`files.html`: 프로젝트 셀렉트, "세션이 고친 파일" 필터, 디렉토리 트리. MFiles.dc.html 기반.
`alerts.html`: GET /api/alerts 폴링 (5초). 알림 항목별 아이콘·텍스트·시간. MAlerts.dc.html 기반.

- [ ] **Step 6: `manifest.json` + `sw.js`**

```json
// manifest.json
{ "name": "Lmux", "short_name": "Lmux", "start_url": "/pwa/sessions",
  "display": "standalone", "background_color": "#0F1115", "theme_color": "#2B5CD6" }
```

```javascript
// sw.js — push 수신 시 알림 표시
self.addEventListener('push', e => {
  const data = e.data ? e.data.json() : {};
  e.waitUntil(self.registration.showNotification(
    data.title || 'Lmux', { body: data.body || '', icon: '/pwa/icon.png' }
  ));
});
```

- [ ] **Step 7: mobile.rs에 PWA 라우트 추가**

```rust
"/pwa/sessions" => respond_html(mobile_pwa::SESSIONS_HTML),
"/pwa/terminal/*" => respond_html(mobile_pwa::TERMINAL_HTML),
// 등등
```

- [ ] **Step 8: 빌드 + 테스트**

Run: `cargo check && cargo test -p lmux_lib`
Expected: 오류 없음

- [ ] **Step 9: Commit**

```
git add src-tauri/src/mobile_pwa.rs src-tauri/mobile-pwa/ src-tauri/src/mobile.rs
git commit -m "feat: mobile PWA pages — sessions, terminal, files, alerts, pair"
```

---

### Task 6: 접속 기록 + Web Push

**Files:**
- Modify: `src-tauri/src/mobile.rs`
- Test: `src-tauri/src/mobile.rs`

**Interfaces:**
- Produces:
  - In-memory circular buffer (최대 100 항목): `AccessLog { timestamp_ms, device_name, action, target }`
  - `GET /api/log` → 최근 접속 기록 JSON
  - `GET /api/alerts` → 푸시 받은 알림 목록 (최근 50개)
  - `POST /api/push/subscribe` → push subscription 등록 `{endpoint, keys: {p256dh, auth}}`
  - VAPID 키 쌍 생성 (앱 시작 시 1회, 메모리에 보관)
  - `mobile_access_log()` Tauri 커맨드 → 접속 기록 Vec

- [ ] **Step 1: 테스트 작성 — 접속 기록 순환 버퍼**

```rust
#[test]
fn access_log_caps_at_100() {
    let log = AccessLogger::new(100);
    for i in 0..150 {
        log.append(AccessEntry { timestamp_ms: i, device_name: "test".into(),
            action: "connect".into(), target: "—".into() });
    }
    let entries = log.recent(200);
    assert_eq!(entries.len(), 100);
    assert_eq!(entries[0].timestamp_ms, 50); // 오래된 것 제거됨
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p lmux_lib access_log_caps`
Expected: FAIL

- [ ] **Step 3: `AccessLogger` 구현**

```rust
pub(crate) struct AccessLogger {
    capacity: usize,
    entries: Mutex<std::collections::VecDeque<AccessEntry>>,
}
```

- [ ] **Step 4: 모든 /api/* 라우트에 접속 기록 추가**

인증 성공 후 `logger.append(...)` 호출.

- [ ] **Step 5: Web Push VAPID 설정**

`web-push` 크레이트의 `VapidSignatureBuilder`. 앱 시작 시 `PartialVapidSignatureBuilder::from_pem_no_sub` 또는 새 키 생성. 구독 endpoint + keys를 메모리에 저장. 세션 이벤트(waiting, done, context > threshold) 발생 시 push 발송. 발송 실패 시 로그만 남기고 패닉하지 않음.

- [ ] **Step 6: `mobile_access_log` Tauri 커맨드**

```rust
#[tauri::command(async)]
fn mobile_access_log(mob: State<'_, ...>) -> Vec<AccessEntry> { ... }
```

- [ ] **Step 7: 테스트 + 빌드**

Run: `cargo test -p lmux_lib`
Expected: 모든 테스트 통과

- [ ] **Step 8: Commit**

```
git add src-tauri/src/mobile.rs
git commit -m "feat: access log circular buffer and Web Push integration"
```

---

### Task 7: 데스크톱 SetMobile.tsx + SettingsPanel 연결

**Files:**
- Create: `src/SetMobile.tsx`
- Modify: `src/SettingsPanel.tsx`
- Modify: `src/api.ts`
- Create: `src/SetMobile.test.tsx`

**Interfaces:**
- Consumes:
  - Tauri 커맨드: `mobile_status`, `mobile_qr_info`, `mobile_approve_pairing`, `mobile_revoke_device`, `mobile_set_permission`, `mobile_get_devices`, `mobile_access_log`
  - Tauri 이벤트: `"mobile-pair-request"` (SetMobile이 listen)
- Produces:
  - `SetMobile` 컴포넌트 — MobileLink.dc.html 기준 레이아웃

- [ ] **Step 1: 테스트 작성 — SetMobile 렌더링**

```typescript
// src/SetMobile.test.tsx
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
afterEach(cleanup);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("SetMobile", () => {
  it("renders connection status section", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
      if (cmd === "mobile_status") return Promise.resolve({ ip: "100.84.12.7", port: 7420, tls_ok: true, server_running: true });
      if (cmd === "mobile_qr_info") return Promise.resolve({ svg: "<svg/>", url: "https://test", expires_in_secs: 272 });
      if (cmd === "mobile_get_devices") return Promise.resolve([]);
      if (cmd === "mobile_access_log") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    const { default: SetMobile } = await import("./SetMobile");
    render(<SetMobile />);
    expect(screen.getByText("연결 상태")).toBeDefined();
    expect(screen.getByText("새 기기 페어링")).toBeDefined();
    expect(screen.getByText("등록된 기기")).toBeDefined();
    expect(screen.getByText("보안 규칙")).toBeDefined();
    expect(screen.getByText("접속 기록")).toBeDefined();
  });
});
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `npx vitest run src/SetMobile.test.tsx`
Expected: FAIL (파일 없음)

- [ ] **Step 3: `api.ts`에 모바일 커맨드 추가**

```typescript
export type MobileStatus = { ip: string; port: number; tls_ok: boolean; server_running: boolean };
export type QrInfo = { svg: string; url: string; expires_in_secs: number };
export type MobileDevice = { id: string; name: string; permission: "view"|"input"; registered_at_ms: number; last_seen_ms: number };
export type MobileAccessEntry = { timestamp_ms: number; device_name: string; action: string; target: string };
// api 객체에 추가:
mobileStatus: () => invoke<MobileStatus>("mobile_status"),
mobileQrInfo: () => invoke<QrInfo>("mobile_qr_info"),
mobileApprove: (pairId: string) => invoke<MobileDevice>("mobile_approve_pairing", { pairId }),
mobileRevoke: (deviceId: string) => invoke<void>("mobile_revoke_device", { deviceId }),
mobileSetPermission: (deviceId: string, perm: "view"|"input") => invoke<void>("mobile_set_permission", { deviceId, perm }),
mobileGetDevices: () => invoke<MobileDevice[]>("mobile_get_devices"),
mobileAccessLog: () => invoke<MobileAccessEntry[]>("mobile_access_log"),
```

- [ ] **Step 4: `SetMobile.tsx` 구현 — MobileLink.dc.html 기준**

섹션:
1. **연결 상태**: ip, port, tls 상태 표시. "지금 끊고 끄기" 버튼.
2. **새 기기 페어링**: QR SVG 인라인 렌더링, 유효 시간 카운트다운, pair-request 이벤트 수신 시 "승인" 모달.
3. **등록된 기기**: 목록, 권한 선택(보기/입력), 해제 버튼.
4. **보안 규칙**: 고정 텍스트 항목들.
5. **접속 기록**: 테이블 (시간, 기기, 동작, 대상).

- [ ] **Step 5: `SettingsPanel.tsx`에 "모바일 연동" 탭 추가**

```typescript
{ id: "mobile", label: "모바일 연동" }
// content 영역:
{activeTab === "mobile" && <SetMobile />}
```

import SetMobile 추가.

- [ ] **Step 6: 테스트 통과 확인**

Run: `npx vitest run`
Expected: 기존 + 신규 테스트 모두 통과 (60+ 테스트)

- [ ] **Step 7: 빌드 통과**

Run: `cargo check`
Expected: 오류 없음

- [ ] **Step 8: Commit**

```
git add src/SetMobile.tsx src/SetMobile.test.tsx src/SettingsPanel.tsx src/api.ts
git commit -m "feat: desktop MobileLink settings screen and mobile tab in SettingsPanel"
```

---

## Final: Stage 8 완료 절차

Stage workflow 준수:
1. 구현 — Tasks 1-7 완료
2. 빌드 — `cargo build` + `npm run build`
3. 테스트 — `cargo test -p lmux_lib` + `npx vitest run`
4. 직접 실행 — Tailscale 환경에서 실제 모바일 접속 확인
5. Codex 교차검증 — orca-cli 스킬로 Codex 세션에 전달
6. 수정 — 높음·중간 항목 수정
7. 재검증 — 빌드·테스트·실행 + Codex 재검토
8. `backups/stage-8.zip` → Stage 9 진행
