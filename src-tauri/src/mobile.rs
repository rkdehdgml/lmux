//! 모바일 연동 — Tailscale 사설망 전용 HTTP 서버.
//!
//! 바인딩 조건: Tailscale 100.64.0.0/10 IP만 허용, 0.0.0.0 금지.
//! 인증: 기기별 device_token (SHA-256 해시를 store에 보관, 평문 저장 금지).
//! 경로 탈출: safe_resolve() — canonicalize + starts_with 검사.
//! 민감 파일: is_sensitive_file() 패턴 목록으로 403 반환.
//! TLS: 향후 tailscale cert 기반 추가 예정 (현재 Tailscale 자체 암호화에 의존).

use crate::store::MobilePermission;
use crate::workspace::Workspace;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

// ──────────────────────── 헬퍼 ────────────────────────

/// Tailscale IP 범위 100.64.0.0/10 (100.64.x.x ~ 100.127.x.x) 여부 확인.
pub(crate) fn is_tailscale_ip(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            let octs = v4.octets();
            octs[0] == 100 && octs[1] >= 64 && octs[1] <= 127
        }
        _ => false,
    }
}

/// Windows `ipconfig /all` 파싱으로 Tailscale IP 감지.
pub(crate) fn detect_tailscale_ip() -> Option<IpAddr> {
    let out = std::process::Command::new("ipconfig")
        .arg("/all")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut in_tailscale = false;
    for line in text.lines() {
        // 어댑터 헤더 줄은 공백 없이 ':'로 끝남 ("Ethernet adapter Tailscale:")
        if !line.starts_with(' ') && line.ends_with(':') {
            in_tailscale = line.to_ascii_lowercase().contains("tailscale");
            continue;
        }
        if !in_tailscale {
            continue;
        }
        if line.contains("IPv4") {
            if let Some(raw) = line.splitn(2, ':').nth(1) {
                // "100.84.12.7(기본)" 형태 처리
                let ip_str = raw.trim().split('(').next().unwrap_or("").trim();
                if let Ok(addr) = ip_str.parse::<IpAddr>() {
                    if is_tailscale_ip(addr) {
                        return Some(addr);
                    }
                }
            }
        }
    }
    None
}

/// `tailscale status --json` 파싱으로 hostname(DNSName) 반환.
pub(crate) fn tailscale_hostname() -> Option<String> {
    let out = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let name = json.get("Self")?.get("DNSName")?.as_str()?;
    // "donghee-pc.tail3a9f.ts.net." — trailing dot 제거
    Some(name.trim_end_matches('.').to_string())
}

/// SHA-256 해시 (hex 문자열). 토큰은 항상 해시해서 저장한다.
pub(crate) fn hash_token(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ──────────────────────── 경로 탈출 방지 ────────────────────────

/// 프로젝트 루트 기준 안전한 절대 경로를 반환. 루트 밖이면 Err.
pub(crate) fn safe_resolve(
    root: &std::path::Path,
    rel: &str,
) -> Result<std::path::PathBuf, &'static str> {
    if std::path::Path::new(rel).is_absolute() {
        return Err("절대 경로 불가");
    }
    let joined = root.join(rel);
    let canonical = joined.canonicalize().map_err(|_| "경로 없음")?;
    let root_canonical = root.canonicalize().map_err(|_| "루트 없음")?;
    if canonical.starts_with(&root_canonical) {
        Ok(canonical)
    } else {
        Err("경로 탈출")
    }
}

/// 민감 파일 패턴 여부 확인.
pub(crate) fn is_sensitive_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    let bare = lower.trim_start_matches('.');
    if matches!(
        bare,
        "env" | "envrc" | "id_rsa" | "id_ed25519" | "id_ecdsa" | "id_dsa"
    ) {
        return true;
    }
    if lower.ends_with(".key")
        || lower.ends_with(".pem")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.ends_with(".jks")
        || lower.ends_with(".keystore")
    {
        return true;
    }
    lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("password")
        || lower.contains("passwd")
        || lower.contains("private")
        || lower.contains(".env.")
}

// ──────────────────────── OTP / 페어링 ────────────────────────

const PAIR_TIMEOUT_MS: u64 = 10 * 60 * 1000; // 페어링 요청 유효 시간 10분

#[derive(Clone)]
struct PendingOtp {
    expires_ms: u64,
    used: bool,
}

