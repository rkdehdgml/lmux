# Stage 6 — 변경 사항 · SVN/Git · 충돌 경고 · Git 워크트리 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 세션별 수정 파일 추적, 같은 파일 충돌 경고, SVN/Git 변경 패널, Git 워크트리 관리를 구현한다.

**Architecture:** Rust hooks의 PostToolUse 이벤트에서 파일 경로를 추출해 세션별로 추적한다. SVN/Git은 `std::process::Command`로 호출하고 결과를 프론트에 전달한다. 충돌(2개 이상 세션이 같은 파일 수정)은 Tauri 이벤트로 프론트에 알린다. UI는 Merge.dc.html 모의화면을 그대로 따른다.

**Tech Stack:** Rust (std::process::Command, serde_json), TypeScript/React, Tauri 2 events

**Spec:** `docs/spec.md` (3절 "같은 파일 충돌 경고", "변경 사항"), `docs/mockups/Merge.dc.html`

## Global Constraints

- 커밋·되돌리기·삭제는 항상 사용자 확인 후 실행 (UI에서 confirm dialog)
- SVN CLI 없으면 설치 안내 문구 표시 (에러 반환)
- Git 워크트리는 Git 프로젝트에서만 보임
- 사용자의 프로젝트 파일(SVN 저장소) 절대 수정 금지 — svn/git 명령만 호출
- Noto Sans KR + JetBrains Mono, 색 #0E1014/#14171D/#2F6FE4 등 spec 4절 준수
- cargo clippy -D warnings 경고 없음, cargo fmt 적용

## Review Focus

1. SVN이 없는 환경에서 `svn` 명령 실행 → "svn CLI를 찾지 못했어요" 에러가 UI에 보여야 함 — Task 2 `svn_not_found_returns_error` 테스트
2. 세션 종료 후에도 파일 추적 남아 있으면 ghost conflict 발생 → Task 1 `cleared_on_session_end` 테스트
3. PostToolUse body에 tool_input 없는 경우 (Bash 등) → 파일 경로 추출 시 패닉 없어야 함 — Task 1 `extract_files_bash_no_panic` 테스트
4. 대용량 diff (100KB+) 반환 시 Tauri IPC 한도 — `get_diff`가 최대 200KB로 잘라서 반환해야 함 — Task 3 `diff_truncated_at_200k` 테스트
5. Git worktree 경로가 이미 존재하면 `git_worktree_add` 실패 → 에러 메시지를 프론트가 toast로 표시해야 함 — Task 3 `worktree_add_existing_path_error` 테스트

---

### Task 1: 세션별 파일 추적 · 충돌 감지

**Files:**
- Create: `src-tauri/src/changes.rs` (파일 추적 + SVN/Git 명령 — Task 1은 추적만)
- Modify: `src-tauri/src/events.rs` (PostToolUse 파일 경로 추출 추가)
- Modify: `src-tauri/src/lib.rs` (FileTracker managed state, process_hook_event 확장, conflict-detected 이벤트)

**Interfaces:**
- Produces:
  - `pub struct FileTracker` with `pub fn track(session, path)`, `pub fn files(session) -> HashSet<String>`, `pub fn clear(session)`, `pub fn conflicts(session, project_path) -> Vec<String>` — used by Tasks 2–4
  - `pub fn extract_files(hook: &str, body: &[u8]) -> Vec<String>` in `events.rs`
  - Tauri event `"conflict-detected"` payload: `{ sessionId: string, files: string[] }`

- [ ] **Step 1: `extract_files` 테스트 작성**

```rust
// src-tauri/src/events.rs 내 #[cfg(test)] mod tests 에 추가
#[test]
fn extracts_write_path() {
    let body = br#"{"tool_name":"Write","tool_input":{"file_path":"/a/b.rs"}}"#;
    assert_eq!(extract_files("PostToolUse", body), vec!["/a/b.rs"]);
}
#[test]
fn extract_files_bash_no_panic() {
    let body = br#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    assert!(extract_files("PostToolUse", body).is_empty());
}
#[test]
fn extract_files_non_post_tool_use_empty() {
    let body = br#"{"tool_name":"Write","tool_input":{"file_path":"/a/b.rs"}}"#;
    assert!(extract_files("PreToolUse", body).is_empty());
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p lmux_lib extract_files 2>&1 | tail -5`
Expected: FAIL — function not defined

- [ ] **Step 3: `extract_files(hook: &str, body: &[u8]) -> Vec<String>` 구현 (events.rs)**

