//! ConPTY 세션 관리: 실행, 출력 전달(링버퍼 + 보고 있는 화면), 입력, 리사이즈, 종료.

use crate::ring::{DEFAULT_MAX_LINES, Ring};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 화면이 아직 처리하지 못한 바이트가 이만큼 쌓이면 셸 출력을 잠시 멈춘다.
const MAX_UNACKED: usize = 2 * 1024 * 1024;
/// 그 상태로 이 시간 동안 응답이 없으면 화면 연결을 끊고 링버퍼에만 쌓는다.
const STALL_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH: usize = 64 * 1024;
const REPLAY_CHUNK: usize = 64 * 1024;
/// 셸이 읽지 않은 입력의 최대 크기.
const MAX_PENDING_INPUT: usize = 4 * 1024 * 1024;
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_REPLY: &[u8] = b"\x1b[1;1R";
/// ConPTY가 시작할 때 보내는 커서 질의를 찾는 범위.
const STARTUP_WINDOW: usize = 4096;

/// 출력을 받는 쪽. false를 돌려주면 연결이 끊긴 것으로 본다.
/// 빈 데이터는 신호다: 붙은 뒤 첫 번째는 "재생 끝", 그 다음은 "연결을 끊었으니 다시 붙어라".
pub type Sink = Box<dyn Fn(Vec<u8>) -> bool + Send>;
pub type OnExit = Box<dyn FnOnce(u32) + Send>;

struct Viewer {
    id: u64,
    sink: Sink,
}

struct Output {
    ring: Ring,
    viewer: Option<Viewer>,
    unacked: usize,
    exit_code: Option<u32>,
    /// 가장 최근에 붙으려 한 화면 번호. 늦게 도착한 이전 화면의 요청은 거절한다.
    latest_viewer: u64,
    /// 화면에 넘겼는데 아직 답이 오지 않은 커서 위치 질의.
    pending_queries: usize,
    queried_at: Instant,
}

impl Output {
    /// 화면 연결을 끊는다. 그 화면이 답하지 못한 질의는 대신 답한다.
    fn drop_viewer(&mut self, input: &Input) -> Option<Viewer> {
        for _ in 0..std::mem::take(&mut self.pending_queries) {
            input.reply();
        }
        self.unacked = 0;
        self.viewer.take()
    }

    /// 미응답 커서 질의의 기한까지 남은 시간. 질의가 없으면 `stall` 전체.
    fn query_time_left(&self, stall: Duration) -> Duration {
        if self.pending_queries == 0 {
            return stall;
        }
        stall.saturating_sub(self.queried_at.elapsed())
    }
}

struct Shared {
    output: Mutex<Output>,
    drained: Condvar,
    input: Input,
}

impl Shared {
    fn new(input: Input) -> Self {
        Shared {
            output: Mutex::new(Output {
                ring: Ring::new(DEFAULT_MAX_LINES),
                viewer: None,
                unacked: 0,
                exit_code: None,
                latest_viewer: 0,
                pending_queries: 0,
                queried_at: Instant::now(),
            }),
            drained: Condvar::new(),
            input,
        }
    }

    fn write(&self, viewer: u64, data: Vec<u8>) -> Result<(), String> {
        if !is_cursor_reply(&data) {
            return self.input.send(data);
        }
        let mut out = self.output.lock().unwrap();
        // 떼어 낸 화면의 늦은 응답이다. 그 질의에는 이미 대신 답했다.
        if out.viewer.as_ref().is_none_or(|v| v.id != viewer) {
            return Ok(());
        }
        // 보내지 못했으면 미응답으로 남겨 대신 답할 수 있게 한다.
        self.input.send(data)?;
        out.pending_queries = out.pending_queries.saturating_sub(1);
        out.queried_at = Instant::now();
        Ok(())
    }
}

/// 셸 입력. 자식이 읽지 않아 파이프가 막혀도 UI가 멈추지 않게 전용 스레드가 쓴다.
#[derive(Clone)]
struct Input {
    tx: Sender<Vec<u8>>,
    pending: Arc<AtomicUsize>,
}

impl Input {
    fn spawn(mut writer: Box<dyn Write + Send>) -> Self {
        let (tx, rx) = channel::<Vec<u8>>();
        let pending = Arc::new(AtomicUsize::new(0));
        let written = pending.clone();
        thread::spawn(move || {
            // 빈 데이터는 close()가 보내는 종료 신호다.
            for data in rx.iter().take_while(|d| !d.is_empty()) {
                if writer
                    .write_all(&data)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
                written.fetch_sub(data.len(), Ordering::SeqCst);
            }
        });
        Input { tx, pending }
    }

    fn send(&self, data: Vec<u8>) -> Result<(), String> {
        let len = data.len();
        if len == 0 {
            return Ok(());
        }
        if self.pending.fetch_add(len, Ordering::SeqCst) + len > MAX_PENDING_INPUT {
            self.pending.fetch_sub(len, Ordering::SeqCst);
            return Err("입력이 너무 많이 밀려 있어요. 잠시 뒤 다시 시도하세요".into());
        }
        self.tx
            .send(data)
            .map_err(|_| "세션 입력이 닫혔어요".to_string())
    }