#[derive(Clone)]
struct PendingPair {
    #[allow(dead_code)]
    code_6: String,
    device_name: String,
    created_ms: u64,
    /// None = 대기, Some("") = 거부, Some(token) = 승인
    approved_token: Option<String>,
}

pub(crate) struct PairingStore {
    otps: Mutex<HashMap<String, PendingOtp>>,
    pairs: Mutex<HashMap<String, PendingPair>>,
}

impl PairingStore {
    pub(crate) fn new() -> Self {
        Self {
            otps: Mutex::new(HashMap::new()),
            pairs: Mutex::new(HashMap::new()),
        }
    }

    /// 새 OTP 토큰 생성 (5분 유효).
    pub(crate) fn generate_otp(&self) -> String {
        let token: String = (0..32)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();
        self.otps.lock().unwrap().insert(
            token.clone(),
            PendingOtp {
                expires_ms: now_ms() + 5 * 60 * 1000,
                used: false,
            },
        );
        token
    }

    /// OTP를 소비(1회만 사용). 유효하면 true.
    pub(crate) fn consume_otp(&self, otp: &str) -> bool {
        let mut map = self.otps.lock().unwrap();
        if let Some(entry) = map.get_mut(otp) {
            if !entry.used && entry.expires_ms > now_ms() {
                entry.used = true;
                return true;
            }
        }
        false
    }

    /// OTP가 아직 소비되지 않고 유효한지 확인.
    pub(crate) fn is_otp_active(&self, otp: &str) -> bool {
        let map = self.otps.lock().unwrap();
        map.get(otp).map_or(false, |e| !e.used && e.expires_ms > now_ms())
    }

    /// 페어링 세션 등록. 6자리 코드 반환.
    pub(crate) fn create_pair(&self, pair_id: &str, device_name: &str) -> String {
        let code: String = format!("{:06}", rand::random::<u32>() % 1_000_000);
        self.pairs.lock().unwrap().insert(
            pair_id.to_string(),
            PendingPair {
                code_6: code.clone(),
                device_name: device_name.to_string(),
                created_ms: now_ms(),
                approved_token: None,
            },
        );
        code
    }

    /// 디스크 저장 전 토큰 생성 단계. 아직 phone에 노출 안 함.
    /// 이미 처리됐거나 만료된 경우 None.
    pub(crate) fn stage_pair_token(&self, pair_id: &str) -> Option<String> {
        let map = self.pairs.lock().unwrap();
        let entry = map.get(pair_id)?;
        if entry.approved_token.is_some() {
            return None;
        }
        if entry.created_ms + PAIR_TIMEOUT_MS < now_ms() {
            return None;
        }
        Some(
            (0..32)
                .map(|_| format!("{:02x}", rand::random::<u8>()))
                .collect(),
        )
    }

    /// 디스크 저장 성공 후 토큰 커밋 — phone이 pair_status()로 조회 가능해짐.
    pub(crate) fn commit_pair_token(&self, pair_id: &str, token: String) {
        let mut map = self.pairs.lock().unwrap();
        if let Some(entry) = map.get_mut(pair_id) {
            entry.approved_token = Some(token);
        }
    }

    /// PC가 페어링 거부.
    pub(crate) fn reject_pair(&self, pair_id: &str) {
        let mut map = self.pairs.lock().unwrap();
        if let Some(entry) = map.get_mut(pair_id) {
            entry.approved_token = Some(String::new());
        }
    }

    pub(crate) fn pair_status(&self, pair_id: &str) -> PairStatus {
        let mut map = self.pairs.lock().unwrap();
        let status = match map.get(pair_id) {
            None => PairStatus::NotFound,
            Some(p) => match &p.approved_token {
                None if now_ms().saturating_sub(p.created_ms) > 10 * 60 * 1000 => {
                    PairStatus::Expired
                }
                None => PairStatus::Pending,
                Some(t) if t.is_empty() => PairStatus::Rejected,
                Some(t) => PairStatus::Approved(t.clone()),
            },
        };
        // 승인 토큰은 1회만 조회 가능 — 조회 즉시 제거
        if matches!(status, PairStatus::Approved(_)) {
            map.remove(pair_id);
        }
        status
    }

    #[cfg(test)]
    pub(crate) fn insert_expired_otp_for_test(&self, token: &str) {
        self.otps.lock().unwrap().insert(
            token.to_string(),
            PendingOtp { expires_ms: 0, used: false },
        );
    }