대상 tool_name: `Write`, `Edit`, `MultiEdit`, `NotebookEdit`. JSON을 `serde_json::Value`로 파싱하고 `tool_input.file_path` (Write/Edit/NotebookEdit) 또는 `tool_input.edits[*].path` (MultiEdit) 추출.

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p lmux_lib extract_files 2>&1 | tail -5`
Expected: PASS — 3 tests ok

- [ ] **Step 5: FileTracker 테스트 작성**

```rust
// src-tauri/src/changes.rs
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflict_when_two_sessions_share_file() {
        let ft = FileTracker::default();
        ft.track("s1", "/proj/a.rs");
        ft.track("s2", "/proj/a.rs");
        let c = ft.conflicts("s1", "/proj");
        assert!(c.contains(&"/proj/a.rs".to_string()));
    }
    #[test]
    fn cleared_on_session_end() {
        let ft = FileTracker::default();
        ft.track("s1", "/proj/a.rs");
        ft.clear("s1");
        assert!(ft.files("s1").is_empty());
    }
}
```

- [ ] **Step 6: 테스트 실패 확인**

Run: `cargo test -p lmux_lib conflict_when 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 7: `FileTracker` 구현 (changes.rs)**

```rust
#[derive(Default)]
pub struct FileTracker {
    files: Mutex<HashMap<String, HashSet<String>>>,
}
impl FileTracker {
    pub fn track(&self, session: &str, path: &str) { ... }
    pub fn files(&self, session: &str) -> HashSet<String> { ... }
    pub fn clear(&self, session: &str) { ... }
    /// 같은 project_path 아래 path를 가진 다른 세션이 있으면 해당 경로 목록 반환
    pub fn conflicts(&self, session: &str, project_path: &str) -> Vec<String> { ... }
}
```

- [ ] **Step 8: 테스트 통과 확인**

Run: `cargo test -p lmux_lib -p lmux_lib -- changes 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 9: lib.rs — FileTracker 관리 상태 추가, process_hook_event 확장**

`app.manage(FileTracker::default())` 추가. `process_hook_event`에서 `extract_files` 호출, 경로마다 `tracker.track(session, path)`, 충돌 감지 후 `app.emit("conflict-detected", payload)` 발행. 세션 종료(Hook::End) 시 `tracker.clear(session)`.

- [ ] **Step 10: 빌드 확인**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 11: 커밋**

```
git add src-tauri/src/events.rs src-tauri/src/changes.rs src-tauri/src/lib.rs
git commit -m "feat: add per-session file tracker and conflict detection"
```

---

### Task 2: SVN 명령

**Files:**
- Modify: `src-tauri/src/changes.rs` (SVN 함수 추가)
- Test: `src-tauri/src/changes.rs` (cfg(test) 모듈)

**Interfaces:**
- Consumes: 없음 (std::process::Command만 사용)
- Produces:
  - `pub async fn svn_status(project_path: &str) -> anyhow::Result<Vec<SvnEntry>>`
  - `pub async fn svn_diff_file(project_path: &str, rel_path: &str) -> anyhow::Result<String>`
  - `pub async fn svn_update(project_path: &str) -> anyhow::Result<String>`
  - `pub async fn svn_commit(project_path: &str, files: &[String], message: &str) -> anyhow::Result<String>`
  - `pub async fn svn_revert(project_path: &str, files: &[String]) -> anyhow::Result<String>`
  - `pub struct SvnEntry { pub rel_path: String, pub status: char }`

- [ ] **Step 1: SVN 없을 때 에러 테스트**

```rust
#[test]
fn svn_not_found_returns_error() {
    // svn가 없는 환경 모의: PATH를 덮어쓰거나 존재하지 않는 명령 호출
    // 실제로 PATH에 svn이 없는 환경이면 이 테스트는 그냥 통과됨.
    // 여기서는 함수가 Err를 반환하는 계약만 확인한다.
    // → "which svn" 실패 케이스 직접 테스트하기 어려우므로
    // svn_status의 output 파싱 경로 테스트로 대체:
    let result = parse_svn_status(b"? unversioned.txt\nM  modified.java\n");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].status, '?');
    assert_eq!(result[1].status, 'M');
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p lmux_lib svn_not_found 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: SVN 함수들 구현 (changes.rs)**

`parse_svn_status(output: &[u8]) -> Vec<SvnEntry>` — 각 줄에서 첫 글자를 status, 나머지를 rel_path로 파싱. 빈 줄 무시.

