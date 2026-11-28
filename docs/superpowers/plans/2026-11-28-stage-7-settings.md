# Stage 7 — Settings Screens, Tray, Sleep Prevention, Session Restore

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement all 6 settings screens, system tray support, Windows sleep prevention when agents are working, and session restore on app restart.

**Architecture:** Add a `Settings` struct to `store.rs`, expose via two Tauri commands, and wire the settings to actual OS behavior (tray, sleep). The UI is a modal overlay with a fixed left nav and scrollable right content; each screen is a focused React component.

**Tech Stack:** Tauri 2 (tray-icon feature), windows-sys Win32_System_Power, React + TypeScript + vitest

**Spec:** docs/spec.md §5 (설정 screens), §3 (절전 막기, 세션 복원), mockups/Settings.dc.html, mockups/SetNotify.dc.html, mockups/SetLook.dc.html, mockups/SetPerf.dc.html, mockups/SetKeys.dc.html, mockups/SetAbout.dc.html

## Global Constraints

- Dark theme colors only: bg #0E1014, surface #14171D, text #E8EAF0, accent #2F6FE4, muted #A0A8B8
- Font: 'Noto Sans KR' for UI, 'JetBrains Mono' for monospace
- Border-radius 8–12px; no visible border lines on most elements
- All settings persist to state.json (the existing store file)
- Never modify user project files
- Korean labels exactly as shown in mockups
- `tray-icon` Tauri feature required; `Win32_System_Power` windows-sys feature required

## Review Focus

- Settings not persisted on save (api.saveSettings not wired to store write)
- Sleep prevention not released when last working agent finishes
- Tray icon: app remains visible after clicking X when minimize_to_tray is false
- Session restore fires duplicate start if session was already started
- Settings panel renders with broken layout on 900px wide window (min-width of app)

---

### Task 1: Settings struct + Rust commands + sleep prevention

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `get_settings` Tauri command → `Settings` JSON; `save_settings(settings: Settings)` → `()`; `prevent_sleep(enable: bool)` → `()`
- Produces: `Settings` struct (camelCase serialization) with all fields below

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // 알림
    pub notify_waiting: bool,     // default true
    pub notify_done: bool,        // default true
    pub notify_error: bool,       // default true
    pub notify_conflict: bool,    // default true
    pub notify_context: bool,     // default true
    pub notify_context_threshold: u8, // default 80
    // 화면
    pub theme: String,            // default "dark"
    pub language: String,         // default "ko"
    pub density: String,          // default "normal"
    pub terminal_font: String,    // default "JetBrains Mono"
    pub terminal_font_size: u8,   // default 13
    pub terminal_line_height: f32,// default 1.6
    pub cursor_shape: String,     // default "block"
    // 성능
    pub webgl: bool,              // default true
    pub hide_inactive: bool,      // default true
    pub scrollback_lines: u32,    // default 10000
    pub prevent_sleep: bool,      // default true
    pub minimize_to_tray: bool,   // default true
    pub restore_sessions: bool,   // default true
}
impl Default for Settings { /* all defaults above */ }
```

- [ ] **Step 1: Write failing test** in `src-tauri/src/store.rs` #[cfg(test)] block:
```rust
#[test]
fn settings_default_values_are_sane() {
    let s = Settings::default();
    assert_eq!(s.theme, "dark");
    assert!(s.notify_waiting);
    assert!(s.prevent_sleep);
    assert_eq!(s.terminal_font_size, 13);
    assert_eq!(s.scrollback_lines, 10000);
}
#[test]
fn settings_roundtrip_json() {
    let s = Settings::default();
    let json = serde_json::to_string(&s).unwrap();
    let back: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(back.theme, s.theme);
}
```

- [ ] **Step 2: Run test to verify fails**

Run: `cargo test -p lmux_lib settings` from `src-tauri`
Expected: FAIL — `Settings` not defined

- [ ] **Step 3: Add Settings struct to `store.rs`**

Add `pub struct Settings` with all fields and their defaults via `impl Default`. Add `pub settings: Settings` field to `State` with `#[serde(default)]`. Update `State::default()` to include `settings: Settings::default()`.

- [ ] **Step 4: Add `Win32_System_Power` to Cargo.toml**