    pub(crate) fn pair_device_name(&self, pair_id: &str) -> Option<String> {
        self.pairs
            .lock()
            .unwrap()
            .get(pair_id)
            .map(|p| p.device_name.clone())
    }

    #[allow(dead_code)]
    pub(crate) fn pair_code(&self, pair_id: &str) -> Option<String> {
        self.pairs
            .lock()
            .unwrap()
            .get(pair_id)
            .map(|p| p.code_6.clone())
    }
}

pub(crate) enum PairStatus {
    NotFound,
    Expired,
    Pending,
    Rejected,
    Approved(String),
}

// ──────────────────────── 접속 기록 ────────────────────────

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessEntry {
    pub timestamp_ms: u64,
    pub device_name: String,
    pub action: String,
    pub target: String,
}

pub(crate) struct AccessLogger {
    capacity: usize,
    entries: Mutex<VecDeque<AccessEntry>>,
}

impl AccessLogger {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub(crate) fn append(&self, entry: AccessEntry) {
        let mut q = self.entries.lock().unwrap();
        if q.len() >= self.capacity {
            q.pop_front();
        }
        q.push_back(entry);
    }

    pub(crate) fn recent(&self, n: usize) -> Vec<AccessEntry> {
        let q = self.entries.lock().unwrap();
        let skip = q.len().saturating_sub(n);
        q.iter().skip(skip).cloned().collect()
    }
}

// ──────────────────────── Web Push 구독 ────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PushSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

pub(crate) struct PushStore {
    subs: Mutex<Vec<PushSubscription>>,
}

impl PushStore {
    pub(crate) fn new() -> Self {
        Self {
            subs: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn add(&self, sub: PushSubscription) {
        let mut v = self.subs.lock().unwrap();
        v.retain(|s| s.endpoint != sub.endpoint);
        v.push(sub);
    }

    #[allow(dead_code)]
    pub(crate) fn all(&self) -> Vec<PushSubscription> {
        self.subs.lock().unwrap().clone()
    }
}

// ──────────────────────── MobileServer ────────────────────────

pub const MOBILE_PORT: u16 = 7420;

/// Tauri 커맨드가 반환하는 서버 상태.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MobileStatus {
    pub server_running: bool,
    pub ip: Option<String>,
    pub port: u16,
    pub tls_ok: bool,
    pub hostname: Option<String>,
}

/// QR 코드 정보.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QrInfo {
    pub url: String,
    pub svg: String,
    pub expires_in_secs: u64,
}

pub struct MobileServer {
    pub status: MobileStatus,
    pub pairing: Arc<PairingStore>,
    pub logger: Arc<AccessLogger>,
    #[allow(dead_code)]
    pub push_store: Arc<PushStore>,
    otp: Arc<Mutex<String>>,
    otp_expires_ms: Arc<Mutex<u64>>,
}

impl MobileServer {
    /// 서버 시작. Tailscale IP 미검출 또는 포트 바인딩 실패 시 Err.
    pub fn start(app_handle: AppHandle) -> Result<Self, String> {
        let ip = detect_tailscale_ip().ok_or_else(|| {
            "Tailscale IP(100.x)를 찾을 수 없어요. Tailscale을 먼저 실행해 주세요.".to_string()
        })?;

        // 반환 전에 실제로 바인딩해야 server_running=true가 의미 있음
        let addr = SocketAddr::new(ip, MOBILE_PORT);
        let std_listener = std::net::TcpListener::bind(addr)
            .map_err(|e| format!("포트 {} 바인딩 실패: {e}", MOBILE_PORT))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|e| format!("소켓 설정 실패: {e}"))?;

        let hostname = tailscale_hostname();
        let pairing = Arc::new(PairingStore::new());
        let logger = Arc::new(AccessLogger::new(100));
        let push_store = Arc::new(PushStore::new());
        let otp: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let otp_expires_ms: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

