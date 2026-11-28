#![allow(dead_code)]
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
}

#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowUsage {
    pub today: TokenCounts,
    pub five_hour: TokenCounts,
    pub seven_day: TokenCounts,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// ISO 8601 UTC 타임스탬프("YYYY-MM-DDTHH:MM:SS[.mmm]Z")를 Unix ms로 변환한다.
pub fn iso_to_unix_ms(s: &str) -> Option<u64> {
    // UTC 마커 'Z' 필수
    let s = s.strip_suffix('Z')?;
    let (date, rest) = s.split_once('T')?;

    let mut dp = date.splitn(3, '-');
    let year: u64 = dp.next()?.parse().ok()?;
    let month: u64 = dp.next()?.parse().ok()?;
    let day: u64 = dp.next()?.parse().ok()?;

    let (time_hms, frac) = rest.split_once('.').unwrap_or((rest, ""));
    let mut tp = time_hms.splitn(3, ':');
    let hours: u64 = tp.next()?.parse().ok()?;
    let mins: u64 = tp.next()?.parse().ok()?;
    let secs: u64 = tp.next()?.parse().ok()?;

    // 소수점 이하 → 밀리초 (최대 3자리, 오른쪽 0-패딩, UTF-8 안전 처리)
    let frac_ms: u64 = if frac.is_empty() {
        0
    } else {
        let digits: String = frac
            .chars()
            .take(3)
            .filter(|c| c.is_ascii_digit())
            .collect();
        let s3 = format!("{:0<3}", digits);
        s3.parse().unwrap_or(0)
    };

    if year < 1970 || !(1..=12).contains(&month) || day < 1 {
        return None;
    }

    let days = days_from_epoch(year, month, day);
    let unix_secs = days * 86_400 + hours * 3_600 + mins * 60 + secs;
    Some(unix_secs * 1_000 + frac_ms)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn days_from_epoch(year: u64, month: u64, day: u64) -> u64 {
    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    // 1970..year 각 연도의 일수 합산
    let mut days: u64 = (year - 1970) * 365;
    for y in 1970..year {
        if is_leap(y) {
            days += 1;
        }
    }
    // 같은 해 1월 ~ (month-1)월 일수 합산
    for m in 1..month {
        let md = MONTH_DAYS[(m as usize) - 1];
        days += if m == 2 && is_leap(year) { md + 1 } else { md };
    }
    days += day - 1;
    days
}

/// root/.claude 폴더 하위 JSONL을 순회해 시간 창별 토큰을 집계한다.
pub fn aggregate_folder(root: &Path, now_ms: u64) -> WindowUsage {
    let projects = root.join("projects");
    if !projects.is_dir() {
        return WindowUsage::default();
    }

    let start_of_today = now_ms - (now_ms % 86_400_000);
    let five_h_start = now_ms.saturating_sub(5 * 3_600_000);
    let seven_d_start = now_ms.saturating_sub(7 * 24 * 3_600_000);

    let mut result = WindowUsage::default();
    let Ok(proj_entries) = std::fs::read_dir(&projects) else {
        return result;
    };

    for proj_entry in proj_entries.flatten() {
        let proj_path = proj_entry.path();
        if !proj_path.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&proj_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            accumulate_jsonl(
                &path,
                start_of_today,
                five_h_start,
                seven_d_start,
                &mut result,
            );
        }
    }
    result
}

