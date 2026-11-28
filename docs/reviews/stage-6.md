# Stage 6 Code Review — SVN/Git 변경 사항, 충돌 경고, 워크트리

검토 도구: code-reviewer 에이전트 (ecc:code-reviewer)  
대상 커밋: a866be1 ~ b7ea1db  
수정 커밋: 0103dfa

---

## 항목별 결과

### Finding 1 — CRITICAL: git/svn 서브프로세스 옵션 주입

**내용**: `git_diff_file`, `svn_diff_file`, `git_worktree_add` 에서 사용자 입력 경로/브랜치 앞에 `--` 구분자가 없어 `--exec=calc.exe` 형태의 git 옵션 주입이 가능했음.

**결과**: **수정함**  
`"diff", "--", rel_path` / `"worktree", "add", "--", worktree_path, branch` 형태로 `--` 구분자 추가. 커밋 0103dfa.

---

### Finding 2 — HIGH: `unsafe &'static` 원시 포인터 (shutdown 경쟁 가능)

**내용**: `lib.rs:664–668`에서 HookStates·FileTracker를 `*const T` → `&'static T`로 변환. HookServer 스레드가 Tauri 종료 중 managed state 해제 이후에도 이 참조를 역참조할 경우 use-after-free 발생 가능.

**결과**: **수정함**  
`Arc<HookStates>`, `Arc<FileTracker>`를 manage하고, 클로저에 Arc 클론을 이동해 소유권을 유지. `unsafe` 블록 완전 제거. 커밋 0103dfa.

---

### Finding 3 — HIGH: `String::truncate(MAX_DIFF_BYTES)` — 멀티바이트 패닉

**내용**: `String::truncate(n)`은 `n`이 멀티바이트 문자 내부를 가리키면 패닉. 한국어 파일명·내용이 포함된 diff에서 반드시 발생.

**결과**: **수정함**  
`s.floor_char_boundary(MAX_DIFF_BYTES)`로 안전한 경계를 먼저 구한 뒤 `s.truncate(cut)` 호출. `svn_diff_file`, `git_diff_file` 양쪽 모두 적용. 커밋 0103dfa.

---

### Finding 4 — HIGH: `handleRevert`가 내부 kind 검사 없이 항상 SVN revert 호출

**내용**: 버튼 렌더링 시 `project.kind === "svn"` 조건이 있으나 함수 자체에는 guard가 없어 향후 호출 경로 추가 시 git 프로젝트에서도 `svn revert`가 실행될 수 있음.

**결과**: **수정함**  
함수 첫 줄에 `if (project.kind !== "svn") return;` 추가. 커밋 0103dfa.

---

### Finding 5 — MEDIUM→LOW: FileTracker 락 경쟁 (재등급)

**내용**: `conflicts()`가 `files` 락을 보유한 채 반복하는 동안 `track()` 호출이 블로킹됨. 올바른 Mutex 동작이며 교착 없음.

**재등급**: 검토 에이전트가 LOW로 재등급. 현재 임계 구역 크기 상 성능 영향 없음.

**결과**: **수정 안 함** — 올바른 동작, 측정 가능한 문제 없음.

---

### Finding 6 — MEDIUM: `parse_git_worktree_list` detached HEAD 파싱

**내용**: `trim_start_matches("branch refs/heads/")`를 먼저 적용하고 `trim_start_matches("branch ")`를 뒤에 적용하면 `"branch (detached)"` 같은 비-refs 줄이 `"branch (detached)"` 그대로 남는 표시 버그.

**결과**: **수정함**  
`strip_prefix("branch ")` → `strip_prefix("refs/heads/")` 순서로 명확히 처리. `"(detached)"` 등 비-refs 브랜치 줄이 올바르게 표시됨. 커밋 0103dfa.

---

### Finding 7 — MEDIUM: toast 타이머 unmount 미정리

**내용**: `ChangesPanel` unmount 시 `setTimeout` 콜백이 취소되지 않아 패널 닫힌 직후 setState 호출 가능 (React 18에서는 경고 없지만 불필요한 타이머).

**결과**: **수정함**  
`useEffect(() => () => { if (toastRef.current) clearTimeout(toastRef.current); }, [])` 추가. 커밋 0103dfa.

---

### Finding 8 — LOW: `svn_status`의 `current_dir` vs `git_status`의 `-C`

**내용**: SVN은 `-C` 플래그가 없으므로 `current_dir` 사용은 의도적. Git은 `-C`가 올바름. 불일치가 아닌 플랫폼 차이.

**결과**: **수정 안 함** — 의도적 구현, 버그 아님.

---

## 최종 검증

| 항목 | 결과 |
|------|------|
| cargo test | 145 passed, 0 failed |
| npm test (vitest) | 52 passed, 0 failed |
| CRITICAL 항목 | 0 (수정 완료) |
| HIGH 항목 | 0 (수정 완료) |
| MEDIUM 항목 | 0 (수정 또는 LOW 재등급) |

**판정: 통과**
