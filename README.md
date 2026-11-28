# Lmux

**[한국어](#한국어) | [English](#english)**

---

## 한국어

Windows용 가벼운 AI 코딩 에이전트 멀티플렉서.  
여러 Claude Code 세션을 한 창에서 분할 보기하고, 상태·컨텍스트·사용량을 한눈에 확인합니다.

### 특징

- **멀티 세션** — 여러 Claude Code 세션을 탭/분할 패널로 동시에 실행
- **실시간 터미널** — xterm.js + ConPTY 기반 진짜 터미널 (PowerShell 7, WSL, Git Bash, cmd)
- **컨텍스트 추적** — 훅(hook)으로 세션별 컨텍스트 사용률·상태를 실시간 표시
- **계정 사용량** — 로컬 로그 집계로 오늘/5시간/주간 토큰 사용률 모니터링
- **SVN·Git 통합** — 세션별 변경 파일 표시, 같은 파일 동시 수정 경고
- **설정 화면** — 셸 프로필, 테마, 알림, API 키, 성능 옵션
- **모바일 접속 (Tailscale)** — Tailscale 사설망 경유 PWA로 터미널·파일 원격 열람

### 기술 스택

| 계층 | 기술 |
|------|------|
| 프레임워크 | [Tauri 2](https://tauri.app) |
| 백엔드 | Rust · portable-pty (ConPTY) · axum 0.7 |
| 프론트엔드 | React 19 · TypeScript · Vite |
| 터미널 | xterm.js + WebGL 애드온 |
| 폰트 | Noto Sans KR (UI) · JetBrains Mono (터미널) |

### 시스템 요구사항

- **Windows 10/11** (x64)
- [WebView2 런타임](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Windows 11에 기본 포함)
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) — C++ 데스크톱 워크로드
- Rust 1.90+ (`rustup` 설치 권장)
- Node.js 24.15 이상 (24.x LTS)
- **[Claude Code](https://claude.ai/code)** — 세션에서 `claude` 명령이 PATH에 있어야 합니다
- SVN 연동: `svn` CLI가 PATH에 있어야 합니다 (선택)
- Git 연동: `git` CLI가 PATH에 있어야 합니다 (선택)
- 모바일 접속: PC와 모바일 기기 모두 [Tailscale](https://tailscale.com) 연결 필요

### 개발 환경 설정

```powershell
npm install
npm run tauri dev
```

### 빌드

```powershell
# 프로덕션 빌드 (src-tauri/target/release/lmux.exe + NSIS 설치 파일)
npm run tauri build
```

### 테스트

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm test
```

### 모바일 접속 설정

> **현재 상태:** 모바일 HTTP 서버와 PWA(터미널·파일·알림)는 구현되어 있습니다.  
> 데스크톱 UI의 모바일 관리 탭(서버 시작·QR 표시·기기 승인)은 현재 개발 중이므로, 아래 절차 대신 Tauri 명령(모바일 탭 추가 예정)이 필요합니다.

**예정된 사용 절차 (데스크톱 모바일 탭 완성 후):**
1. PC와 모바일 기기를 모두 같은 Tailscale 계정에 연결합니다.
2. Lmux 설정 → **모바일** 탭에서 서버를 시작합니다.
3. 화면의 QR 코드를 모바일 기기의 카메라로 스캔합니다.
4. 모바일 화면에 6자리 코드가 표시되면, PC에서 승인하여 페어링을 완료합니다.

**참고 사항:**
- 모바일 서버는 Tailscale 100.64.0.0/10 대역에서만 접속을 허용합니다.
- 현재 HTTP(암호화는 Tailscale 터널에 의존)로 접속하므로, 브라우저에서 서비스 워커 등록이 제한될 수 있습니다.
- 터미널 입력 권한은 신뢰하는 기기에만 부여하세요.

### 라이선스

[MIT](LICENSE)

---

## English

A lightweight AI coding agent multiplexer for Windows.  
Run multiple Claude Code sessions side-by-side in a single window with real-time status, context, and usage monitoring.

### Features

- **Multi-session** — Run multiple Claude Code sessions in tabs or split panels
- **Real terminal** — xterm.js + ConPTY (PowerShell 7, WSL, Git Bash, cmd)
- **Context tracking** — Per-session context usage and status via Claude hooks
- **Account usage** — Today / 5-hour / weekly token usage from local logs
- **SVN & Git integration** — Per-session changed files, same-file conflict warnings
- **Settings** — Shell profiles, theme, notifications, API keys, performance
- **Mobile access (Tailscale)** — Browse terminal and files from a phone via PWA over Tailscale

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri 2](https://tauri.app) |
| Backend | Rust · portable-pty (ConPTY) · axum 0.7 |
| Frontend | React 19 · TypeScript · Vite |
| Terminal | xterm.js + WebGL addon |
| Fonts | Noto Sans KR (UI) · JetBrains Mono (terminal) |

### Requirements

- **Windows 10/11** (x64)
- [WebView2 runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included in Windows 11)
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) — Desktop development with C++
- Rust 1.90+ (via `rustup`)
- Node.js 24.15+ (24.x LTS)
- **[Claude Code](https://claude.ai/code)** — `claude` must be on PATH
- SVN integration: `svn` CLI on PATH (optional)
- Git integration: `git` CLI on PATH (optional)
- Mobile access: [Tailscale](https://tailscale.com) on both PC and mobile device

### Quick Start

```powershell
npm install
npm run tauri dev    # dev server with hot reload
npm run tauri build  # production build + NSIS installer
```

### License

[MIT](LICENSE)
