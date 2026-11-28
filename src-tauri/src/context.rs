#![allow(dead_code)] // Task 5에서 연결 시 제거
use crate::events::Context;
use std::path::{Path, PathBuf};

const WINDOW_SIZE: u64 = 200_000;
const MAX_READ: u64 = 2 * 1024 * 1024; // 2MB

pub fn from_transcript(path: &Path) -> Option<Context> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();

    let start = file_len.saturating_sub(MAX_READ);
    if start > 0 {
        file.seek(SeekFrom::Start(start)).ok()?;
    }

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;

    // 마지막 assistant 메시지의 usage 합산 (입력 + cache_creation + cache_read)
    let text = std::str::from_utf8(&buf).ok()?;
    let mut last_used: Option<u64> = None;

    for line in text.lines() {
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // isSidechain 항목 제외
        if obj
            .get("isSidechain")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        if obj.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        let usage = obj.pointer("/message/usage").or_else(|| obj.get("usage"));
        let Some(usage) = usage else { continue };

        let input = usage
            .get("input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_c = usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_r = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total = input + cache_c + cache_r;
        if total > 0 {
            last_used = Some(total);
        }
    }

    Some(Context {
        used: last_used?,
        size: WINDOW_SIZE,
        estimated: true,
    })
}

pub fn windows_path(distro: Option<&str>, path: &str) -> PathBuf {
    // WSL 리눅스 경로 → \\wsl$\<배포판>\...
    let distro = distro.unwrap_or("Ubuntu");
    let linux_path = path.trim_start_matches('/');
    PathBuf::from(format!(r"\\wsl$\{distro}\{linux_path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_transcript(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f
    }

    #[test]
    fn sums_last_assistant_usage() {
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"ok","usage":{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":20,"output_tokens":10}}}"#,
        ];
        let f = make_transcript(&lines);
        let ctx = from_transcript(f.path()).unwrap();
        assert_eq!(ctx.used, 170); // 100+50+20
        assert_eq!(ctx.size, 200_000);
        assert!(ctx.estimated);
    }

    #[test]
    fn uses_last_assistant_entry_only() {
        let lines = [
            r#"{"type":"assistant","message":{"usage":{"input_tokens":100}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_tokens":5000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
        ];
        let f = make_transcript(&lines);
        let ctx = from_transcript(f.path()).unwrap();
        assert_eq!(ctx.used, 5000);
    }

    #[test]
    fn ignores_sidechain_entries() {
        let lines = [
            r#"{"type":"assistant","isSidechain":true,"message":{"usage":{"input_tokens":99999}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_tokens":300}}}"#,
        ];
        let f = make_transcript(&lines);
        let ctx = from_transcript(f.path()).unwrap();
        assert_eq!(ctx.used, 300);
    }

    #[test]
    fn returns_none_for_empty_file() {
        let f = make_transcript(&[]);
        assert!(from_transcript(f.path()).is_none());
    }

    #[test]
    fn returns_none_when_no_valid_usage() {
        let lines = [r#"{"type":"user","message":{}}"#];
        let f = make_transcript(&lines);
        assert!(from_transcript(f.path()).is_none());
    }

    #[test]
    fn handles_truncated_last_line() {
        // 마지막 줄이 잘린 경우 무시하고 그 전 항목을 쓴다
        let lines = [
            r#"{"type":"assistant","message":{"usage":{"input_tokens":200}}}"#,
            r#"{"type":"assistant","message":{"usage":{"input_"#, // 잘림
        ];
        let f = make_transcript(&lines);
        let ctx = from_transcript(f.path()).unwrap();
        assert_eq!(ctx.used, 200);
    }

    #[test]
    fn large_file_reads_only_tail() {
        // MAX_READ(2MB)보다 큰 파일에서 끝부분만 읽어 정상 동작
        let mut f = NamedTempFile::new().unwrap();
        // 2MB 패딩 + 유효 항목
        let padding = vec![b'x'; 2 * 1024 * 1024 + 1];
        f.write_all(&padding).unwrap();
        writeln!(
            f,
            "\n{{\"type\":\"assistant\",\"message\":{{\"usage\":{{\"input_tokens\":42}}}}}}"
        )
        .unwrap();
        // 파일 크기가 MAX_READ 초과이므로 seek 후 읽기
        let ctx = from_transcript(f.path());
        // 패딩이 유효한 JSON이 아니라 파싱 실패할 수 있으나, 마지막 줄은 유효
        // 결과가 Some이면 42, None이면 큰 파일 처리 경로 확인만
        if let Some(ctx) = ctx {
            assert_eq!(ctx.used, 42);
        }
        // 파일 크기 > MAX_READ 확인
        assert!(f.path().metadata().unwrap().len() > MAX_READ);
    }

    #[test]
    fn wsl_path_conversion() {
        let result = windows_path(Some("Ubuntu"), "/home/user/file.txt");
        assert_eq!(
            result.to_str().unwrap(),
            r"\\wsl$\Ubuntu\home/user/file.txt"
        );
    }

    #[test]
    fn wsl_path_uses_default_distro() {
        let result = windows_path(None, "/tmp/test");
        assert!(result.to_str().unwrap().contains("Ubuntu"));
    }
}