In `src-tauri/Cargo.toml`, add `"Win32_System_Power"` to the `windows-sys` features array.

- [ ] **Step 5: Add `tray-icon` feature to tauri in Cargo.toml**

Change `tauri = { version = "2", features = [] }` to `tauri = { version = "2", features = ["tray-icon"] }`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p lmux_lib settings`
Expected: 2 tests PASS

- [ ] **Step 7: Add get_settings and save_settings commands to lib.rs**

```rust
#[tauri::command]
fn get_settings(workspace: Shared<'_>) -> Result<store::Settings, String> {
    Ok(lock(&workspace)?.store.state().settings.clone())
}

#[tauri::command]
fn save_settings(workspace: Shared<'_>, settings: store::Settings) -> Result<(), String> {
    let mut ws = lock(&workspace)?;
    ws.store.update_settings(settings);
    Ok(())
}
```

Add `update_settings` method to `Workspace` in workspace.rs (or directly to store via a new `Store::set_settings` method).

- [ ] **Step 8: Add prevent_sleep command to lib.rs**

```rust
#[tauri::command]
fn prevent_sleep(enable: bool) {
    use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED, ES_DISPLAY_REQUIRED};
    unsafe {
        if enable {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        } else {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
```

Register all three in `invoke_handler!`.

- [ ] **Step 9: Wire sleep prevention to process_hook_event**

In `process_hook_event`, after updating session state to `Working`, call `prevent_sleep(true)`. When transitioning to `Done`, check if any other session is still `Working`; if not, call `prevent_sleep(false)`.

Add a helper `fn any_session_working(states: &HookStates) -> bool` that iterates the states map.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat: add Settings struct, get/save commands, sleep prevention"
```

---

### Task 2: Tray icon (Rust)

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `minimize_to_tray: bool` from `Settings` (Task 1)
- Produces: system tray icon with "열기" / "종료" menu; window close → hide when setting is on

- [ ] **Step 1: Write failing test** — cargo test already confirms tray builds (compile test only):

Add this to `lib.rs` test module or compile-time check: for now just add a `#[test] fn tray_imports_compile()` that references `tauri::tray::TrayIconBuilder`. Since we can't unit-test OS tray in CI, verify by build success.

- [ ] **Step 2: Add tray configuration to tauri.conf.json**

Add under `app`:
```json
"trayIcon": {
  "iconPath": "icons/32x32.png",
  "iconAsTemplate": false,
  "tooltip": "Lmux"
}
```

- [ ] **Step 3: Set up tray in lib.rs setup()**

In the `setup` closure, after creating the HookServer:
```rust
let tray = tauri::tray::TrayIconBuilder::new()
    .tooltip("Lmux")
    .icon(app.default_window_icon().unwrap().clone())
    .menu(&tauri::menu::MenuBuilder::new(app)
        .text("open", "Lmux 열기")
        .separator()
        .text("quit", "종료")
        .build()?)
    .on_menu_event(|app, event| match event.id().as_ref() {
        "open" => { let _ = app.get_webview_window("main").map(|w| w.show()); }
        "quit" => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|icon, event| {
        if let tauri::tray::TrayIconEvent::Click { .. } = event {
            let _ = icon.app_handle().get_webview_window("main").map(|w| { let _ = w.show(); let _ = w.set_focus(); });
        }
    })
    .build(app)?;
app.manage(tray);
```

- [ ] **Step 4: Handle window CloseRequested event**

In `run()`, add `.on_window_event(|win, ev| { ... })` (or extend the existing one if present):
```rust
.on_window_event(|win, ev| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
        // Check minimize_to_tray setting
        if let Ok(ws) = win.state::<Mutex<Workspace>>().lock() {
            if ws.store.state().settings.minimize_to_tray {
                api.prevent_close();
                let _ = win.hide();
                return;
            }
        }
    }
})
```

- [ ] **Step 5: Run cargo test**

Run: `cargo test -p lmux_lib`
Expected: 145+ tests pass, 0 failed

- [ ] **Step 6: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/lib.rs
git commit -m "feat: add system tray icon and minimize-to-tray behavior"
```

---

### Task 3: Session restore

**Files:**
- Modify: `src-tauri/src/store.rs` (add `last_running` field)
- Modify: `src-tauri/src/lib.rs` (emit restore event on startup, update last_running on session start/delete)
- Modify: `src/App.tsx` (listen for restore event, start sessions)

**Interfaces:**
- Produces: `"restore-sessions"` Tauri event payload `{ sessionIds: string[] }`
- Consumes: `restore_sessions: bool` from Settings (Task 1)

- [ ] **Step 1: Write failing test in store.rs**

```rust
#[test]
fn last_running_persists_in_state() {
    let mut s = State::default();
    s.last_running = vec!["s1".to_string()];
    let json = serde_json::to_string(&s).unwrap();
    let back: State = serde_json::from_str(&json).unwrap();
    assert_eq!(back.last_running, vec!["s1"]);
}
```

- [ ] **Step 2: Run test to verify fails**

Run: `cargo test -p lmux_lib last_running`
Expected: FAIL

- [ ] **Step 3: Add `last_running` field to State**

```rust
#[serde(default)]
pub last_running: Vec<String>,
```

Update `State::default()` to include `last_running: Vec::new()`.

- [ ] **Step 4: Emit restore-sessions event in setup()**

In `setup()`, after app setup:
```rust
{
    let ws = lock(&app.state::<Mutex<Workspace>>()).expect("workspace");
    let settings = ws.store.state().settings.clone();
    let ids = ws.store.state().last_running.clone();
    if settings.restore_sessions && !ids.is_empty() {
        let _ = app.emit("restore-sessions", serde_json::json!({ "sessionIds": ids }));
    }
}
```

- [ ] **Step 5: Track last_running when sessions start/delete**

Add a `update_last_running` Tauri command (or handle it inside `session_start` / `session_delete`): when a session starts, add its id to `last_running`; when deleted, remove it.

Actually simpler: add `update_last_running_cmd(ids: Vec<String>)` Tauri command, called from frontend. The frontend already knows which sessions are "started" (run state is `started`). Call it in `useEffect` when sessions change.

- [ ] **Step 6: Run tests**

Run: `cargo test -p lmux_lib last_running`
Expected: PASS

- [ ] **Step 7: Add event listener in App.tsx**

```tsx
useEffect(() => {
  const unlisten = listen<{ sessionIds: string[] }>("restore-sessions", ({ payload }) => {
    for (const id of payload.sessionIds) {
      void api.sessionStart(id, 80, 24);
    }
  });
  return () => { void unlisten.then(f => f()); };
}, []);
```

Add `update_last_running` Tauri command call in a `useEffect` that tracks `state.sessions` and runs starts.

- [ ] **Step 8: Run vitest**

Run: `npm test -- --run`
Expected: 52 tests pass

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/store.rs src-tauri/src/lib.rs src/App.tsx
git commit -m "feat: session restore on app restart"
```

---

### Task 4: SettingsPanel + SetShells screen

**Files:**
- Create: `src/SettingsPanel.tsx`
- Create: `src/SetShells.tsx`
- Create: `src/SettingsPanel.test.tsx`
- Modify: `src/styles.css` (settings overlay CSS)
- Modify: `src/api.ts` (getSettings/saveSettings/preventSleep)

**Interfaces:**
- Consumes: Settings type from Rust (Task 1)
- Produces: `SettingsPanel` default export with props `{ onClose: () => void }`

**api.ts additions:**
```ts
export interface AppSettings {
  notifyWaiting: boolean; notifyDone: boolean; notifyError: boolean;
  notifyConflict: boolean; notifyContext: boolean; notifyContextThreshold: number;
  theme: string; language: string; density: string;
  terminalFont: string; terminalFontSize: number; terminalLineHeight: number; cursorShape: string;
  webgl: boolean; hideInactive: boolean; scrollbackLines: number;
  preventSleep: boolean; minimizeToTray: boolean; restoreSessions: boolean;
}
export const getSettings = () => invoke<AppSettings>("get_settings");
export const saveSettings = (s: AppSettings) => invoke<void>("save_settings", { settings: s });
export const preventSleep = (enable: boolean) => invoke<void>("prevent_sleep", { enable });
export const updateLastRunning = (ids: string[]) => invoke<void>("update_last_running", { ids });
```

- [ ] **Step 1: Write failing test for SettingsPanel**

```tsx
// @vitest-environment jsdom
import { vi, test, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SettingsPanel from "./SettingsPanel";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue({
  notifyWaiting: true, notifyDone: true, notifyError: true, notifyConflict: true,
  notifyContext: true, notifyContextThreshold: 80, theme: "dark", language: "ko",
  density: "normal", terminalFont: "JetBrains Mono", terminalFontSize: 13,
  terminalLineHeight: 1.6, cursorShape: "block", webgl: true, hideInactive: true,
  scrollbackLines: 10000, preventSleep: true, minimizeToTray: true, restoreSessions: true,
}) }));
test("renders settings nav", () => {
  render(<SettingsPanel onClose={() => {}} />);
  expect(screen.getByText("셸 프로필")).toBeTruthy();
  expect(screen.getByText("알림")).toBeTruthy();
  expect(screen.getByText("정보 · 업데이트")).toBeTruthy();
});
test("renders initial screen heading", () => {
  render(<SettingsPanel onClose={() => {}} />);
  expect(screen.getByRole("heading", { name: "셸 프로필" })).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify fails**

Run: `npm test -- --run src/SettingsPanel.test.tsx`
Expected: FAIL — component not defined

- [ ] **Step 3: Add getSettings/saveSettings/preventSleep to api.ts**

- [ ] **Step 4: Implement SettingsPanel.tsx**

Modal overlay (position:fixed inset 0, z-index 300, dark backdrop). Inside: flex row with left nav (240px) + right content. Left nav: title "설정", 6 nav items (셸 프로필, 알림, 화면, 성능, 단축키, 정보·업데이트). Active item highlighted. Right content: renders the active sub-screen component passing `settings` and `onChange` props. Close button (X) in top-right. ESC key closes.

State:
- `settings: AppSettings | null` (loaded on mount via `api.getSettings()`)
- `activeTab: string` (default "shells")
- `saving: boolean`

OnMount: `api.getSettings()` → set settings state. Save button: `api.saveSettings(settings)`.

- [ ] **Step 5: Implement SetShells.tsx**

Shows detected shells list (from `api.shellsList()`) and custom profiles from settings (since we're not adding custom shells in store for now, just display detected ones). For each shell: color dot, name. Right panel shows editable fields for selected shell (name, executable, args - read from ShellInfo for detected shells). Include "WSL 배포판 다시 찾기" button that calls `api.shellsList()` again. For now, custom shell editing saves to settings as `customShells: []` array. Keep it simple: display-only for auto-detected shells, with a note that custom profiles can be added.

- [ ] **Step 6: Add settings overlay CSS to styles.css**

```css
.settings-overlay { position:fixed;inset:0;z-index:300;background:rgba(9,11,14,0.85);display:flex;align-items:center;justify-content:center;padding:30px 16px }
.settings-panel { background:#0E1014;border-radius:16px;box-shadow:0 30px 80px #000000b0,0 0 0 1px #20252F;width:100%;max-width:1240px;max-height:calc(100vh - 60px);display:flex;overflow:hidden }
.settings-nav { flex:0 0 220px;background:#14171D;padding:20px 10px;display:flex;flex-direction:column;gap:2px;overflow-y:auto }
.settings-content { flex:1;padding:24px 32px;overflow-y:auto;min-width:0 }
.settings-nav-item { display:block;padding:9px 12px;border-radius:8px;font-size:13.5px;color:#C7CCD6;background:transparent;border:0;cursor:pointer;text-align:left;width:100%;font-family:inherit }
.settings-nav-item.active { background:#1E2330;color:#E8EAF0 }
.settings-nav-item:hover:not(.active) { background:#171B25 }
.settings-card { background:#14171D;border-radius:12px;padding:4px 20px }
.settings-item { display:flex;align-items:center;gap:12px;padding:14px 0;border-top:1px solid #1F242D;font-size:13.5px;flex-wrap:wrap }
.settings-item:first-child { border-top:0 }
.settings-sw { width:38px;height:22px;border-radius:999px;border:0;padding:3px;display:inline-flex;cursor:pointer;background:#3A4152;flex:none }
.settings-sw.on { background:#2F6FE4;justify-content:flex-end }
.settings-sw > span { width:16px;height:16px;border-radius:50%;background:#fff;pointer-events:none }
.settings-input { font-family:inherit;font-size:13px;background:#0E1014;border:1px solid #262B36;border-radius:8px;color:#E8EAF0;padding:7px 10px }
```

- [ ] **Step 7: Run test to verify passes**

Run: `npm test -- --run src/SettingsPanel.test.tsx`
Expected: 2 tests PASS

- [ ] **Step 8: Run full vitest suite**

Run: `npm test -- --run`
Expected: 54 tests pass

- [ ] **Step 9: Commit**

```bash
git add src/api.ts src/SettingsPanel.tsx src/SetShells.tsx src/SettingsPanel.test.tsx src/styles.css
git commit -m "feat: settings panel shell and nav"
```

---

### Task 5: SetNotify + SetLook + SetPerf screens

**Files:**
- Create: `src/SetNotify.tsx`
- Create: `src/SetLook.tsx`
- Create: `src/SetPerf.tsx`
- Create: `src/SetLook.test.tsx`

**Interfaces:**
- Consumes: `settings: AppSettings` and `onChange: (patch: Partial<AppSettings>) => void` props

- [ ] **Step 1: Write failing test for SetLook**

```tsx
// @vitest-environment jsdom
import { vi, test, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SetLook from "./SetLook";
const defaults = { theme: "dark", language: "ko", density: "normal", terminalFont: "JetBrains Mono", terminalFontSize: 13, terminalLineHeight: 1.6, cursorShape: "block" } as const;
test("renders theme heading", () => {
  render(<SetLook settings={defaults as any} onChange={() => {}} />);
  expect(screen.getByRole("heading", { name: "화면" })).toBeTruthy();
});
test("dark theme radio is checked", () => {
  render(<SetLook settings={defaults as any} onChange={() => {}} />);
  const darkBtn = screen.getByRole("radio", { name: "다크" });
  expect(darkBtn.getAttribute("aria-checked")).toBe("true");
});
```

- [ ] **Step 2: Run test to verify fails**

Run: `npm test -- --run src/SetLook.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement SetNotify.tsx**

Notification matrix table: rows = 6 event types (입력 대기, 완료, 오류, 충돌, 컨텍스트, 사용량). Columns = PC 알림. Each cell = checkbox bound to corresponding settings field. Context threshold: `<select>` for 70%/80%/90% maps to `notifyContextThreshold`. Phone/Sound columns are displayed but grayed out as "Stage 8에서 추가됩니다" or just disabled checkboxes. Test notification button calls `api.sendNotification("테스트 알림")`.

- [ ] **Step 4: Implement SetLook.tsx**

Theme: 3 radio buttons (다크/라이트/Windows 설정 따라가기) bound to `settings.theme`. Show warning "라이트 테마는 다음 버전에서 지원해요" for non-dark. Accent color: 4 color circles. Language: select (한국어/English). Density: 기본/조밀 radio group. Terminal: font select, font size input, line height input, preview block, cursor shape select. All changes call `onChange` with the patch.

- [ ] **Step 5: Implement SetPerf.tsx**

App status stats: session count from props (pass `sessionCount` and `runningCount` as props from SettingsPanel). Render toggles: `hideInactive`, `webgl`, scrollback select (5000/10000/50000), minimize refresh on hide. Intervals section: usage poll (30s/1m/5m - read-only for now). Execution: `preventSleep` toggle, `minimizeToTray` toggle, startup toggle (not implemented - note "다음 버전"), `restoreSessions` toggle.

- [ ] **Step 6: Run tests**

Run: `npm test -- --run src/SetLook.test.tsx`
Expected: 2 tests PASS

Run: `npm test -- --run`
Expected: 56 tests pass

- [ ] **Step 7: Commit**

```bash
git add src/SetNotify.tsx src/SetLook.tsx src/SetPerf.tsx src/SetLook.test.tsx
git commit -m "feat: SetNotify, SetLook, SetPerf screens"
```

---

### Task 6: SetKeys + SetAbout + App.tsx integration

**Files:**
- Create: `src/SetKeys.tsx`
- Create: `src/SetAbout.tsx`
- Modify: `src/App.tsx` (Ctrl+, opens settings, update_last_running)
- Modify: `src-tauri/src/lib.rs` (register update_last_running command)

**Interfaces:**
- Consumes: all 6 screen components
- Produces: `Ctrl+,` keyboard shortcut opens settings; last_running updated when sessions start/change

- [ ] **Step 1: Write failing tests** in `SettingsPanel.test.tsx` (add new tests):

```tsx
test("Ctrl+, keyboard shortcut opens settings", () => {
  // This is integration-tested through App.tsx; just test SetAbout renders version
});
```

Add a `SetAbout.test.tsx`:
```tsx
// @vitest-environment jsdom
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import SetAbout from "./SetAbout";
test("renders app version", () => {
  render(<SetAbout />);
  expect(screen.getByText(/v0\.1\.0/)).toBeTruthy();
});
test("renders license type", () => {
  render(<SetAbout />);
  expect(screen.getByText("MIT")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify fails**

Run: `npm test -- --run src/SetAbout.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement SetKeys.tsx**

Read-only shortcuts table matching the mockup exactly. 4 sections: 세션 (Ctrl+N, Ctrl+W, Ctrl+Tab, Ctrl+J), 화면 분할 (Ctrl+\, Ctrl+Shift+\, Ctrl+1-4, Ctrl+Shift+Enter, Ctrl+Shift+=), 에이전트 (Alt+1/2/3, Ctrl+Shift+K, Ctrl+Shift+G), 앱 (Ctrl+K, Ctrl+,, Ctrl+B with conflict note). Each row has action label + kbd badge(s) + a disabled "변경" button (편집은 다음 버전).

- [ ] **Step 4: Implement SetAbout.tsx**

Header card: Lmux icon, name "Lmux", subtitle "가벼운 AI 에이전트 멀티플렉서", version "v0.1.0" (hardcoded from tauri.conf). Status: "✓ 최신 버전이에요 · 방금 확인". Button "업데이트 확인" (shows toast "최신 버전이에요"). Auto-update section: 2 toggles (bound to settings if we add `autoUpdate: bool` — add to Settings struct). Open-source section: GitHub link placeholder, MIT license. Diagnostics: anonymous stats toggle (default false), "진단 로그 내보내기" button.

- [ ] **Step 5: Add Ctrl+, handler in App.tsx**

In the `onKey` handler (before the existing Ctrl+Shift+G check):
```tsx
if (e.ctrlKey && e.key === "," && !e.shiftKey && !document.querySelector("dialog[open]")) {
  e.preventDefault(); e.stopPropagation();
  setModal({ type: "settings" });
  return;
}
```

Add `| { type: "settings" }` to the Modal union. Add SettingsPanel rendering after ChangesPanel rendering.

- [ ] **Step 6: Add update_last_running in lib.rs**

```rust
#[tauri::command]
fn update_last_running(workspace: Shared<'_>, ids: Vec<String>) -> Result<(), String> {
    let mut ws = lock(&workspace)?;
    ws.store.update_last_running(ids);
    Ok(())
}
```

Add `fn update_last_running` to workspace.rs/store that sets `state.last_running = ids` and saves.

Add a `useEffect` in App.tsx that calls `api.updateLastRunning(runningSessionIds)` whenever the set of running sessions changes.

- [ ] **Step 7: Run test to verify SetAbout passes**

Run: `npm test -- --run src/SetAbout.test.tsx`
Expected: 2 tests PASS

Run: `npm test -- --run`
Expected: 58 tests pass, tsc clean

- [ ] **Step 8: Run cargo tests**

Run: `cargo test -p lmux_lib`
Expected: 147+ tests pass

- [ ] **Step 9: Commit**

```bash
git add src/SetKeys.tsx src/SetAbout.tsx src/SetAbout.test.tsx src/App.tsx src-tauri/src/lib.rs src-tauri/src/workspace.rs
git commit -m "feat: stage 7 complete — SetKeys, SetAbout, Ctrl+, shortcut, session restore wiring"
```
