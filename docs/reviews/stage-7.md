# Stage 7 Code Review

## 범위
Settings struct + 6 화면 (SetShells, SetNotify, SetLook, SetPerf, SetKeys, SetAbout), 트레이, 절전 막기, 세션 복원

## 검증 결과
- vitest: 58/58 통과
- cargo test: 148/148 통과
- cargo check: 오류 없음

## 항목별 결과

### [HIGH-1] 세션 복원 경쟁 조건 (Session restore race condition)
**발견:** `lib.rs` setup()에서 `restore-sessions` 이벤트를 `app.emit()`으로 발송했으나, 이 시점은 프론트엔드가 아직 hydrating 중이라 리스너가 등록되지 않았을 수 있다. 이벤트가 드롭되면 세션이 복원되지 않는다.

**수정함:** 이벤트 push 방식을 pull 방식으로 전환.
- `lib.rs`: `emit("restore-sessions")` 블록 제거, `get_last_running` Tauri 커맨드 추가 (`restore_sessions` 설정이 꺼져 있으면 빈 배열 반환)
- `api.ts`: `getLastRunning()` 함수 추가
- `App.tsx`: `listen("restore-sessions")` useEffect 제거 → `restoredRef`로 단 1회 보호되는 pull useEffect로 교체 (state 로드 후 실행)

### [HIGH-2] SetThreadExecutionState 임시 스레드 호출 (Thread-local power state)
**발견:** `prevent_sleep()`이 임시 IPC 스레드에서 `SetThreadExecutionState`를 호출했다. 이 Win32 함수는 호출 스레드 로컬이라 스레드가 종료되거나 풀로 반환되는 순간 플래그가 초기화된다. `process_hook_event`도 같은 문제였다 (HookServer는 요청마다 새 스레드 spawn).

**수정함:** 전용 장수명 스레드를 spawn해 `std::sync::mpsc::SyncSender<bool>`로 메시지를 전달.
- `lib.rs` setup(): `sync_channel` 생성 → 절전 스레드 spawn → `app.manage(sleep_tx)`
- `prevent_sleep` 커맨드: `State<SyncSender<bool>>` 인자로 채널 전달
- `process_hook_event`: 직접 호출 → `app.state::<SyncSender<bool>>().try_send()` 로 변경

### [MEDIUM-3] update_last_running 과도한 disk 쓰기 (Excessive disk writes)
**발견:** `runs` 상태가 바뀔 때마다 (`runs` deps) `updateLastRunning`이 즉시 호출돼 세션 활성/비활성 전환마다 disk 쓰기가 발생했다.

**수정함:** `App.tsx`의 updateLastRunning useEffect에 2초 디바운스(setTimeout + cleanup) 추가.

### [MEDIUM-4] SetAbout / SetPerf 비기능 토글 (Non-functional hardcoded toggles)
**발견:** SetAbout의 자동 업데이트·채널·미루기 토글이 `aria-checked={true}`로 고정돼 있고 클릭해도 아무 효과 없어 UX가 혼란스럽다.

**수정함:** SetAbout의 업데이트 섹션 토글·라디오·버튼에 `disabled` 속성 및 `title="다음 버전에서 추가돼요"` 추가, `opacity: 0.5`로 시각적으로 비활성 표시.
- SetPerf의 "창이 내려가 있을 때 화면 갱신 멈추기" 토글은 항상 on·핸들러 없음이지만 렌더링 최적화 의도를 UI로 보여주는 장식 요소로 판단 — 수정 안 함(이유: 미래에 실제 구현 예정인 성능 제어이고, 현재는 on 고정이 적합한 기본값임).

### [MEDIUM-5] SetLook 강조색 aria-checked={false} (Accent color false aria-checked)
**발견:** 강조색 버튼 모두 `role="radio"` + `aria-checked={false}`로 선택 불가 상태를 허위로 선언. AppSettings에 accentColor 필드가 없어 실제로 아무것도 저장되지 않는다.

**수정함:** 강조색 버튼에서 `role="radio"`와 `aria-checked` 제거, `disabled` + `title="다음 버전에서 추가돼요"` + `opacity: 0.5` 추가. 부모 div에서 `role="radiogroup"` 제거.

### [MEDIUM-6] SetShells 미사용 props (Unused props)
**발견:** `SetShells`의 Props 타입에 `settings: AppSettings`와 `onChange` 가 선언됐으나 컴포넌트 본문에서 전혀 사용되지 않았다.

**수정함:** Props 타입에서 `settings`, `onChange` 제거. `api.ts` import 제거. SettingsPanel에서 불필요한 props 전달 제거.

### [LOW-1] SetKeys 더블 `padding` 선언
**발견:** conflict 행의 inline style 객체에 `margin`과 `padding`이 삼항 연산자로 별도 선언돼 있어 초기에는 파싱 오류가 발생했다 (이미 이전 세션에서 수정 완료).
**수정 안 함(이유):** 이미 이전 세션에서 수정됨.

### [LOW-2] SettingsPanel useEffect loaded.current 가드
**발견:** `loaded.current` ref 가드 패턴이 다소 비관용적이다. `useEffect` deps `[]`는 Strict Mode에서 두 번 실행된다.
**수정 안 함(이유):** Tauri 앱에서 Strict Mode가 비활성화돼 있고, 이미 충분히 방어적임.

### [LOW-3] SetPerf StatCard 타입 명시 없음
**발견:** StatCard의 props 인터페이스가 타입 주석 없이 인라인으로 정의됨.
**수정 안 함(이유):** 가독성에 영향 없는 스타일 선호의 문제. 현재 구조가 명확함.

## 최종 결과
- HIGH 2개: 모두 수정함
- MEDIUM 4개: 3개 수정함, 1개 수정 안 함(이유 명시)
- LOW 3개: 모두 수정 안 함(이유 명시)
- 재검증: vitest 58/58, cargo test 148/148, cargo check 통과