        let srv_pairing = Arc::clone(&pairing);
        let srv_logger = Arc::clone(&logger);
        let srv_push = Arc::clone(&push_store);

        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                run_server_with_listener(std_listener, app_handle, srv_pairing, srv_logger, srv_push)
                    .await
            {
                eprintln!("[MobileServer] 서버 오류: {e}");
            }
        });

        Ok(Self {
            status: MobileStatus {
                server_running: true,
                ip: Some(ip.to_string()),
                port: MOBILE_PORT,
                tls_ok: false,
                hostname,
            },
            pairing,
            logger,
            push_store,
            otp,
            otp_expires_ms,
        })
    }

    /// 현재 QR 정보 반환. OTP 만료 시 자동 갱신.
    pub fn qr_info(&self) -> QrInfo {
        let mut otp = self.otp.lock().unwrap();
        let mut exp = self.otp_expires_ms.lock().unwrap();
        let now = now_ms();
        if otp.is_empty() || *exp <= now || !self.pairing.is_otp_active(&otp) {
            *otp = self.pairing.generate_otp();
            *exp = now + 5 * 60 * 1000;
        }
        let url = build_pair_url(&self.status, &otp);
        let expires_in_secs = exp.saturating_sub(now) / 1000;
        QrInfo {
            svg: generate_qr_svg(&url),
            url,
            expires_in_secs,
        }
    }
}

fn build_pair_url(status: &MobileStatus, otp: &str) -> String {
    let host = status
        .hostname
        .as_deref()
        .unwrap_or_else(|| status.ip.as_deref().unwrap_or("localhost"));
    let scheme = if status.tls_ok { "https" } else { "http" };
    format!("{scheme}://{host}:{}/pwa/pair?token={otp}", status.port)
}

fn generate_qr_svg(url: &str) -> String {
    use qrcode::QrCode;
    match QrCode::new(url.as_bytes()) {
        Ok(code) => code.render::<qrcode::render::svg::Color>().build(),
        Err(_) => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text y="20">{url}</text></svg>"#
        ),
    }
}

// ──────────────────────── axum 서버 ────────────────────────

use axum::extract::{Path, Query, State as AxumState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::convert::Infallible;
use std::net::SocketAddr;

#[derive(Clone)]
struct AppState {
    app_handle: AppHandle,
    pairing: Arc<PairingStore>,
    logger: Arc<AccessLogger>,
    push_store: Arc<PushStore>,
}

async fn run_server_with_listener(
    std_listener: std::net::TcpListener,
    app_handle: AppHandle,
    pairing: Arc<PairingStore>,
    logger: Arc<AccessLogger>,
    push_store: Arc<PushStore>,
) -> anyhow::Result<()> {
    let state = AppState {
        app_handle,
        pairing,
        logger,
        push_store,
    };
    let app = build_router(state);
    let listener = tokio::net::TcpListener::from_std(std_listener)?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handle_root))
        .route("/pwa/pair", get(handle_pwa_pair))
        .route("/pair/begin", post(handle_pair_begin))
        .route("/pair/status/:pair_id", get(handle_pair_status))
        .route("/pwa/sessions", get(handle_pwa_sessions))
        .route("/pwa/terminal/:session_id", get(handle_pwa_terminal))
        .route("/pwa/files", get(handle_pwa_files))
        .route("/pwa/fileview", get(handle_pwa_fileview))
        .route("/pwa/alerts", get(handle_pwa_alerts))
        .route("/pwa/manifest.json", get(handle_manifest))
        .route("/pwa/sw.js", get(handle_sw))
        .route("/api/sessions", get(handle_api_sessions))
        .route(
            "/api/terminal/:session_id/stream",
            get(handle_terminal_stream),
        )
        .route(
            "/api/terminal/:session_id/input",
            post(handle_terminal_input),
        )
        .route("/api/files", get(handle_api_files_root))
        .route("/api/files/:proj_id", get(handle_api_files_dir))
        .route("/api/files/:proj_id/content", get(handle_api_file_content))
        .route("/api/log", get(handle_api_log))
        .route("/api/alerts", get(handle_api_alerts))
        .route("/api/push/subscribe", post(handle_push_subscribe))
        .with_state(state)
}

// ──────────────────────── 핸들러 ────────────────────────

async fn handle_root() -> &'static str {
    "Lmux Mobile Server"
}

async fn handle_pwa_pair() -> Response {
    html_response(crate::mobile_pwa::PAIR_HTML)
}

