# Stage 5 — 계정 사용량 30초 갱신 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 30초마다 로컬 JSONL 로그를 집계해 계정별 오늘·5h·7d 토큰 사용량을 헤더 미니바와 계정 패널에 표시한다.

**Architecture:** Rust 백엔드(`usage.rs`, `account.rs`)가 `~/.claude` 폴더를 순회해 JSONL을 집계하고, `account_list` Tauri 명령으로 프론트에 전달한다. 30초 타이머는 `"usage-tick"` 이벤트만 발송하고 실제 집계는 프론트가 `account_list`를 호출해서 처리한다.

**Tech Stack:** Tauri 2, Rust (std only, no chrono), React + TypeScript, vitest

**Spec:** `docs/spec.md` §3 계정 사용량 절

## Global Constraints

- git 명령 금지 (사용자 지시 시만)
- 추측 수치는 반드시 "추정" 표시
- 공식 한도 모름 → `estimated: true`, 기준: 5h=500K, 주간=2.5M (설정 변경 가능 — 7단계)
- `docs/mockups/Accounts.dc.html` 색·간격 그대로
- `docs/mockups/Main.dc.html` 헤더 미니바 그대로 (56px wide, 4px height)
- JSONL `timestamp` 필드: ISO 8601 UTC 문자열 (`"YYYY-MM-DDTHH:MM:SS[.mmm]Z"`)
- WSL 배포판 감지: 기존 `shell::wsl_distros()` 재사용
- 추가 의존성 없음 (chrono 불필요 — ISO 파서 직접 구현)

## Review Focus

1. JSONL `timestamp` 필드가 없거나 형식이 다를 때(e.g. `null`, `"2026-11-28"`) — 항목 건너뜀, 집계에서 제외해야 함
2. WSL `\\wsl$\<distro>\home\` 하위에 사용자 디렉터리가 여럿일 때 — 모두 순회, 각각 별도 경로로 집계
3. `~/.claude` 폴더가 없거나 접근 불가일 때 — `WindowUsage::default()` 반환, 오류 전파 없음
4. 타이머 스레드가 앱 종료보다 오래 살 때 — `app.app_handle()` drop 이후 emit 실패를 무시
5. 계정이 0개일 때 헤더에서 사용량 클러스터가 보여야 하는지 — 빈 계정 목록이면 헤더 클러스터 숨김

---

### Task 1: usage.rs — JSONL 집계

**Files:**
- Create: `src-tauri/src/usage.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct TokenCounts { pub input: u64, pub output: u64, pub cache_creation: u64, pub cache_read: u64 }
  pub struct WindowUsage { pub today: TokenCounts, pub five_hour: TokenCounts, pub seven_day: TokenCounts }
  pub fn aggregate_folder(root: &Path, now_ms: u64) -> WindowUsage
  pub fn now_ms() -> u64  // SystemTime::now() ms since epoch
  ```

- [ ] **Step 1: 실패 테스트 작성 — iso_to_unix_ms**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_known_timestamp() {
        // 2026-11-28 00:00:00 UTC = 1795046400000 ms
        let ms = iso_to_unix_ms("2026-11-28T00:00:00.000Z").unwrap();
        assert_eq!(ms, 1_795_046_400_000);
    }
    #[test]
    fn parses_without_millis() {
        let ms = iso_to_unix_ms("2026-11-28T00:00:00Z").unwrap();
        assert_eq!(ms, 1_795_046_400_000);
    }
    #[test]
    fn returns_none_for_invalid() {
        assert!(iso_to_unix_ms("not-a-date").is_none());
    }
}
```

- [ ] **Step 2: 테스트 실행 (실패 확인)**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test usage -- --nocapture 2>&1
```
Expected: FAIL (function not defined)

- [ ] **Step 3: iso_to_unix_ms + days_from_epoch 구현**

`iso_to_unix_ms(s: &str) -> Option<u64>` — `"YYYY-MM-DDTHH:MM:SS[.mmm]Z"` 파싱.
`days_from_epoch(y: u64, m: u64, d: u64) -> u64` — 1970-01-01 기준 일수.

알고리즘:
```
days = (year-1970)*365 + leap_count_since_1970(year) + month_days_sum + (day-1)
unix_ms = (days*86400 + hours*3600 + mins*60 + secs) * 1000 + frac_ms
```

- [ ] **Step 4: 테스트 통과 확인**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test usage::tests::parses -- --nocapture 2>&1
```
Expected: PASS

- [ ] **Step 5: 실패 테스트 작성 — aggregate_folder**

