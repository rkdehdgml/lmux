//! 화면에 보이지 않는 세션의 출력을 쌓아 두는 줄 단위 링버퍼.

use std::collections::{BTreeMap, VecDeque};

pub const DEFAULT_MAX_LINES: usize = 10_000;
const MAX_BYTES: usize = 8 * 1024 * 1024;
/// 커서 이동만 쓰는 TUI는 개행 없이 길게 이어지므로 이 길이에서 강제로 자른다.
const MAX_LINE: usize = 64 * 1024;

/// DEC 개인 모드(`ESC [ ? n h|l`)와 터미널 리셋(`ESC c`)만 읽는 작은 상태 파서.
/// 조각 경계에 걸친 시퀀스도 이어서 읽는다.
#[derive(Default)]
struct Modes {
    set: BTreeMap<u16, bool>,
    seq: Vec<u8>,
}

impl Modes {
    fn feed(&mut self, data: &[u8]) {
        for &b in data {
            if b == 0x1b {
                self.seq.clear();
                self.seq.push(b);
                continue;
            }
            match self.seq.len() {
                0 => {}
                1 if b == b'[' => self.seq.push(b),
                1 => {
                    if b == b'c' {
                        self.set.clear();
                    }
                    self.seq.clear();
                }
                2 if b == b'?' => self.seq.push(b),
                2 => self.seq.clear(),
                len if len < 64 && (b.is_ascii_digit() || b == b';') => self.seq.push(b),
                _ => {
                    if b == b'h' || b == b'l' {
                        for param in self.seq[3..].split(|&c| c == b';') {
                            let mode = std::str::from_utf8(param).ok().and_then(|p| p.parse().ok());
                            if let Some(mode) = mode {
                                self.set.insert(mode, b == b'h');
                            }
                        }
                    }
                    self.seq.clear();
                }
            }
        }
    }
}

pub struct Ring {
    lines: VecDeque<Vec<u8>>,
    bytes: usize,
    max_lines: usize,
    /// 밀려난 구간에서 바뀐 DEC 개인 모드(대체 화면, 붙여넣기 괄호 등). 재생 앞에 다시 붙인다.
    // shortcut: 스크롤 영역·색상 같은 다른 터미널 상태는 복원하지 않는다. 문제가 되면 VT 상태 체크포인트로 바꾼다.
    modes: Modes,
}

impl Ring {
    pub fn new(max_lines: usize) -> Self {
        Ring {
            lines: VecDeque::new(),
            bytes: 0,
            max_lines: max_lines.max(1),
            modes: Modes::default(),
        }
    }

    pub fn push(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let open = self
                .lines
                .back()
                .is_some_and(|l| l.last() != Some(&b'\n') && l.len() < MAX_LINE);
            if !open {
                self.lines.push_back(Vec::new());
            }
            let last = self.lines.back_mut().expect("line pushed above");
            let room = MAX_LINE - last.len();
            let take = data
                .iter()
                .take(room)
                .position(|&b| b == b'\n')
                .map_or(room.min(data.len()), |i| i + 1);
            last.extend_from_slice(&data[..take]);
            self.bytes += take;
            data = &data[take..];
            self.trim();
        }
    }

    fn trim(&mut self) {
        while self.lines.len() > 1 && (self.lines.len() > self.max_lines || self.bytes > MAX_BYTES)
        {
            if let Some(old) = self.lines.pop_front() {
                self.bytes -= old.len();
                self.modes.feed(&old);
            }
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bytes);
        for (mode, on) in &self.modes.set {
            let end = if *on { 'h' } else { 'l' };
            out.extend_from_slice(format!("\x1b[?{mode}{end}").as_bytes());
        }
        // 밀려난 구간 끝에 걸친 시퀀스의 앞부분. 첫 줄이 이어서 완성한다.
        out.extend_from_slice(&self.modes.seq);
        for line in &self.lines {
            out.extend_from_slice(line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_last_lines() {
        let mut r = Ring::new(3);
        for i in 0..10 {
            r.push(format!("line{i}\r\n").as_bytes());
        }
        assert_eq!(r.snapshot(), b"line7\r\nline8\r\nline9\r\n");
    }

    #[test]
    fn joins_chunks_of_one_line() {
        let mut r = Ring::new(2);
        r.push(b"ab");
        r.push(b"c\nde");
        r.push(b"f\n");
        assert_eq!(r.snapshot(), b"abc\ndef\n");
    }

    #[test]
    fn splits_and_bounds_output_without_newlines() {
        let mut r = Ring::new(DEFAULT_MAX_LINES);
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..12 {
            r.push(&chunk);
        }
        let len = r.snapshot().len();
        assert!(
            len <= MAX_BYTES && len > MAX_BYTES - 2 * MAX_LINE,
            "len={len}"
        );
    }

    #[test]
    fn terminal_reset_clears_remembered_modes() {
        let mut r = Ring::new(1);
        r.push(b"\x1b[?2004h\n\x1bc\n\x1b[?25l\na\n");
        assert_eq!(r.snapshot(), b"\x1b[?25la\n");
    }

    #[test]
    fn reads_a_mode_split_at_the_forced_line_break() {
        let mut r = Ring::new(1);
        let mut data = vec![b'x'; MAX_LINE - 3];
        data.extend_from_slice(b"\x1b[?2004h\nz\n");
        r.push(&data);
        assert_eq!(r.snapshot(), b"\x1b[?2004hz\n");
    }

    #[test]
    fn completes_a_mode_whose_start_alone_was_evicted() {
        let mut r = Ring::new(1);
        let mut data = vec![b'x'; MAX_LINE - 3];
        data.extend_from_slice(b"\x1b[?2004hz\n");
        r.push(&data);
        assert_eq!(r.snapshot(), b"\x1b[?2004hz\n");
    }

    #[test]
    fn restores_modes_set_in_evicted_lines() {
        let mut r = Ring::new(2);
        r.push(b"\x1b[?1049h\x1b[?25l\x1b[?2004;1004h\n\x1b[?25h\n");
        r.push(b"a\nb\n");
        assert_eq!(
            r.snapshot(),
            b"\x1b[?25h\x1b[?1004h\x1b[?1049h\x1b[?2004ha\nb\n"
        );
    }
}
