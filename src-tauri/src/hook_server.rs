#![allow(dead_code)] // Task 5에서 연결 시 제거

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use tiny_http::{Response, Server, StatusCode};

const MAX_BODY: usize = 1024 * 1024; // 1 MiB

#[derive(Debug)]
pub struct SessionEvent {
    pub session: String,
    pub hook: String,
    pub body: Vec<u8>,
}

struct Registry(Mutex<HashMap<String, String>>);

pub struct HookServer {
    port: u16,
    registry: Arc<Registry>,
}

impl HookServer {
    pub fn start(on_event: impl Fn(SessionEvent) + Send + Sync + 'static) -> io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(|e| io::Error::other(e.to_string()))?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| io::Error::other("expected IP listen addr"))?
            .port();
        let registry = Arc::new(Registry(Mutex::new(HashMap::new())));
        let reg = Arc::clone(&registry);
        let cb = Arc::new(on_event);

        std::thread::spawn(move || {
            for mut req in server.incoming_requests() {
                let reg2 = Arc::clone(&reg);
                let cb2 = Arc::clone(&cb);
                std::thread::spawn(move || {
                    let status = dispatch(&mut req, &reg2, &*cb2);
                    let _ = req.respond(Response::empty(StatusCode(status)));
                });
            }
        });

        Ok(Self { port, registry })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn register(&self, session: &str, token: &str) {
        self.registry
            .0
            .lock()
            .unwrap()
            .insert(session.to_string(), token.to_string());
    }

    pub fn unregister(&self, session: &str) {
        self.registry.0.lock().unwrap().remove(session);
    }
}

/// URL 경로 `/event/<session>/<token>/<hook>` 파싱 → 이벤트 전달 → HTTP 상태 코드 반환.
fn dispatch(req: &mut tiny_http::Request, reg: &Registry, on_event: &dyn Fn(SessionEvent)) -> u16 {
    let path = req.url().to_string();
    let tail = path.trim_start_matches('/');
    let parts: Vec<&str> = tail.splitn(4, '/').collect();
    if parts.len() != 4 || parts[0] != "event" {
        return 404;
    }
    let (session, token, hook) = (parts[1], parts[2], parts[3]);

    let valid = reg
        .0
        .lock()
        .unwrap()
        .get(session)
        .map(|t| t.as_str() == token)
        .unwrap_or(false);
    if !valid {
        return 404;
    }

    // Content-Length 초과 조기 거부 (본문을 읽기 전)
    if req.body_length().map(|l| l > MAX_BODY).unwrap_or(false) {
        return 413;
    }

    // 본문 읽기 — 최대 MAX_BODY + 1 바이트 (초과 시 413)
    use std::io::Read;
    let mut body = Vec::new();
    if req
        .as_reader()
        .take(MAX_BODY as u64 + 1)
        .read_to_end(&mut body)
        .is_err()
    {
        return 500;
    }
    if body.len() > MAX_BODY {
        return 413;
    }

    on_event(SessionEvent {
        session: session.to_string(),
        hook: hook.to_string(),
        body,
    });

    200
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// 로컬 HTTP POST 요청 → (상태 코드, 응답 바이트)
    fn http_post(port: u16, path: &str, body: &[u8]) -> u16 {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        let req = format!(
            "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            path,
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        stream.write_all(body).unwrap();

        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();

        std::str::from_utf8(&resp)
            .unwrap_or("")
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0)
    }

    #[test]
    fn correct_token_returns_200_and_delivers_event() {
        let events: Arc<Mutex<Vec<SessionEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev = Arc::clone(&events);
        let server = HookServer::start(move |e| ev.lock().unwrap().push(e)).unwrap();
        server.register("sess-1", "tok-abc");

        let status = http_post(
            server.port(),
            "/event/sess-1/tok-abc/UserPromptSubmit",
            b"{}",
        );
        assert_eq!(status, 200);

        // on_event는 respond() 전에 호출되므로 응답이 돌아오면 이미 처리됨.
        let got = events.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].session, "sess-1");
        assert_eq!(got[0].hook, "UserPromptSubmit");
        assert_eq!(got[0].body, b"{}");
    }

    #[test]
    fn wrong_token_returns_404() {
        let server = HookServer::start(|_| {}).unwrap();
        server.register("sess-1", "correct-token");

        let status = http_post(server.port(), "/event/sess-1/wrong-token/Stop", b"{}");
        assert_eq!(status, 404);
    }

    #[test]
    fn unknown_session_returns_404() {
        let server = HookServer::start(|_| {}).unwrap();

        let status = http_post(server.port(), "/event/unknown-sess/tok/Stop", b"{}");
        assert_eq!(status, 404);
    }

    #[test]
    fn body_over_1mb_returns_413() {
        let server = HookServer::start(|_| {}).unwrap();
        server.register("sess-1", "tok");

        // Content-Length 헤더 초과 → 본문 읽기 전에 413
        let big = vec![0u8; MAX_BODY + 1];
        let status = http_post(server.port(), "/event/sess-1/tok/Stop", &big);
        assert_eq!(status, 413);
    }

    #[test]
    fn unregister_stops_delivering_events() {
        let events: Arc<Mutex<Vec<SessionEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let ev = Arc::clone(&events);
        let server = HookServer::start(move |e| ev.lock().unwrap().push(e)).unwrap();
        server.register("sess-1", "tok");

        let status = http_post(server.port(), "/event/sess-1/tok/Stop", b"{}");
        assert_eq!(status, 200);

        server.unregister("sess-1");

        let status = http_post(server.port(), "/event/sess-1/tok/Stop", b"{}");
        assert_eq!(status, 404);

        // 첫 번째 이벤트만 전달됨
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn server_binds_to_loopback_only() {
        let server = HookServer::start(|_| {}).unwrap();
        let result = TcpStream::connect(("127.0.0.1", server.port()));
        assert!(result.is_ok(), "loopback 연결 실패");
    }

    #[test]
    fn concurrent_requests_do_not_block_server() {
        // on_event가 느려도 서버가 다른 요청을 계속 받는지 확인
        let count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let cnt = Arc::clone(&count);
        let server = HookServer::start(move |_| {
            std::thread::sleep(Duration::from_millis(50));
            *cnt.lock().unwrap() += 1;
        })
        .unwrap();
        server.register("sess-1", "tok");
        let port = server.port();

        let handles: Vec<_> = (0..3)
            .map(|_| std::thread::spawn(move || http_post(port, "/event/sess-1/tok/Stop", b"{}")))
            .collect();

        for h in handles {
            assert_eq!(h.join().unwrap(), 200);
        }

        // 모든 on_event 완료 대기
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn malformed_path_returns_404() {
        let server = HookServer::start(|_| {}).unwrap();
        let status = http_post(server.port(), "/bad/path", b"{}");
        assert_eq!(status, 404);
    }
}