    /// 커서 위치 응답. 작고, 없으면 프로그램이 멈추므로 입력 한도와 무관하게 보낸다.
    fn reply(&self) {
        self.pending.fetch_add(CURSOR_REPLY.len(), Ordering::SeqCst);
        let _ = self.tx.send(CURSOR_REPLY.to_vec());
    }

    fn close(&self) {
        let _ = self.tx.send(Vec::new());
    }
}

type Master = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

struct Session {
    run: u64,
    shared: Arc<Shared>,
    master: Master,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct AttachInfo {
    pub run: u64,
    pub exit_code: Option<u32>,
}

/// 맵 잠금은 조회·등록에만 쓴다. 오래 걸릴 수 있는 OS 호출은 잠금 밖에서 한다.
#[derive(Default)]
pub struct Sessions {
    map: Mutex<HashMap<String, Session>>,
    /// 시작·종료를 한 번에 하나씩 처리한다. 값이 true면 앱을 닫는 중이다.
    closing: Mutex<bool>,
    runs: AtomicU64,
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.clamp(2, 1000),
        rows: rows.clamp(1, 1000),
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Sessions {
    /// 실행 번호를 돌려준다. 같은 id의 세션이 이미 있으면 그 번호를 돌려주고 아무것도 하지 않는다.
    /// `on_exit`는 만들어진 실행 번호와 종료 코드를 받는다.
    pub fn start(
        &self,
        id: &str,
        cmd: CommandBuilder,
        cols: u16,
        rows: u16,
        startup: Option<String>,
        on_exit: impl FnOnce(u64, u32) + Send + 'static,
    ) -> Result<u64, String> {
        let closing = self.closing.lock().unwrap();
        if *closing {
            return Err("앱을 종료하는 중이에요".into());
        }
        if let Ok(run) = self.with(id, |s| s.run) {
            return Ok(run);
        }
        let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        let session = spawn(
            run,
            cmd,
            cols,
            rows,
            startup,
            Box::new(move |code| on_exit(run, code)),
        )?;
        self.map.lock().unwrap().insert(id.to_string(), session);
        Ok(run)
    }

    fn with<T>(&self, id: &str, f: impl FnOnce(&Session) -> T) -> Result<T, String> {
        self.map
            .lock()
            .unwrap()
            .get(id)
            .map(f)
            .ok_or_else(|| "세션이 없어요".to_string())
    }

    /// 이미 띄운 세션의 실행 번호.
    pub fn running(&self, id: &str) -> Option<u64> {
        self.with(id, |s| s.run).ok()
    }

    /// 지금까지의 출력을 조각내어 먼저 보내고, 빈 데이터로 끝을 알린 뒤 실시간 출력을 이어 보낸다.
    pub fn attach(&self, id: &str, viewer: u64, sink: Sink) -> Result<AttachInfo, String> {
        let (run, shared) = self.with(id, |s| (s.run, s.shared.clone()))?;
        let mut out = shared.output.lock().unwrap();
        if viewer < out.latest_viewer {
            return Err("이미 새 화면이 붙었어요".into());
        }
        out.latest_viewer = viewer;
        out.drop_viewer(&shared.input);
        shared.drained.notify_all();
        let replay = out.ring.snapshot();
        let sent = replay.chunks(REPLAY_CHUNK).all(|c| sink(c.to_vec())) && sink(Vec::new());
        if !sent {
            return Err("화면에 출력을 보내지 못했어요".into());
        }
        out.unacked = replay.len();
        out.viewer = Some(Viewer { id: viewer, sink });
        Ok(AttachInfo {
            run,
            exit_code: out.exit_code,
        })
    }

    /// 그 사이 다른 화면이 붙었다면 건드리지 않는다.
    pub fn detach(&self, id: &str, viewer: u64) {
        if let Ok(shared) = self.with(id, |s| s.shared.clone()) {
            let mut out = shared.output.lock().unwrap();
            if out.viewer.as_ref().is_some_and(|v| v.id == viewer) {
                out.drop_viewer(&shared.input);
                shared.drained.notify_all();
            }
        }
    }

    pub fn ack(&self, id: &str, viewer: u64, bytes: usize) {
        if let Ok(shared) = self.with(id, |s| s.shared.clone()) {
            let mut out = shared.output.lock().unwrap();
            if out.viewer.as_ref().is_some_and(|v| v.id == viewer) {
                out.unacked = out.unacked.saturating_sub(bytes);
                shared.drained.notify_all();
            }
        }
    }

    /// 다시 시작하기 전 실행(`run`)에 보낸 늦은 입력은 버린다.
    pub fn write(&self, id: &str, run: u64, viewer: u64, data: Vec<u8>) -> Result<(), String> {
        let (current, shared) = self.with(id, |s| (s.run, s.shared.clone()))?;
        if run != current {
            return Ok(());
        }
        shared.write(viewer, data)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let master = self.with(id, |s| s.master.clone())?;
        let master = master.lock().unwrap();
        match master.as_ref() {
            Some(m) => m.resize(pty_size(cols, rows)).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    pub fn kill(&self, id: &str) {
        let _order = self.closing.lock().unwrap();
        let removed = self.map.lock().unwrap().remove(id);
        if let Some(s) = removed {
            end(s);
        }
    }

    /// 앱 종료용. 이후의 시작 요청은 거절한다.
    pub fn kill_all(&self) {
        let mut closing = self.closing.lock().unwrap();
        *closing = true;
        let all: Vec<Session> = self.map.lock().unwrap().drain().map(|(_, s)| s).collect();
        all.into_iter().for_each(end);
    }
}

fn spawn(
    run: u64,
    cmd: CommandBuilder,
    cols: u16,
    rows: u16,
    startup: Option<String>,
    on_exit: OnExit,
) -> Result<Session, String> {
    let pair = native_pty_system()
        .openpty(pty_size(cols, rows))
        .map_err(|e| e.to_string())?;
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let shared = Arc::new(Shared::new(Input::spawn(writer)));
    let master: Master = Arc::new(Mutex::new(Some(pair.master)));
    let killer = child.clone_killer();

    let (output_tx, output_rx) = sync_channel::<Vec<u8>>(64);
    thread::spawn(move || read_loop(&mut *reader, output_tx));

    let pump_shared = shared.clone();
    thread::spawn(move || {
        pump(&pump_shared, output_rx, STALL_TIMEOUT, startup);
        let code = pump_shared.output.lock().unwrap().exit_code.unwrap_or(1);
        on_exit(code);
    });

    // ConPTY는 자식이 끝나도 닫아 주기 전까지 EOF를 보내지 않는다.
    let wait_shared = shared.clone();
    let wait_master = master.clone();
    thread::spawn(move || {
        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
        wait_shared.output.lock().unwrap().exit_code = Some(code);
        wait_shared.drained.notify_all();
        wait_shared.input.close();
        let closed = wait_master.lock().unwrap().take();
        drop(closed);
    });

    Ok(Session {
        run,
        shared,
        master,
        killer,
    })
}

fn end(mut s: Session) {
    {
        let mut out = s.shared.output.lock().unwrap();
        out.viewer = None;
        s.shared.drained.notify_all();
    }
    // 셸이 이미 끝났으면 실패해도 문제없다. ConPTY는 대기 스레드가 닫는다.
    let _ = s.killer.kill();
}

fn read_loop(reader: &mut dyn Read, tx: SyncSender<Vec<u8>>) {
    let mut buf = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        }
    }
}

fn find_query(data: &[u8]) -> Option<usize> {
    data.windows(CURSOR_QUERY.len())
        .position(|w| w == CURSOR_QUERY)
}

fn count_queries(data: &[u8]) -> usize {
    data.windows(CURSOR_QUERY.len())
        .filter(|w| *w == CURSOR_QUERY)
        .count()
}

/// 화면(xterm)이 보내는 커서 위치 응답 `ESC [ 행 ; 열 R` 인지.
fn is_cursor_reply(data: &[u8]) -> bool {
    let number =
        |p: Option<&[u8]>| p.is_some_and(|p| !p.is_empty() && p.iter().all(u8::is_ascii_digit));
    data.strip_prefix(b"\x1b[")
        .and_then(|d| d.strip_suffix(b"R"))
        .is_some_and(|d| {
            let mut parts = d.split(|&b| b == b';');
            number(parts.next()) && number(parts.next()) && parts.next().is_none()
        })
}

/// 끝에 걸친 미완성 커서 질의를 떼어 다음 묶음 앞에 붙일 수 있게 한다.
fn split_partial_query(data: &mut Vec<u8>) -> Vec<u8> {
    (1..CURSOR_QUERY.len())
        .rev()
        .find(|&n| data.ends_with(&CURSOR_QUERY[..n]))
        .map_or_else(Vec::new, |n| data.split_off(data.len() - n))
}

/// 밀린 출력은 한 번에 묶어 보낸다.
/// `startup`은 셸이 뜬 뒤 한 번 입력할 명령 줄이다.
fn pump(shared: &Shared, rx: Receiver<Vec<u8>>, stall: Duration, mut startup: Option<String>) {
    let mut held = Vec::new();
    let mut startup_left = STARTUP_WINDOW;
    loop {
        // 질의 응답을 기다리느라 조용해진 때에도, 출력이 계속될 때에도 확인한다.
        let wait = expire_query(shared, stall);
        let first = match rx.recv_timeout(wait) {
            Ok(first) => first,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let mut batch = std::mem::take(&mut held);
        batch.extend(first);
        while batch.len() < MAX_BATCH {
            match rx.try_recv() {
                Ok(more) => batch.extend(more),
                Err(_) => break,
            }
        }
        held = split_partial_query(&mut batch);
        // 시작 질의는 항상 여기서 답하고 지운다. 화면 쪽에서 답하면 놓치거나 두 번 답할 수 있다.
        if startup_left > 0 {
            match find_query(&batch) {
                Some(at) => {
                    batch.drain(at..at + CURSOR_QUERY.len());
                    shared.input.reply();
                    startup_left = 0;
                }
                None => startup_left = startup_left.saturating_sub(batch.len()),
            }
            // 질의 응답보다 먼저 입력하지 않는다.
            if startup_left == 0
                && let Some(line) = startup.take()
                && let Err(error) = shared.input.send(format!("{line}\r").into_bytes())
            {
                eprintln!("시작 명령을 입력하지 못했어요: {error}");
            }
        }
        deliver(shared, batch, stall);
    }
    deliver(shared, held, stall);
}

/// 응답이 끊긴 화면을 떼고, 다시 붙으라는 신호를 보낸다.
fn give_up_on_viewer(shared: &Shared, out: &mut Output) {
    if let Some(v) = out.drop_viewer(&shared.input) {
        (v.sink)(Vec::new());
    }
}

/// 기한이 지난 질의가 있으면 화면을 뗀다. 다음에 확인하기까지 기다릴 시간을 돌려준다.
fn expire_query(shared: &Shared, stall: Duration) -> Duration {
    let mut out = shared.output.lock().unwrap();
    let left = out.query_time_left(stall);
    if !left.is_zero() {
        return left;
    }
    give_up_on_viewer(shared, &mut out);
    stall
}

fn deliver(shared: &Shared, batch: Vec<u8>, stall: Duration) {
    // 빈 데이터는 화면에 보내는 신호로 쓰이므로 건너뛴다.
    if batch.is_empty() {
        return;
    }
    let mut out = shared.output.lock().unwrap();
    // 화면이 마지막으로 진전을 보였을 때의 (화면 번호, 미확인 바이트, 시각).
    let mut progress = (None, 0, Instant::now());
    while out.viewer.is_some() && out.unacked > MAX_UNACKED && out.exit_code.is_none() {
        let id = out.viewer.as_ref().map(|v| v.id);
        if progress.0 != id || out.unacked < progress.1 {
            progress = (id, out.unacked, Instant::now());
        }
        // 출력 정체와 커서 질의 중 먼저 오는 기한까지만 기다린다.
        let left = stall
            .saturating_sub(progress.2.elapsed())
            .min(out.query_time_left(stall));
        if left.is_zero() {
            give_up_on_viewer(shared, &mut out);
        } else {
            out = shared.drained.wait_timeout(out, left).unwrap().0;
        }
    }
    out.ring.push(&batch);
    let queries = count_queries(&batch);
    let len = batch.len();
    if out.viewer.as_ref().is_some_and(|v| (v.sink)(batch)) {
        out.unacked += len;
        if queries > 0 {
            if out.pending_queries == 0 {
                out.queried_at = Instant::now();
            }
            out.pending_queries += queries;
        }
    } else {
        // 보는 화면이 없으면 커서 위치 질의에 대신 답해야 프로그램이 멈추지 않는다.
        out.drop_viewer(&shared.input);
        for _ in 0..queries {
            shared.input.reply();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell;
    use std::time::Instant;

    type Seen = Arc<Mutex<Vec<Vec<u8>>>>;

    fn collector() -> (Sink, Seen) {
        let seen: Seen = Arc::default();
        let writer = seen.clone();
        let sink: Sink = Box::new(move |d| {
            writer.lock().unwrap().push(d);
            true
        });
        (sink, seen)
    }

    fn text(seen: &Seen) -> String {
        String::from_utf8_lossy(&seen.lock().unwrap().concat()).into_owned()
    }

    fn wait_for(seen: &Seen, needle: &str) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(30) {
            if text(seen).contains(needle) {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// 입력을 파이프 대신 메모리로 받는다.
    fn memory_input() -> (Input, Receiver<Vec<u8>>) {
        let (tx, rx) = channel();
        (
            Input {
                tx,
                pending: Arc::default(),
            },
            rx,
        )
    }

    #[test]
    fn answers_startup_query_split_across_reads_and_removes_it() {
        let (input, replies) = memory_input();
        let shared = Shared::new(input);
        let (tx, rx) = sync_channel(8);
        thread::scope(|scope| {
            scope.spawn(|| pump(&shared, rx, STALL_TIMEOUT, None));
            tx.send(b"abc\x1b[".to_vec()).unwrap();
            thread::sleep(Duration::from_millis(50));
            tx.send(b"6nxy".to_vec()).unwrap();
            drop(tx);
        });
        assert_eq!(
            replies.try_iter().collect::<Vec<_>>(),
            vec![CURSOR_REPLY.to_vec()]
        );
        assert_eq!(shared.output.lock().unwrap().ring.snapshot(), b"abcxy");
    }

    fn shared_with_viewer() -> (Shared, Receiver<Vec<u8>>, Seen) {
        let (input, replies) = memory_input();
        let shared = Shared::new(input);
        let (sink, seen) = collector();
        shared.output.lock().unwrap().viewer = Some(Viewer { id: 1, sink });
        (shared, replies, seen)
    }

    #[test]
    fn drops_viewer_that_stops_acknowledging() {
        let (shared, _replies, seen) = shared_with_viewer();
        shared.output.lock().unwrap().unacked = MAX_UNACKED + 1;
        deliver(&shared, b"more".to_vec(), Duration::from_millis(50));
        let out = shared.output.lock().unwrap();
        assert!(out.viewer.is_none());
        assert_eq!(*seen.lock().unwrap(), vec![Vec::<u8>::new()]);
        assert_eq!(out.ring.snapshot(), b"more");
    }

    #[test]
    fn answers_queries_the_viewer_leaves_unanswered() {
        let (shared, replies, seen) = shared_with_viewer();
        deliver(&shared, b"a\x1b[6nb\x1b[6n".to_vec(), STALL_TIMEOUT);
        assert!(
            replies.try_recv().is_err(),
            "화면이 붙어 있으면 화면이 답한다"
        );
        expire_query(&shared, STALL_TIMEOUT);
        assert!(
            shared.output.lock().unwrap().viewer.is_some(),
            "시간 전에는 기다린다"
        );
        expire_query(&shared, Duration::ZERO);
        assert_eq!(replies.try_iter().count(), 2);
        assert!(shared.output.lock().unwrap().viewer.is_none());
        assert_eq!(seen.lock().unwrap().last(), Some(&Vec::new()));
    }

    #[test]
    fn answers_every_query_when_hidden_even_if_input_is_full() {
        let (input, replies) = memory_input();
        input.send(vec![b'a'; MAX_PENDING_INPUT]).unwrap();
        let shared = Shared::new(input);
        deliver(&shared, b"\x1b[6n\x1b[6n".to_vec(), STALL_TIMEOUT);
        assert_eq!(replies.try_iter().filter(|r| r == CURSOR_REPLY).count(), 2);
    }

    const REPLY: &[u8] = b"\x1b[3;7R";

    #[test]
    fn a_rejected_reply_keeps_the_query_pending() {
        let (shared, replies, _seen) = shared_with_viewer();
        deliver(&shared, b"\x1b[6n".to_vec(), STALL_TIMEOUT);
        shared.input.send(vec![b'a'; MAX_PENDING_INPUT]).unwrap();
        assert!(shared.write(1, REPLY.to_vec()).is_err());
        expire_query(&shared, Duration::ZERO);
        assert_eq!(replies.try_iter().filter(|r| r == CURSOR_REPLY).count(), 1);
    }

    #[test]
    fn ignores_a_late_reply_from_a_dropped_viewer() {
        let (shared, replies, _seen) = shared_with_viewer();
        deliver(&shared, b"\x1b[6n".to_vec(), STALL_TIMEOUT);
        expire_query(&shared, Duration::ZERO);
        shared.write(1, REPLY.to_vec()).unwrap();
        assert_eq!(
            replies.try_iter().collect::<Vec<_>>(),
            vec![CURSOR_REPLY.to_vec()]
        );
        // 응답이 아닌 입력은 떼어 낸 화면의 것이라도 전달한다.
        shared.write(1, b"dir\r".to_vec()).unwrap();
        assert_eq!(replies.try_iter().count(), 1);
    }

    #[test]
    fn each_reply_restarts_the_timeout_for_the_rest() {
        let (shared, replies, _seen) = shared_with_viewer();
        deliver(&shared, b"\x1b[6n\x1b[6n".to_vec(), STALL_TIMEOUT);
        thread::sleep(Duration::from_millis(150));
        shared.write(1, REPLY.to_vec()).unwrap();
        expire_query(&shared, Duration::from_millis(100));
        assert!(shared.output.lock().unwrap().viewer.is_some());
        assert_eq!(replies.try_iter().collect::<Vec<_>>(), vec![REPLY.to_vec()]);
    }

    #[test]
    fn expires_an_unanswered_query_while_output_continues() {
        let (shared, replies, _seen) = shared_with_viewer();
        let (tx, rx) = sync_channel(8);
        thread::scope(|scope| {
            scope.spawn(|| pump(&shared, rx, Duration::from_millis(100), None));
            // 첫 질의는 시작 질의로 처리되고, 두 번째가 화면에 넘어간다.
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(10));
                tx.send(b".".to_vec()).unwrap();
            }
            assert!(shared.output.lock().unwrap().viewer.is_none());
            drop(tx);
        });
        assert_eq!(replies.try_iter().count(), 2);
    }

    #[test]
    fn output_before_the_deadline_does_not_postpone_expiry() {
        let (shared, _replies, _seen) = shared_with_viewer();
        let (tx, rx) = sync_channel(8);
        thread::scope(|scope| {
            scope.spawn(|| pump(&shared, rx, Duration::from_secs(1), None));
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            thread::sleep(Duration::from_millis(800));
            tx.send(b".".to_vec()).unwrap();
            thread::sleep(Duration::from_millis(600));
            // 질의 후 1.4초: 마지막 출력부터 다시 1초를 세면 아직 붙어 있게 된다.
            let attached = shared.output.lock().unwrap().viewer.is_some();
            drop(tx);
            assert!(!attached);
        });
    }

    /// 밀린 출력을 기다리게 해 놓고, `during`이 도는 동안 `deliver`가 걸린 시간을 잰다.
    fn deliver_while_backed_up(shared: &Shared, during: impl FnOnce() + Send) -> Duration {
        shared.output.lock().unwrap().unacked = MAX_UNACKED + 1000;
        thread::scope(|scope| {
            scope.spawn(during);
            let start = Instant::now();
            deliver(shared, b"more".to_vec(), Duration::from_millis(400));
            start.elapsed()
        })
    }

    #[test]
    fn slow_acknowledgements_do_not_postpone_query_expiry() {
        let (shared, _replies, _seen) = shared_with_viewer();
        deliver(&shared, b"\x1b[6n".to_vec(), STALL_TIMEOUT);
        let took = deliver_while_backed_up(&shared, || {
            for _ in 0..40 {
                thread::sleep(Duration::from_millis(50));
                {
                    // 화면이 떨어지면 0이 되므로 그 아래로는 빼지 않는다.
                    let mut out = shared.output.lock().unwrap();
                    out.unacked = out.unacked.saturating_sub(1);
                }
                shared.drained.notify_all();
            }
        });
        assert!(took < Duration::from_millis(1200), "{took:?}");
        assert!(shared.output.lock().unwrap().viewer.is_none());
    }

    #[test]
    fn a_new_viewer_gets_the_full_stall_time() {
        let (shared, _replies, _seen) = shared_with_viewer();
        deliver_while_backed_up(&shared, || {
            thread::sleep(Duration::from_millis(300));
            {
                let mut out = shared.output.lock().unwrap();
                out.viewer = Some(Viewer {
                    id: 2,
                    sink: collector().0,
                });
                out.unacked = MAX_UNACKED + 2000;
            }
            shared.drained.notify_all();
            thread::sleep(Duration::from_millis(250));
            assert!(shared.output.lock().unwrap().viewer.is_some());
        });
    }

    #[test]
    fn types_the_startup_line_once_after_the_startup_query() {
        let (input, replies) = memory_input();
        let shared = Shared::new(input);
        let (tx, rx) = sync_channel(8);
        thread::scope(|scope| {
            scope.spawn(|| pump(&shared, rx, STALL_TIMEOUT, Some("claude".into())));
            tx.send(b"boot".to_vec()).unwrap();
            thread::sleep(Duration::from_millis(50));
            assert!(replies.try_recv().is_err(), "질의 전에는 입력하지 않는다");
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            tx.send(b"\x1b[6n".to_vec()).unwrap();
            drop(tx);
        });
        assert_eq!(
            replies.try_iter().collect::<Vec<_>>(),
            vec![
                CURSOR_REPLY.to_vec(),
                b"claude\r".to_vec(),
                CURSOR_REPLY.to_vec()
            ]
        );
    }

    #[test]
    fn types_the_startup_line_even_without_a_startup_query() {
        let (input, replies) = memory_input();
        let shared = Shared::new(input);
        let (tx, rx) = sync_channel(8);
        thread::scope(|scope| {
            scope.spawn(|| pump(&shared, rx, STALL_TIMEOUT, Some("claude".into())));
            tx.send(vec![b'x'; STARTUP_WINDOW]).unwrap();
            tx.send(b"more".to_vec()).unwrap();
            drop(tx);
        });
        assert_eq!(
            replies.try_iter().collect::<Vec<_>>(),
            vec![b"claude\r".to_vec()]
        );
    }

    /// 에이전트 명령 `line`으로 셸을 띄우고, `expected`가 나온 뒤에도 셸을 쓸 수 있는지 본다.
    fn startup_prints(
        shell: &str,
        cwd: &std::path::Path,
        line: String,
        expected: &str,
        home: Option<&std::path::Path>,
    ) -> String {
        let sessions = Sessions::default();
        let (mut cmd, typed) = shell::command(shell, cwd.to_str().unwrap(), Some(&line)).unwrap();
        if let Some(home) = home {
            cmd.env("HOME", home);
        }
        let run = sessions
            .start(shell, cmd, 400, 30, typed, |_, _| {})
            .unwrap();
        let (sink, seen) = collector();
        sessions.attach(shell, 1, sink).unwrap();
        let found = wait_for(&seen, expected);
        if home.is_some() {
            // 명령 뒤에 남는 셸이 시작 파일을 다시 읽는 동안 기다린다.
            thread::sleep(Duration::from_secs(4));
        }
        // 명령이 끝난 뒤에도 셸이 남아 입력을 받는다. 에코와 구분되는 결과를 확인한다.
        let probe = if shell == "cmd" {
            "echo still-^here\r"
        } else {
            "echo still-\"\"here\r"
        };
        sessions.write(shell, run, 1, probe.into()).unwrap();
        let alive = wait_for(&seen, "still-here");
        sessions.kill_all();
        let output = text(&seen);
        assert!(found, "{shell}: {line}\n{output}");
        assert!(alive, "{shell}: 셸이 남지 않음\n{output}");
        output
    }

    /// 시작 스크립트가 입력을 읽어도 에이전트 명령을 가져가지 못한다.
    #[test]
    fn a_profile_that_reads_input_does_not_take_the_command() {
        let dir = std::env::temp_dir().join(format!("lmux-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".bash_profile"),
            "echo run >> \"$HOME/runs\"\nread -t 2 -r TAKEN\necho \"taken=[$TAKEN]\"\nexport lmux=old\nset -a\n",
        )
        .unwrap();
        let output = startup_prints(
            "gitbash",
            &dir,
            "printf 'deli%s\\n' vered; echo copies=$(env | grep -c -e '^lmux=' -e '^LMUX_CMD=')"
                .into(),
            "delivered",
            Some(&dir),
        );
        let runs = std::fs::read_to_string(dir.join("runs")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(output.contains("taken=[]"), "{output}");
        assert!(output.contains("copies=0"), "{output}");
        assert_eq!(
            runs.lines().count(),
            1,
            "로그인 시작 파일은 한 번만\n{output}"
        );
    }

    /// 첫 지시에 담긴 셸 문법이 실행되지 않고 인수 하나로 전달되는지, 설치된 셸마다 확인한다.
    #[test]
    fn real_shells_receive_the_prompt_as_one_argument() {
        use crate::agent::{ShellKind, argument};
        let dir = std::env::temp_dir().join(format!("lmux 인수 {}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("args.js"),
            "const a = process.argv.slice(2);\
             console.log(`TTY=${process.stdin.isTTY === true && process.stdout.isTTY === true}`);\
             console.log(`LEAK=${'LMUX_CMD' in process.env}`);\
             console.log(`ARGS${a.length}<${a.join('|')}>`);",
        )
        .unwrap();
        // npm이 만드는 .cmd 셔임처럼 인수를 cmd.exe가 다시 해석하는 경로.
        std::fs::write(dir.join("shim.cmd"), "@node \"%~dp0args.js\" %*\r\n").unwrap();
        std::fs::write(
            dir.join("shim2.ps1"),
            "& node \"$PSScriptRoot\\args.js\" $args\r\n",
        )
        .unwrap();
        let prompt = "한글 a&echo x \"q\" %OS% $(whoami) ; | > 'i' \u{2019}j ^k !l";
        let windows = "ARGS1<한글 a&echo x 'q' ％OS％ $(whoami) ; | > 'i' \u{2019}j ^k !l>";
        let posix = format!("ARGS1<{prompt}>");
        let installed: Vec<String> = shell::list(true).into_iter().map(|s| s.id).collect();
        for (id, kind, programs, expected) in [
            (
                "powershell",
                ShellKind::PowerShell,
                &["node args.js", r".\shim.cmd", r".\shim2.ps1"][..],
                windows,
            ),
            (
                "cmd",
                ShellKind::Cmd,
                &["node args.js", r".\shim.cmd"][..],
                windows,
            ),
            ("gitbash", ShellKind::Posix, &["node args.js"][..], &posix),
        ] {
            assert!(
                installed.iter().any(|i| i == id),
                "{id} 없음: {installed:?}"
            );
            for program in programs {
                let argument = argument(kind, prompt).unwrap();
                let line = format!("{program} {argument}");
                let output = startup_prints(id, &dir, line, expected, None);
                assert!(output.contains("TTY=true"), "{id}: {output}");
                assert!(output.contains("LEAK=false"), "{id}: {output}");
            }
        }
        // WSL은 설치된 PC에서만. 배포판 안에 node가 없을 수 있어 printf로 확인한다.
        if let Some(id) = installed.iter().find(|i| i.starts_with("wsl:")) {
            let argument = argument(ShellKind::Posix, prompt).unwrap();
            let output = startup_prints(
                id,
                &dir,
                format!(
                    "test -t 0 -a -t 1 && echo TTY=tr''ue; \
                     sh -c 'test -z \"${{LMUX_CMD+x}}\" && echo LEAK=fal\"\"se'; \
                     printf 'ARGS%s<%s>\\n' $(ls args.js | wc -l) {argument}"
                ),
                &posix,
                None,
            );
            assert!(output.contains("TTY=true"), "{id}: {output}");
            assert!(output.contains("LEAK=false"), "{id}: {output}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 프로젝트 폴더에 심은 프로그램이 셸을 열거나 에이전트를 부를 때 대신 실행되지 않는다.
    #[test]
    fn cmd_does_not_run_programs_planted_in_the_project_folder() {
        use crate::agent::{Agent, ShellKind, command_line};
        let dir = std::env::temp_dir().join(format!("lmux-planted-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["chcp.cmd", "lmuxagent.cmd"] {
            std::fs::write(dir.join(name), "@echo PLANTED-%~n0\r\n").unwrap();
        }
        let real = command_line(ShellKind::Cmd, Agent::Claude, "", "").unwrap();
        // 진짜 에이전트는 어떤 경우에도 실행하지 않는다. 앞의 준비 명령만 가져와 심은 이름을 부른다.
        let prefix = real.strip_suffix("claude").expect("에이전트 이름으로 끝남");
        let line = format!("{prefix}lmuxagent & echo agent-^done");
        let (mut cmd, typed) = shell::command("cmd", dir.to_str().unwrap(), Some(&line)).unwrap();
        // 테스트를 돌리는 환경에 이미 설정돼 있으면 검사가 의미 없다.
        cmd.env_remove("NoDefaultCurrentDirectoryInExePath");
        let sessions = Sessions::default();
        sessions.start("p", cmd, 200, 30, typed, |_, _| {}).unwrap();
        let (sink, seen) = collector();
        sessions.attach("p", 1, sink).unwrap();
        let done = wait_for(&seen, "agent-done");
        sessions.kill_all();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(done, "{}", text(&seen));
        assert!(!text(&seen).contains("PLANTED-"), "{}", text(&seen));
    }

    /// 입력을 읽는 AutoRun이 시작 명령을 가져가지 않게, 에이전트를 띄울 때만 건너뛴다.
    #[test]
    fn cmd_skips_autorun_only_when_it_runs_an_agent() {
        let cwd = std::env::temp_dir();
        let has = |run| {
            let (cmd, _) = shell::command("cmd", cwd.to_str().unwrap(), run).unwrap();
            cmd.get_argv().iter().any(|a| a == "/D")
        };
        assert!(has(Some("claude")));
        assert!(!has(None), "셸만 열 때는 사용자의 AutoRun을 그대로 둔다");
    }

    /// 첫 지시의 느낌표가 지연 확장으로 바뀌지 않는다.
    #[test]
    fn cmd_keeps_delayed_expansion_off() {
        let cmd = shell::command("cmd", std::env::temp_dir().to_str().unwrap(), None)
            .unwrap()
            .0;
        assert!(cmd.get_argv().iter().any(|a| a == "/V:OFF"));
    }

    #[test]
    fn reports_the_run_of_a_session_that_exists() {
        let sessions = Sessions::default();
        assert_eq!(sessions.running("r"), None);
        let cwd = std::env::temp_dir();
        let cmd = shell::command("cmd", cwd.to_str().unwrap(), None)
            .unwrap()
            .0;
        let run = sessions.start("r", cmd, 80, 24, None, |_, _| {}).unwrap();
        assert_eq!(sessions.running("r"), Some(run));
        sessions.kill("r");
        assert_eq!(sessions.running("r"), None);
    }

    #[test]
    fn recognizes_only_cursor_replies() {
        assert!(is_cursor_reply(b"\x1b[12;80R"));
        assert!(!is_cursor_reply(b"\x1b[A"));
        assert!(!is_cursor_reply(b"\x1b[;R"));
        assert!(!is_cursor_reply(b"12;80R"));
    }

    #[test]
    fn rejects_input_beyond_the_pending_limit() {
        let (input, _rx) = memory_input();
        input.send(vec![b'a'; MAX_PENDING_INPUT]).unwrap();
        assert!(input.send(vec![b'b']).is_err());
    }

    #[test]
    fn powershell_runs_korean_replays_resizes_and_exits() {
        let sessions = Sessions::default();
        let cwd = std::env::temp_dir();
        let (exit_tx, exit_rx) = channel();
        let run = sessions
            .start(
                "t",
                shell::command("powershell", cwd.to_str().unwrap(), None)
                    .unwrap()
                    .0,
                100,
                30,
                None,
                move |run, code| exit_tx.send((run, code)).unwrap(),
            )
            .unwrap();
        // 화면이 붙기 전의 출력도 링버퍼에 남아야 한다.
        thread::sleep(Duration::from_millis(500));
        let (sink, seen) = collector();
        assert_eq!(sessions.attach("t", 1, sink).unwrap().run, run);
        // 입력한 명령의 에코와 구분하려고 출력에서만 합쳐지는 문자열을 확인한다.
        sessions
            .write("t", run, 1, "Write-Output ('한글' + '확인')\r".into())
            .unwrap();
        assert!(wait_for(&seen, "한글확인"), "{}", text(&seen));

        sessions.detach("t", 1);
        sessions.resize("t", 90, 20).unwrap();
        sessions
            .write(
                "t",
                run,
                1,
                "\"다시보기 $($Host.UI.RawUI.WindowSize.Width)폭\"\r".into(),
            )
            .unwrap();
        thread::sleep(Duration::from_secs(2));
        let (sink, replay) = collector();
        sessions.attach("t", 3, sink).unwrap();
        assert!(
            sessions.attach("t", 2, collector().0).is_err(),
            "늦은 이전 화면"
        );
        assert!(wait_for(&replay, "다시보기 90폭"), "{}", text(&replay));
        assert!(
            replay.lock().unwrap().iter().any(|c| c.is_empty()),
            "재생 끝 표시"
        );

        // 이전 실행의 늦은 입력은 새 실행의 셸에 들어가면 안 된다.
        sessions.write("t", run - 1, 3, "exit 3\r".into()).unwrap();
        sessions.write("t", run, 3, "exit 7\r".into()).unwrap();
        assert_eq!(
            exit_rx.recv_timeout(Duration::from_secs(30)).unwrap(),
            (run, 7)
        );
        sessions.kill("t");
        assert!(sessions.write("t", run, 3, b"x".to_vec()).is_err());
        sessions.kill_all();
        let cmd = shell::command("powershell", cwd.to_str().unwrap(), None)
            .unwrap()
            .0;
        assert!(
            sessions
                .start("late", cmd, 80, 24, None, |_, _| {})
                .is_err()
        );
    }

    #[test]
    fn kill_ends_a_running_shell() {
        let sessions = Sessions::default();
        let cwd = std::env::temp_dir();
        let (exit_tx, exit_rx) = channel();
        sessions
            .start(
                "k",
                shell::command("powershell", cwd.to_str().unwrap(), None)
                    .unwrap()
                    .0,
                80,
                24,
                None,
                move |_, code| exit_tx.send(code).unwrap(),
            )
            .unwrap();
        sessions.kill("k");
        assert!(exit_rx.recv_timeout(Duration::from_secs(30)).is_ok());
    }
}