async fn handle_pair_begin(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let token = body.get("token").and_then(|v| v.as_str()).unwrap_or("");
    let device_name = body
        .get("deviceName")
        .and_then(|v| v.as_str())
        .unwrap_or("모바일 기기");

    if !state.pairing.consume_otp(token) {
        return error_response(StatusCode::FORBIDDEN, "유효하지 않은 토큰이에요");
    }

    let pair_id: String = (0..16)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();
    let code_6 = state.pairing.create_pair(&pair_id, device_name);

    json_response(
        StatusCode::OK,
        serde_json::json!({ "pairId": pair_id, "code6": code_6 }),
    )
}

async fn handle_pair_status(
    Path(pair_id): Path<String>,
    AxumState(state): AxumState<AppState>,
) -> Response {
    match state.pairing.pair_status(&pair_id) {
        PairStatus::NotFound | PairStatus::Expired => {
            error_response(StatusCode::NOT_FOUND, "페어링 세션 없음")
        }
        PairStatus::Pending => {
            json_response(StatusCode::OK, serde_json::json!({ "status": "pending" }))
        }
        PairStatus::Rejected => {
            json_response(StatusCode::OK, serde_json::json!({ "status": "rejected" }))
        }
        PairStatus::Approved(token) => json_response(
            StatusCode::OK,
            serde_json::json!({ "status": "approved", "deviceToken": token }),
        ),
    }
}

async fn handle_pwa_sessions() -> Response {
    html_response(crate::mobile_pwa::SESSIONS_HTML)
}

async fn handle_pwa_terminal(Path(_sid): Path<String>) -> Response {
    html_response(crate::mobile_pwa::TERMINAL_HTML)
}

async fn handle_pwa_files() -> Response {
    html_response(crate::mobile_pwa::FILES_HTML)
}

async fn handle_pwa_fileview() -> Response {
    html_response(crate::mobile_pwa::FILEVIEW_HTML)
}

async fn handle_pwa_alerts() -> Response {
    html_response(crate::mobile_pwa::ALERTS_HTML)
}

async fn handle_manifest() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/manifest+json")],
        crate::mobile_pwa::MANIFEST_JSON,
    )
        .into_response()
}

async fn handle_sw() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        crate::mobile_pwa::SW_JS,
    )
        .into_response()
}

async fn handle_api_sessions(
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "세션 조회".into(),
        target: "—".into(),
    });

    let ws_guard = state.app_handle.state::<Mutex<Workspace>>();
    let ws = match ws_guard.lock() {
        Ok(w) => w,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "상태 잠금 실패"),
    };
    let stored_sessions = ws.store.state().sessions.clone();
    let stored_projects = ws.store.state().projects.clone();
    drop(ws);

    let sessions_state = state.app_handle.state::<crate::session::Sessions>();
    let result: Vec<_> = stored_sessions
        .iter()
        .map(|s| {
            let project_name = stored_projects
                .iter()
                .find(|p| p.id == s.project_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            {
                let run = sessions_state.running(&s.id);
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "projectName": project_name,
                    "running": run.is_some(),
                    "run": run.unwrap_or(0),
                })
            }
        })
        .collect();

    json_response(StatusCode::OK, serde_json::json!(result))
}

async fn handle_terminal_stream(
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "터미널 열람".into(),
        target: session_id.clone(),
    });

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let sessions = state.app_handle.state::<crate::session::Sessions>();
    // 고비트 플래그로 데스크톱 순차 ID와 충돌 최소화
    let viewer_id: u64 = rand::random::<u64>() | (1u64 << 62);
    let sid = session_id.clone();

    let attach_ok = sessions
        .attach(
            &sid,
            viewer_id,
            Box::new(move |data| tx.send(data).is_ok()),
        )
        .is_ok();

    if !attach_ok {
        return error_response(StatusCode::NOT_FOUND, "세션이 없어요");
    }

    let app_handle2 = state.app_handle.clone();
    let app_handle_ack = state.app_handle.clone();
    let session_id_ack = session_id.clone();
    let sse_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    use futures_util::StreamExt;
    let sse_stream = sse_stream.map(move |data| {
        let len = data.len();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
        app_handle_ack
            .state::<crate::session::Sessions>()
            .ack(&session_id_ack, viewer_id, len);
        Ok::<_, Infallible>(format!("data: {b64}\n\n"))
    });

    let body = axum::body::Body::from_stream(sse_stream);

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        app_handle2.state::<crate::session::Sessions>().detach(&session_id, viewer_id);
    });

    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

#[derive(Deserialize)]
struct InputBody {
    text: String,
    run: u64,
}