```rust
#[test]
fn aggregates_today_tokens() {
    let dir = tempfile::tempdir().unwrap();
    // 오늘 00:30 항목 (input=100, cache_creation=50, output=10)
    // now_ms = 2026-11-28 01:00:00 UTC
    let now = 1_795_046_400_000u64 + 3600_000;
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
    let now = 1_795_046_400_000u64 + 3600_000; // 01:00
    // 6시간 전 항목 — 5h 창 밖
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
```

- [ ] **Step 6: 테스트 실행 (실패 확인)**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test usage::tests::aggregates -- --nocapture 2>&1
```
Expected: FAIL

- [ ] **Step 7: aggregate_folder 구현**

`aggregate_folder(root: &Path, now_ms: u64) -> WindowUsage` — `root/projects/*/` 하위 `.jsonl` 파일 순회, `type: "assistant"` + `timestamp` 파싱, 창별 토큰 합산.

창 경계:
- today: `entry_ms >= start_of_today_ms` (= `now_ms - now_ms % 86_400_000`)
- five_hour: `entry_ms >= now_ms - 5*3600*1000`
- seven_day: `entry_ms >= now_ms - 7*24*3600*1000`

usage 경로: `entry["message"]["usage"]` 우선, 없으면 `entry["usage"]`.

- [ ] **Step 8: 테스트 통과 확인**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test usage -- --nocapture 2>&1
```
Expected: all PASS

- [ ] **Step 9: `cargo fmt && cargo clippy -- -D warnings`**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -5
```
Expected: no warnings

---

### Task 2: account.rs — AccountRegistry

**Files:**
- Create: `src-tauri/src/account.rs`

**Interfaces:**
- Consumes: `usage::{aggregate_folder, WindowUsage, now_ms}`
- Produces:
  ```rust
  pub const FIVE_HOUR_LIMIT: u64 = 500_000;
  pub const WEEKLY_LIMIT: u64 = 2_500_000;
  pub struct AccountLimits { pub five_hour: u64, pub weekly: u64, pub estimated: bool }
  pub struct AccountSummary { pub name: String, pub paths: Vec<String>, pub usage: WindowUsage, pub limits: AccountLimits }
  pub struct AccountRegistry { accounts: Vec<(String, Vec<std::path::PathBuf>)> }
  impl AccountRegistry {
      pub fn new() -> Self   // Windows .claude + WSL .claude 자동 감지
      pub fn all_summaries(&self, now_ms: u64) -> Vec<AccountSummary>
  }
  ```

- [ ] **Step 1: 실패 테스트 작성**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_new_includes_windows_claude() {
        let reg = AccountRegistry::new();
        // Windows %USERPROFILE%\.claude가 존재하면 적어도 1개 계정
        // 없으면 0개 — 오류 없이 빈 목록
        let _ = reg.all_summaries(crate::usage::now_ms());
        // 오류가 없으면 통과
    }
    #[test]
    fn all_summaries_returns_estimated_limits() {
        let dir = tempfile::tempdir().unwrap();
        let reg = AccountRegistry {
            accounts: vec![("테스트".to_string(), vec![dir.path().to_path_buf()])],
        };
        let summaries = reg.all_summaries(crate::usage::now_ms());
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].limits.estimated);
        assert_eq!(summaries[0].limits.five_hour, FIVE_HOUR_LIMIT);
    }
}
```

- [ ] **Step 2: 테스트 실행 (실패 확인)**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test account -- --nocapture 2>&1
```
Expected: FAIL

- [ ] **Step 3: AccountRegistry 구현**

`new()`:
1. Windows: `%USERPROFILE%\.claude` → ("기본 계정", [path])
2. WSL: `crate::shell::wsl_distros()`로 배포판 목록, 각 배포판의 `\\wsl$\<distro>\home\` 하위 사용자 폴더 중 `.claude` 존재하면 추가

`all_summaries(now_ms)`: 각 계정의 paths를 aggregate_folder로 집계하고 합산.

- [ ] **Step 4: 테스트 통과 확인**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test account -- --nocapture 2>&1
```
Expected: PASS

- [ ] **Step 5: `cargo fmt && cargo clippy -- -D warnings`**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -5
```
Expected: no warnings

---

### Task 3: lib.rs + build.rs + capabilities — account_list 명령 + 타이머

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: `account::{AccountRegistry, AccountSummary, AccountLimits}`, `usage::WindowUsage`
- Produces Tauri 명령: `account_list() -> Vec<AccountSummaryPayload>`
- Produces Tauri 이벤트: `"usage-tick"` (페이로드 없음, 30s 주기)

페이로드 타입:
```rust
#[derive(serde::Serialize, Clone)]
struct TokenCountsPayload { input: u64, output: u64, cache_creation: u64, cache_read: u64 }
#[derive(serde::Serialize, Clone)]
struct WindowUsagePayload { today: TokenCountsPayload, five_hour: TokenCountsPayload, seven_day: TokenCountsPayload }
#[derive(serde::Serialize, Clone)]
struct AccountLimitsPayload { five_hour: u64, weekly: u64, estimated: bool }
#[derive(serde::Serialize, Clone)]
struct AccountSummaryPayload { name: String, paths: Vec<String>, usage: WindowUsagePayload, limits: AccountLimitsPayload }
```

- [ ] **Step 1: build.rs에 `"account_list"` 추가**

COMMANDS 배열에 `"account_list"` 추가.

- [ ] **Step 2: capabilities/default.json에 `"allow-account-list"` 추가**

permissions 배열에 추가.

- [ ] **Step 3: lib.rs 수정**

1. `mod account;` 추가
2. `AccountRegistry` managed state 등록 (`Mutex<AccountRegistry>`)
3. `account_list` 명령 구현
4. `run()` 함수에 `.manage(Mutex::new(AccountRegistry::new()))` 추가
5. 30초 타이머 스레드: `app.app_handle()` clone → 별도 thread에서 30s sleep loop, `app.emit("usage-tick", ())` 호출

- [ ] **Step 4: 빌드 통과 확인**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo build 2>&1 | tail -10
```
Expected: `Compiling lmux ...` → `Finished`

- [ ] **Step 5: 전체 테스트**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test 2>&1 | tail -10
```
Expected: all tests pass

---

### Task 4: api.ts + App.tsx — 타입·훅·헤더 미니바

**Files:**
- Modify: `src/api.ts`
- Modify: `src/App.tsx`

**Interfaces:**
- Produces TS 타입:
  ```typescript
  export type TokenCounts = { input: number; output: number; cacheCreation: number; cacheRead: number }
  export type WindowUsage = { today: TokenCounts; fiveHour: TokenCounts; sevenDay: TokenCounts }
  export type AccountLimits = { fiveHour: number; weekly: number; estimated: boolean }
  export type AccountSummary = { name: string; paths: string[]; usage: WindowUsage; limits: AccountLimits }
  ```
- `api.accountList: () => invoke<AccountSummary[]>("account_list")`

헤더 미니바 (Main.dc.html 기준):
- 계정별: 이름 + 56px×4px 막대 (5h 사용 %) + 퍼센트 숫자
- 구분선 `|` + "N초 후" 카운트다운

- [ ] **Step 1: api.ts 타입 + accountList 추가**

기존 api 객체에 `accountList: () => invoke<AccountSummary[]>("account_list")` 추가.

- [ ] **Step 2: App.tsx — usages state + usage-tick 리스너**

```typescript
const [usages, setUsages] = useState<AccountSummary[]>([]);
const [tickSecs, setTickSecs] = useState(30);

useEffect(() => {
  api.accountList().then(setUsages);
  const unlisten = listen<void>("usage-tick", () => {
    setTickSecs(30);
    api.accountList().then(setUsages);
  });
  const countdown = setInterval(() => setTickSecs(s => Math.max(0, s - 1)), 1000);
  return () => { unlisten.then(f => f()); clearInterval(countdown); };
}, []);
```

- [ ] **Step 3: 헤더 미니바 렌더링**

기존 헤더 `<div className="brand">` 옆에 추가:
```tsx
{usages.length > 0 && (
  <div className="usage-cluster">
    {usages.map((acc, i) => {
      const pct = acc.limits.fiveHour > 0
        ? Math.round((acc.usage.fiveHour.input + acc.usage.fiveHour.cacheCreation + acc.usage.fiveHour.cacheRead) / acc.limits.fiveHour * 100)
        : 0;
      return (
        <Fragment key={acc.name}>
          {i > 0 && <span className="usage-sep" />}
          <span className="usage-label">{acc.name}</span>
          <span className="usage-mini"><span style={{ width: `${pct}%` }} /></span>
          <span className="usage-pct mono">{pct}%</span>
        </Fragment>
      );
    })}
    <span className="usage-tick mono">· {tickSecs}초</span>
  </div>
)}
```

- [ ] **Step 4: styles.css에 usage-cluster 스타일 추가**

Main.dc.html 기준:
```css
.usage-cluster { display:flex;align-items:center;gap:6px;background:#161A21;border-radius:9px;padding:6px 10px;font-size:12px }
.usage-label { color:#A0A8B8 }
.usage-mini { height:4px;width:56px;border-radius:2px;background:#262B36;overflow:hidden;flex:none }
.usage-mini>span { display:block;height:100%;border-radius:2px;background:#6EA8FF }
.usage-pct { font-family:'JetBrains Mono',monospace }
.usage-sep { width:1px;height:14px;background:#2A303B;margin:0 2px }
.usage-tick { color:#8A93A5;font-size:11px;margin-left:4px }
```

- [ ] **Step 5: vitest 빌드 + 타입 체크**

```
cd C:\dev\dhkang\lmux && npm run typecheck 2>&1 | tail -10
```
Expected: 오류 없음

---

### Task 5: AccountsPanel.tsx — 계정 전체 화면

**Files:**
- Create: `src/AccountsPanel.tsx`
- Modify: `src/App.tsx` (계정 클러스터 클릭 → 패널 열기)
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `AccountSummary` from api.ts
- Props:
  ```typescript
  interface AccountsPanelProps {
    usages: AccountSummary[];
    tickSecs: number;
    onRefresh: () => void;
    onClose: () => void;
  }
  ```

UI 요소 (Accounts.dc.html 기준):
- 헤더: "계정 · 사용량" + 갱신 시각 + "지금 갱신" 버튼
- 계정 카드: 이름·아바타·경로 목록, 5h 바, 주간 바, 오늘 토큰 3칸(입력/출력/캐시읽기)
- 토큰 포맷: `formatTokens(n)` → `"1.8M"`, `"212K"`, `"640"` 등
- 한도 표시: `estimated` 이면 "(추정)" 추가

- [ ] **Step 1: formatTokens 테스트**

```typescript
// AccountsPanel.test.tsx
import { formatTokens } from "./AccountsPanel";
test("formats millions", () => expect(formatTokens(1_800_000)).toBe("1.8M"));
test("formats thousands", () => expect(formatTokens(212_000)).toBe("212K"));
test("formats small", () => expect(formatTokens(500)).toBe("500"));
```

- [ ] **Step 2: 테스트 실행 (실패 확인)**

```
cd C:\dev\dhkang\lmux && npx vitest run src/AccountsPanel.test.tsx 2>&1 | tail -10
```
Expected: FAIL

- [ ] **Step 3: AccountsPanel.tsx 구현**

export `formatTokens`, export default `AccountsPanel`.

- [ ] **Step 4: 테스트 통과 확인**

```
cd C:\dev\dhkang\lmux && npx vitest run src/AccountsPanel.test.tsx 2>&1 | tail -10
```
Expected: PASS

- [ ] **Step 5: App.tsx에 AccountsPanel 연결**

Modal 타입에 `{ type: "accounts" }` 추가, 헤더 usage-cluster 클릭 시 열기, 닫기 핸들러.

- [ ] **Step 6: 전체 vitest**

```
cd C:\dev\dhkang\lmux && npx vitest run 2>&1 | tail -10
```
Expected: all pass

---

### Task 6: 빌드 + 직접 실행 확인

- [ ] **Step 1: Rust 전체 빌드**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo build 2>&1 | tail -5
```
Expected: `Finished`

- [ ] **Step 2: 전체 테스트 (Rust + TS)**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test 2>&1 | tail -5
cd C:\dev\dhkang\lmux && npx vitest run 2>&1 | tail -5
```
Expected: all pass

- [ ] **Step 3: dev 서버 빌드 확인**

```
cd C:\dev\dhkang\lmux && npm run build 2>&1 | tail -5
```
Expected: build succeeded

- [ ] **Step 4: `docs/progress.md` 5단계 항목 추가**

한 일, 바뀐 파일, 검증 결과 기록.

---

### Task 7: Codex 교차검증 + 스냅샷

- [ ] **Step 1: Codex 검증 (orca-cli 스킬 사용)**

변경 파일: `usage.rs`, `account.rs`, `lib.rs` (account 부분), `api.ts`, `App.tsx`, `AccountsPanel.tsx`
요청: 버그·명세 불일치·보안(셸 접근·파일 읽기)·성능·놓친 예외를 심각도와 파일·줄 위치로 지적.
결과: `docs/reviews/stage-5.md`

- [ ] **Step 2: 지적 사항 수정 (높음·중간)**

- [ ] **Step 3: 재검증 (빌드·테스트)**

```
cd C:\dev\dhkang\lmux\src-tauri && cargo test 2>&1 | tail -5
cd C:\dev\dhkang\lmux && npx vitest run 2>&1 | tail -5
```

- [ ] **Step 4: 스냅샷 생성**

```powershell
Compress-Archive -Path C:\dev\dhkang\lmux\* -DestinationPath C:\dev\dhkang\lmux\backups\stage-5.zip -Exclude @("*\node_modules\*","*\target\*","*\dist\*") -Force
```