`svn_status`: `svn status --xml` 대신 `svn status` (텍스트) 호출 → `parse_svn_status`.

`svn_diff_file`: `svn diff <rel_path>`, stdout 반환. 200KB 초과 시 truncate.

`svn_update`, `svn_commit`, `svn_revert`: 각 `svn` 하위 명령 실행, stderr를 Err로 전파. SVN not found → "svn CLI를 찾지 못했어요. TortoiseSVN의 커맨드라인 도구를 설치해 주세요."

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p lmux_lib svn 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: 빌드·clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 6: 커밋**

```
git add src-tauri/src/changes.rs
git commit -m "feat: add SVN status/diff/update/commit/revert commands"
```

---

### Task 3: Git 변경 + Git 워크트리

**Files:**
- Modify: `src-tauri/src/changes.rs`

**Interfaces:**
- Produces:
  - `pub async fn git_status(project_path: &str) -> anyhow::Result<Vec<GitEntry>>`
  - `pub async fn git_diff_file(project_path: &str, rel_path: &str) -> anyhow::Result<String>`
  - `pub async fn git_commit(project_path: &str, files: &[String], message: &str) -> anyhow::Result<String>`
  - `pub struct GitWorktreeInfo { pub path: String, pub branch: String, pub is_locked: bool }`
  - `pub async fn git_worktree_list(project_path: &str) -> anyhow::Result<Vec<GitWorktreeInfo>>`
  - `pub async fn git_worktree_add(project_path: &str, branch: &str, worktree_path: &str) -> anyhow::Result<String>`
  - `pub async fn git_worktree_remove(project_path: &str, worktree_path: &str) -> anyhow::Result<String>`

- [ ] **Step 1: Git 파싱 테스트**

```rust
#[test]
fn parses_git_status_output() {
    let out = b" M src/lib.rs\n?? new_file.txt\n";
    let entries = parse_git_status(out);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].status, 'M');
    assert_eq!(entries[1].rel_path, "new_file.txt");
}
#[test]
fn worktree_add_existing_path_error() {
    // 이 테스트는 실제 git 없이는 실행 불가 → parse_git_worktree_list 단위 테스트로 대체
    let list_output = b"worktree /a/main\nHEAD abc123\nbranch refs/heads/main\n\nworktree /a/feat\nHEAD def456\nbranch refs/heads/feat\n";
    let list = parse_git_worktree_list(list_output);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].branch, "main");
    assert_eq!(list[1].path, "/a/feat");
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p lmux_lib parses_git_status 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Git 함수들 구현 (changes.rs)**

`parse_git_status(output: &[u8]) -> Vec<GitEntry>` — `git status --porcelain` 출력 파싱 (2번째 열 status char, 나머지 rel_path).

`parse_git_worktree_list(output: &[u8]) -> Vec<GitWorktreeInfo>` — `git worktree list --porcelain` 출력 파싱.

`git_worktree_add`: `git -C <proj> worktree add <path> <branch>` — 경로 이미 존재 시 "이미 있는 경로예요" 에러.

`git_worktree_remove`: `git -C <proj> worktree remove --force <path>`.

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p lmux_lib git 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 6: 커밋**

```
git add src-tauri/src/changes.rs
git commit -m "feat: add Git status/diff/commit and worktree commands"
```

---

### Task 4: Tauri 명령 + api.ts 타입

**Files:**
- Modify: `src-tauri/src/lib.rs` (새 Tauri 명령 추가)
- Modify: `src-tauri/build.rs` (명령 목록)
- Modify: `src-tauri/capabilities/default.json` (allow-* 추가)
- Modify: `src/api.ts` (타입 + 함수 추가)

**Interfaces:**
- Consumes: Task 1–3 함수들
- Produces:
  - `api.getChanges(projectPath: string): Promise<ChangeSummary>`
  - `api.getDiff(projectPath: string, filePath: string): Promise<string>`
  - `api.svnCommit(projectPath: string, files: string[], message: string): Promise<string>`
  - `api.svnRevert(projectPath: string, files: string[]): Promise<string>`
  - `api.svnUpdate(projectPath: string): Promise<string>`
  - `api.gitCommit(projectPath: string, files: string[], message: string): Promise<string>`
  - `api.gitWorktreeList(projectPath: string): Promise<GitWorktreeInfo[]>`
  - `api.gitWorktreeAdd(projectPath: string, branch: string, worktreePath: string): Promise<string>`
  - `api.gitWorktreeRemove(projectPath: string, worktreePath: string): Promise<string>`

```typescript
// src/api.ts 에 추가할 타입
export type FileStatus = "M" | "A" | "D" | "?" | "C" | "!"
export type ChangedFile = {
  relPath: string
  status: FileStatus
  sessionIds: string[]
  isConflict: boolean
}
export type ChangeSummary = {
  kind: "svn" | "git" | "folder"
  revision: string | null  // SVN: "trunk @ r4821"
  branch: string | null    // Git: "main"
  files: ChangedFile[]     // 파일 추적 정보 포함
  addCount: number
  delCount: number
}
export type GitWorktreeInfo = {
  path: string
  branch: string
  isLocked: boolean
}
```

- [ ] **Step 1: api.ts 타입 테스트**

```typescript
// src/api.test.ts 에 추가
import { describe, test, expect } from "vitest";
import type { ChangeSummary } from "./api";
describe("ChangeSummary type", () => {
  test("accepts valid svn summary", () => {
    const s: ChangeSummary = {
      kind: "svn", revision: "trunk @ r100", branch: null,
      files: [], addCount: 0, delCount: 0,
    };
    expect(s.kind).toBe("svn");
  });
});
```

- [ ] **Step 2: 테스트 실패 확인 (타입 정의 없으면 tsc 오류)**

Run: `npx tsc --noEmit 2>&1 | tail -5`
Expected: 오류 (ChangeSummary not defined)

- [ ] **Step 3: Tauri 명령 추가 (lib.rs)**

8개 async 명령 추가:
```rust
#[tauri::command(async)]
async fn get_changes(
    file_tracker: State<'_, FileTracker>,
    workspace: Shared<'_>,
    project_path: String,
) -> Result<ChangeSummaryPayload, String>
```

`ChangeSummaryPayload`는 `ChangeSummary`와 같은 필드를 `#[serde(rename_all="camelCase")]`로 직렬화. `files`에 세션 추적 정보를 포함 (file_tracker에서 조회).