async fn handle_terminal_input(
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
    Json(body): Json<InputBody>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    if let Err(msg) = check_input_permission(&device) {
        return error_response(StatusCode::FORBIDDEN, msg);
    }

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "입력 보냄".into(),
        target: session_id.clone(),
    });

    let sessions = state.app_handle.state::<crate::session::Sessions>();
    // 오래된 run 번호는 무음 무시 대신 오류 반환 — PWA가 입력 보존 가능
    match sessions.current_run(&session_id) {
        Err(_) => return error_response(StatusCode::NOT_FOUND, "세션이 없어요"),
        Ok(cur) if cur != body.run => {
            return json_response(
                StatusCode::CONFLICT,
                serde_json::json!({"ok": false, "staleRun": true, "currentRun": cur}),
            );
        }
        Ok(_) => {}
    }
    match sessions.write(&session_id, body.run, 0, body.text.into_bytes()) {
        Ok(()) => json_response(StatusCode::OK, serde_json::json!({"ok": true})),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e),
    }
}

async fn handle_api_files_root(
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    let ws_guard = state.app_handle.state::<Mutex<Workspace>>();
    let ws = match ws_guard.lock() {
        Ok(w) => w,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "상태 잠금 실패"),
    };
    let projects: Vec<_> = ws
        .store
        .state()
        .projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "path": p.path,
                "kind": format!("{:?}", p.kind).to_lowercase(),
            })
        })
        .collect();
    drop(ws);

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "파일 목록".into(),
        target: "프로젝트 목록".into(),
    });

    json_response(StatusCode::OK, serde_json::json!(projects))
}

#[derive(Deserialize)]
struct FileQuery {
    path: Option<String>,
}

async fn handle_api_files_dir(
    Path(proj_id): Path<String>,
    Query(q): Query<FileQuery>,
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    let project_path = {
        let ws_guard = state.app_handle.state::<Mutex<Workspace>>();
        let ws = match ws_guard.lock() {
            Ok(w) => w,
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "잠금 실패"),
        };
        ws.store
            .state()
            .projects
            .iter()
            .find(|p| p.id == proj_id)
            .map(|p| p.path.clone())
    };

    let project_path = match project_path {
        Some(p) => p,
        None => return error_response(StatusCode::NOT_FOUND, "프로젝트 없음"),
    };

    let root = std::path::Path::new(&project_path);
    let rel = q.path.as_deref().unwrap_or("");
    let target = if rel.is_empty() {
        root.to_path_buf()
    } else {
        match safe_resolve(root, rel) {
            Ok(p) => p,
            Err(_) => return error_response(StatusCode::FORBIDDEN, "접근 불가"),
        }
    };

    if !target.is_dir() {
        return error_response(StatusCode::BAD_REQUEST, "디렉토리가 아니에요");
    }

    let entries = match std::fs::read_dir(&target) {
        Ok(e) => e,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "디렉토리 읽기 실패"),
    };

    let mut items = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_sensitive_file(&name) {
            continue; // 민감 파일은 목록에서 제외 — 이름·메타데이터 노출 방지
        }
        let meta = entry.metadata().ok();
        let size = meta
            .as_ref()
            .and_then(|m| if is_dir { None } else { Some(m.len()) });
        let modified_ms = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);
        items.push(serde_json::json!({
            "name": name,
            "isDir": is_dir,
            "sizeBytes": size,
            "modifiedMs": modified_ms,
        }));
    }

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "파일 열람".into(),
        target: rel.to_string(),
    });

    json_response(StatusCode::OK, serde_json::json!(items))
}