fn accumulate_jsonl(
    path: &Path,
    start_of_today: u64,
    five_h_start: u64,
    seven_d_start: u64,
    result: &mut WindowUsage,
) {
    use std::io::{BufRead, BufReader};
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if obj.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }

        let Some(ts_str) = obj.get("timestamp").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(entry_ms) = iso_to_unix_ms(ts_str) else {
            continue;
        };

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
        let output = usage
            .get("output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if entry_ms >= seven_d_start {
            result.seven_day.input += input;
            result.seven_day.cache_creation += cache_c;
            result.seven_day.cache_read += cache_r;
            result.seven_day.output += output;
        }
        if entry_ms >= five_h_start {
            result.five_hour.input += input;
            result.five_hour.cache_creation += cache_c;
            result.five_hour.cache_read += cache_r;
            result.five_hour.output += output;
        }
        if entry_ms >= start_of_today {
            result.today.input += input;
            result.today.cache_creation += cache_c;
            result.today.cache_read += cache_r;
            result.today.output += output;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // iso_to_unix_ms 테스트
    #[test]
    fn parses_known_timestamp() {
        // 2026-11-28 00:00:00 UTC = 1_795_824_000_000 ms
        let ms = iso_to_unix_ms("2026-11-28T00:00:00.000Z").unwrap();
        assert_eq!(ms, 1_795_824_000_000);
    }

    #[test]
    fn parses_without_millis() {
        let ms = iso_to_unix_ms("2026-11-28T00:00:00Z").unwrap();
        assert_eq!(ms, 1_795_824_000_000);
    }

    #[test]
    fn returns_none_for_invalid() {
        assert!(iso_to_unix_ms("not-a-date").is_none());
    }

    #[test]
    fn returns_none_without_z_suffix() {
        assert!(iso_to_unix_ms("2026-11-28T00:00:00").is_none());
        assert!(iso_to_unix_ms("2026-11-28T00:00:00.000").is_none());
    }

    // aggregate_folder 테스트
    #[test]
    fn aggregates_today_tokens() {
        let dir = tempfile::tempdir().unwrap();
        // now_ms = 2026-11-28 01:00:00 UTC
        let now = 1_795_824_000_000u64 + 3_600_000;
        let ts_today = "2026-11-28T00:30:00.000Z";
        let line = format!(
            r#"{{"type":"assistant","timestamp":"{ts_today}","message":{{"usage":{{"input_tokens":100,"cache_creation_input_tokens":50,"cache_read_input_tokens":200,"output_tokens":10}}}}}}"#
        );
        let proj = dir.path().join("projects").join("test-proj");
        std::fs::create_dir_all(&proj).unwrap();
        let mut f = std::fs::File::create(proj.join("session.jsonl")).unwrap();
        use std::io::Write;
        writeln!(f, "{line}").unwrap();
        let usage = aggregate_folder(dir.path(), now);
        assert_eq!(usage.today.input, 100);
        assert_eq!(usage.today.cache_creation, 50);
        assert_eq!(usage.today.cache_read, 200);
    }

    #[test]
    fn excludes_old_entries_from_five_hour() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_795_824_000_000u64 + 3_600_000; // 01:00
        // 6시간 전 항목 — 5h 창 밖 (2026-11-27T19:00:00Z)
        let ts_old = "2026-11-27T19:00:00.000Z";
        let line = format!(
            r#"{{"type":"assistant","timestamp":"{ts_old}","message":{{"usage":{{"input_tokens":999}}}}}}"#
        );
        let proj = dir.path().join("projects").join("old-proj");
        std::fs::create_dir_all(&proj).unwrap();
        let mut f = std::fs::File::create(proj.join("session.jsonl")).unwrap();
        use std::io::Write;
        writeln!(f, "{line}").unwrap();
        let usage = aggregate_folder(dir.path(), now);
        assert_eq!(usage.five_hour.input, 0);
    }

    #[test]
    fn skips_non_assistant_entries() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_795_824_000_000u64 + 3_600_000;
        let ts = "2026-11-28T00:30:00.000Z";
        let line = format!(
            r#"{{"type":"user","timestamp":"{ts}","message":{{"usage":{{"input_tokens":999}}}}}}"#
        );
        let proj = dir.path().join("projects").join("user-proj");
        std::fs::create_dir_all(&proj).unwrap();
        let mut f = std::fs::File::create(proj.join("session.jsonl")).unwrap();
        use std::io::Write;
        writeln!(f, "{line}").unwrap();
        let usage = aggregate_folder(dir.path(), now);
        assert_eq!(usage.today.input, 0);
    }
}