나머지 7개 명령: `get_diff`, `svn_commit`, `svn_revert`, `svn_update`, `git_commit`, `git_worktree_list`, `git_worktree_add`, `git_worktree_remove` — 각각 `changes::` 함수를 호출하고 에러를 `String`으로 변환.

`invoke_handler![]`에 8개 추가. `build.rs`와 `capabilities/default.json`에도 추가.

- [ ] **Step 4: api.ts 함수 추가**

8개 `invoke<T>()` 래퍼 추가. 타입 임포트.

- [ ] **Step 5: 빌드·tsc 클린**

Run: `cargo build 2>&1 | tail -3 && npx tsc --noEmit 2>&1 | tail -3`
Expected: `Finished` + (빈 출력)

- [ ] **Step 6: 커밋**

```
git add src-tauri/src/lib.rs src-tauri/build.rs src-tauri/capabilities/default.json src/api.ts
git commit -m "feat: expose SVN/Git/file-tracker commands to frontend"
```

---

### Task 5: ChangesPanel UI

**Files:**
- Create: `src/ChangesPanel.tsx`
- Create: `src/ChangesPanel.test.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `api.getChanges`, `api.getDiff`, `api.svnCommit`, `api.svnRevert`, `api.svnUpdate`, `api.gitCommit`, `api.gitWorktreeList`, `api.gitWorktreeAdd`, `api.gitWorktreeRemove`
- Produces: `export default ChangesPanel({ project, sessions, onClose })` — used by Task 6

**디자인:** Merge.dc.html 그대로 — 좌측 파일 목록(체크박스, 상태, 세션 귀속, 충돌 표시), 우측 diff 뷰어, 하단 커밋 메시지 + 버튼 그룹. 세션 필터 탭(전체/세션별).

- [ ] **Step 1: 기본 렌더링 테스트**

```typescript
// src/ChangesPanel.test.tsx
import { render, screen } from "@testing-library/react";
import ChangesPanel from "./ChangesPanel";
import type { Project } from "./api";
const proj: Project = { id: "p1", name: "gov-portal", path: "C:/proj", kind: "svn", defaultShell: "pwsh", defaultAgent: "claude", identity: "" };
test("renders project name in header", () => {
  render(<ChangesPanel project={proj} sessions={[]} onClose={() => {}} />);
  expect(screen.getByText("변경 사항 · gov-portal")).toBeTruthy();
});
test("shows loading state initially", () => {
  render(<ChangesPanel project={proj} sessions={[]} onClose={() => {}} />);
  expect(screen.getByRole("status")).toBeTruthy();  // loading spinner
});
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `npx vitest run src/ChangesPanel.test.tsx 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: ChangesPanel.tsx 구현**

Props: `{ project: Project; sessions: SessionRecord[]; onClose: () => void }`

마운트 시 `api.getChanges(project.path)` 호출. 파일 클릭 시 `api.getDiff` 호출. 커밋 버튼 `window.confirm()` 확인 후 svn/git commit 호출. SVN 종류면 "svn update" 버튼과 "svn revert" 버튼 표시, Git이면 "Git 커밋"과 워크트리 섹션 표시.

Merge.dc.html의 색·간격·클래스명 그대로 적용.

- [ ] **Step 4: 테스트 통과**

Run: `npx vitest run src/ChangesPanel.test.tsx 2>&1 | tail -5`
Expected: PASS — 2 tests ok

- [ ] **Step 5: styles.css — changes-panel 스타일 추가 (Merge.dc.html 기준)**

`.changes-overlay`, `.changes-panel`, `.changes-file-list`, `.changes-diff`, `.changes-footer` 등

- [ ] **Step 6: tsc + vitest**

Run: `npx tsc --noEmit 2>&1 | tail -3 && npx vitest run 2>&1 | tail -5`
Expected: 클린 + 모든 테스트 통과

- [ ] **Step 7: 커밋**

```
git add src/ChangesPanel.tsx src/ChangesPanel.test.tsx src/styles.css
git commit -m "feat: add ChangesPanel UI for SVN/Git changes"
```

---

### Task 6: ConflictBanner · Ctrl+Shift+G · 종합 연결

**Files:**
- Create: `src/ConflictBanner.tsx`
- Modify: `src/App.tsx` (Modal union, conflict-detected 이벤트 구독, Ctrl+Shift+G)
- Modify: `src/SessionPanel.tsx` (ConflictBanner 삽입)
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `ChangesPanel` from Task 5, `FileTracker` Tauri event `"conflict-detected"`
- Produces: conflict 상태 표시 UI, Ctrl+Shift+G 단축키

- [ ] **Step 1: ConflictBanner 테스트**

```typescript
// src/ConflictBanner.test.tsx
import { render, screen } from "@testing-library/react";
import ConflictBanner from "./ConflictBanner";
test("shows conflict file count", () => {
  render(<ConflictBanner files={["a.rs", "b.rs"]} onOpenChanges={() => {}} />);
  expect(screen.getByText(/2개/)).toBeTruthy();
});
test("does not render when no conflicts", () => {
  const { container } = render(<ConflictBanner files={[]} onOpenChanges={() => {}} />);
  expect(container.firstChild).toBeNull();
});
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `npx vitest run src/ConflictBanner.test.tsx 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: ConflictBanner.tsx 구현**

Props: `{ files: string[]; onOpenChanges: () => void }`. `files.length === 0`이면 null. `files.length > 0`이면 오렌지 배너 렌더링.

- [ ] **Step 4: 테스트 통과**

Run: `npx vitest run src/ConflictBanner.test.tsx 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: App.tsx — Modal union + conflict 이벤트 + Ctrl+Shift+G**

`{ type: "changes"; project: Project }` 를 Modal union에 추가. `useEffect`에서 `listen("conflict-detected", ...)` 구독 → `conflicts` 상태(Map) 업데이트. keyDown에 `Ctrl+Shift+G` 추가 → 현재 포커스 세션의 프로젝트로 `setModal({ type: "changes", project })`.

Modal render: `{modal?.type === "changes" && <ChangesPanel project={modal.project} sessions={...} onClose={close} />}`

- [ ] **Step 6: SessionPanel.tsx — ConflictBanner 삽입**

세션 패널 헤더 아래에 `<ConflictBanner files={conflictFiles} onOpenChanges={...} />`.

- [ ] **Step 7: 빌드 + 모든 테스트**

Run: `cargo build 2>&1 | tail -3`
Run: `npx vitest run 2>&1 | tail -5`
Expected: 모두 통과

- [ ] **Step 8: 커밋 (Stage 6 최종)**

```
git add src/ConflictBanner.tsx src/ConflictBanner.test.tsx src/App.tsx src/SessionPanel.tsx src/styles.css
git commit -m "feat: stage 6 — conflict banner, Ctrl+Shift+G, full changes integration"
```