async fn handle_api_file_content(
    Path(proj_id): Path<String>,
    Query(q): Query<FileQuery>,
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };

    let project_path = {
        let ws_guard = state.app_handle.state::<Mutex<Workspace>>();
        let ws = match ws_guard.lock() {
            Ok(w) => w,
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "잠금 실패"),
        };
        ws.store
            .state()
            .projects
            .iter()
            .find(|p| p.id == proj_id)
            .map(|p| p.path.clone())
    };

    let project_path = match project_path {
        Some(p) => p,
        None => return error_response(StatusCode::NOT_FOUND, "프로젝트 없음"),
    };

    let rel = match q.path.as_deref() {
        Some(r) if !r.is_empty() => r,
        _ => return error_response(StatusCode::BAD_REQUEST, "path 파라미터 필요"),
    };

    let root = std::path::Path::new(&project_path);
    let resolved = match safe_resolve(root, rel) {
        Ok(p) => p,
        Err(_) => return error_response(StatusCode::FORBIDDEN, "접근 불가"),
    };

    // rel 구성요소 검사 — 명시적 민감 이름 차단
    for component in std::path::Path::new(rel).components() {
        if let std::path::Component::Normal(c) = component {
            if let Some(name) = c.to_str() {
                if is_sensitive_file(name) {
                    return error_response(StatusCode::FORBIDDEN, "민감 경로에요");
                }
            }
        }
    }

    // 심볼릭 링크·정션 우회 방지: 정규화된 경로의 root 이후 구성요소도 검사
    {
        let root_canon = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf());
        let rel_canon = resolved.strip_prefix(&root_canon).unwrap_or(&resolved);
        for component in rel_canon.components() {
            if let std::path::Component::Normal(c) = component {
                if let Some(name) = c.to_str() {
                    if is_sensitive_file(name) {
                        return error_response(StatusCode::FORBIDDEN, "민감 경로에요");
                    }
                }
            }
        }
    }

    const MAX_BYTES: u64 = 100 * 1024;
    let meta = match std::fs::metadata(&resolved) {
        Ok(m) => m,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "파일 없음"),
    };
    if meta.len() > MAX_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "파일이 너무 커요 (최대 100KB)",
        );
    }

    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(_) => {
            return error_response(StatusCode::UNPROCESSABLE_ENTITY, "텍스트 파일이 아니에요")
        }
    };

    state.logger.append(AccessEntry {
        timestamp_ms: now_ms(),
        device_name: device.name.clone(),
        action: "파일 내용".into(),
        target: rel.to_string(),
    });

    json_response(
        StatusCode::OK,
        serde_json::json!({ "content": content, "path": rel }),
    )
}

async fn handle_api_log(
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let _device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let entries = state.logger.recent(100);
    json_response(StatusCode::OK, serde_json::json!(entries))
}

async fn handle_api_alerts(
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
) -> Response {
    let _device = match auth_device(&headers, &state) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let all = state.logger.recent(50);
    json_response(StatusCode::OK, serde_json::json!(all))
}

#[derive(Deserialize)]
struct PushBody {
    endpoint: String,
    p256dh: String,
    auth: String,
}

async fn handle_push_subscribe(
    headers: axum::http::HeaderMap,
    AxumState(state): AxumState<AppState>,
    Json(body): Json<PushBody>,
) -> Response {
    if let Err(r) = auth_device(&headers, &state) {
        return r;
    }
    state.push_store.add(PushSubscription {
        endpoint: body.endpoint,
        p256dh: body.p256dh,
        auth: body.auth,
    });
    json_response(StatusCode::OK, serde_json::json!({ "ok": true }))
}

// ──────────────────────── 인증 ────────────────────────

#[derive(Clone)]
pub struct AuthedDevice {
    pub name: String,
    pub permission: MobilePermission,
}

fn auth_device(
    headers: &axum::http::HeaderMap,
    state: &AppState,
) -> Result<AuthedDevice, Response> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = raw.strip_prefix("Bearer ").unwrap_or("").trim();
    if token.is_empty() {
        return Err(error_response(StatusCode::UNAUTHORIZED, "인증이 필요해요"));
    }
    let token_hash = hash_token(token);
    let ws_guard = state.app_handle.state::<Mutex<Workspace>>();
    let ws = ws_guard
        .lock()
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "상태 잠금 실패"))?;
    let device = ws
        .store
        .state()
        .mobile_devices
        .iter()
        .find(|d| d.token_hash == token_hash)
        .map(|d| AuthedDevice {
            name: d.name.clone(),
            permission: d.permission.clone(),
        });
    drop(ws);
    device.ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "등록되지 않은 기기예요"))
}

pub(crate) fn check_input_permission(device: &AuthedDevice) -> Result<(), &'static str> {
    if device.permission == MobilePermission::Input {
        Ok(())
    } else {
        Err("입력 권한이 없어요")
    }
}

// ──────────────────────── 응답 헬퍼 ────────────────────────

fn html_response(html: &'static str) -> Response {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/html; charset=utf-8",
        )],
        html,
    )
        .into_response()
}

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

fn error_response(status: StatusCode, msg: &str) -> Response {
    json_response(status, serde_json::json!({ "error": msg }))
}

// ──────────────────────── 테스트 ────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tailscale_ip_returns_100x_or_none() {
        let result = detect_tailscale_ip();
        if let Some(ip) = result {
            let first_octet = match ip {
                IpAddr::V4(v4) => v4.octets()[0],
                _ => panic!("IPv6 unexpected"),
            };
            assert_eq!(first_octet, 100, "Tailscale IP must start with 100");
            assert!(is_tailscale_ip(ip));
        }
        // None도 정상 (Tailscale 미실행 환경)
    }

    #[test]
    fn filter_tailscale_ip_selects_correct_range() {
        use std::net::Ipv4Addr;
        assert!(is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 84, 12, 7))));
        assert!(is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 100, 0, 1))));
        assert!(is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 127, 255, 255))));
        assert!(!is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 0, 0, 1))));
        assert!(!is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 63, 0, 1))));
        assert!(!is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(100, 128, 0, 1))));
        assert!(!is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_tailscale_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }

    #[test]
    fn token_hash_is_deterministic_and_not_plaintext() {
        let token = "test-device-token-abc123";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, token);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64자
    }

    #[test]
    fn otp_is_consumed_after_first_use() {
        let store = PairingStore::new();
        let otp = store.generate_otp();
        assert!(store.consume_otp(&otp), "첫 번째 사용은 성공");
        assert!(!store.consume_otp(&otp), "두 번째 사용은 실패(재사용 불가)");
    }

    #[test]
    fn path_traversal_blocked() {
        let root = std::env::temp_dir();
        let abs = root.to_string_lossy().to_string();
        assert!(safe_resolve(&root, &abs).is_err(), "절대 경로 거부");
    }

    #[test]
    fn path_traversal_escape_blocked() {
        let root = std::env::temp_dir();
        // .. 경로 탈출 시도: canonicalize가 실제 경로 반환하면 starts_with 체크로 차단
        let result = safe_resolve(&root, r"..\..\Windows");
        match result {
            Err(_) => {} // 경로 없거나 탈출 → 정상
            Ok(p) => {
                let root_canon = root.canonicalize().unwrap_or(root.clone());
                assert!(p.starts_with(&root_canon), "root 밖 경로가 허용됨: {p:?}");
            }
        }
    }

    #[test]
    fn safe_path_within_root_allowed() {
        let root = std::env::temp_dir();
        let result = safe_resolve(&root, ".");
        assert!(result.is_ok(), "현재 디렉토리는 허용되어야 함");
        let root_canon = root.canonicalize().unwrap_or(root.clone());
        assert!(result.unwrap().starts_with(&root_canon));
    }

    #[test]
    fn expired_otp_is_rejected() {
        let store = PairingStore::new();
        store.insert_expired_otp_for_test("expired-token");
        assert!(!store.consume_otp("expired-token"), "만료 OTP는 거부");
    }

    #[test]
    fn env_file_is_hidden() {
        assert!(is_sensitive_file(".env"));
        assert!(is_sensitive_file("server.key"));
        assert!(is_sensitive_file("cert.pem"));
        assert!(is_sensitive_file("keystore.p12"));
        assert!(is_sensitive_file("id_rsa"));
        assert!(is_sensitive_file("id_ed25519"));
        assert!(!is_sensitive_file("application.yml"));
        assert!(!is_sensitive_file("README.md"));
        assert!(!is_sensitive_file("Main.java"));
    }

    #[test]
    fn access_log_caps_at_capacity() {
        let log = AccessLogger::new(5);
        for i in 0u64..10 {
            log.append(AccessEntry {
                timestamp_ms: i,
                device_name: "test".into(),
                action: "connect".into(),
                target: "—".into(),
            });
        }
        let entries = log.recent(100);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].timestamp_ms, 5);
    }

    #[test]
    fn view_permission_cannot_send_input() {
        let device = AuthedDevice {
            name: "test".into(),
            permission: MobilePermission::View,
        };
        assert!(check_input_permission(&device).is_err());
    }

    #[test]
    fn input_permission_can_send_input() {
        let device = AuthedDevice {
            name: "test".into(),
            permission: MobilePermission::Input,
        };
        assert!(check_input_permission(&device).is_ok());
    }
}
