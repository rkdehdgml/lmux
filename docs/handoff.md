# 다른 PC에서 이어서 작업하기

## 지금 상태 (2026-11-27)
- **1단계 통과**(창 + PowerShell 패널). Codex 6차에서 높음 0·중간 0.
- **2단계 통과**(셸 프로필, 프로젝트 등록, 사이드바, 다중 세션, JSON 저장). Codex 7차에서 높음 0·중간 0.
- **3단계 통과**(화면 분할). Codex·Claude 검토 최종 높음 0·중간 0.
- **4단계는 설계·계획까지 끝남, 구현 전.** 설계 `docs/superpowers/specs/2026-11-27-stage-4-design.md`, 계획 `docs/superpowers/plans/2026-11-27-stage-4.md`(Task 0부터), 결정 D43~D51. 이 프로젝트에서 쓰는 스킬·플러그인은 `docs/tooling.md`.
- 4단계 주의: Lmux 세션 안에서는 사용자 `statusLine`이 가려진다(사용자는 쓰지 않음). 계획서 Task 0은 실제 Claude를 한 번 실행하므로 사용자 확인이 필요하다.
- 진행 기록 `docs/progress.md`, 결정 `docs/decisions.md`, 검토 기록 `docs/reviews/`.

## 노트북 준비물
Windows 11 기준. 괄호는 이 작업을 한 PC에서 확인한 버전이다.

| 필요한 것 | 비고 |
|---|---|
| Git | |
| Node.js 24 (24.22.1, npm 11.19.1) | |
| Rust stable, MSVC 툴체인 (1.100.0) | rustup으로 설치 |
| Visual Studio 2022 Build Tools | "C++를 사용한 데스크톱 개발" 워크로드. 설치에 관리자 권한(UAC)이 필요 |
| WebView2 런타임 | Windows 11에는 기본 포함 |

작업 도구(앱 빌드에는 필요 없음)

| 도구 | 쓰는 곳 |
|---|---|
| Claude Code + superpowers 플러그인 | 구현. superpowers 스킬을 활용하라는 사용자 지시가 있음 |
| Codex CLI (로그인 필요) | 단계마다 교차검증. Orca에 이 폴더로 Codex 세션을 열고 권한을 **Read Only**로 두면 그 세션을 쓴다. 없으면 `codex exec --sandbox read-only` |
| agent-browser | `scripts/e2e/` 실행 확인 |
| Python 3 | `scripts/file-hashes.py` |

## 받아서 확인
```bash
git clone https://github.com/rkdehdgml/lmux.git
cd lmux
npm ci
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npx tauri build --no-bundle    # 결과: src-tauri/target/release/Lmux.exe
bash scripts/e2e/stage-3.sh    # 3단계 확인(실행 중 창을 건드리지 않는다. 최소화되면 실패한다)
bash scripts/e2e/stage-2.sh    # 선택, Git Bash에서. 실행 중에는 앱 창을 건드리지 않는다
```
첫 Rust 빌드는 몇 분 걸린다. 2026-11-27에 소스만 빈 폴더에 풀어 `npm ci`·`npm run build`·`npm test`·`cargo test`가 통과하는 것을 확인했다(같은 PC, 설치된 도구와 내려받은 패키지 캐시를 사용). 릴리스 빌드는 원래 폴더에서만 확인했고, 개발 모드(`npm run tauri dev`)는 아직 확인하지 않았다.

## Claude Code에서 이어가기
프로젝트 폴더에서 `claude`를 실행하고 이렇게 요청한다.

> CLAUDE.md, docs/handoff.md, docs/progress.md, docs/decisions.md, docs/spec.md를 읽고 4단계 계획서(docs/superpowers/plans/2026-11-27-stage-4.md)의 Task 0부터 이어서 진행해줘. docs/tooling.md의 스킬을 설치해 두었는지 먼저 확인하고, superpowers 스킬을 활용하고, Codex 검토는 높음·중간이 없을 때까지 반복해.

## git에 담기지 않는 것
- **Claude의 대화 기록과 메모리.** 필요한 규칙은 `CLAUDE.md`와 `docs/decisions.md`에 옮겨 두었다.
- **PC별 전역 지침·스킬**(`~/.claude`). 노트북에 없으면 `CLAUDE.md`의 규칙만 적용된다.
- `node_modules/`, `dist/`, `src-tauri/target/`, `backups/`(단계별 zip 스냅샷).

## 노트북에서 알아 둘 것
- 확인 스크립트는 WSL 배포판과 Git Bash가 있어야 전 항목이 돈다(WSL이 없으면 그 항목은 건너뜀). `node`가 필요한 테스트가 있다.
- `cargo test`의 실제 셸 테스트는 PowerShell·cmd·Git Bash가 설치돼 있어야 통과한다.
- 작업 데이터(프로젝트·세션 목록)는 PC마다 따로이며 git에 담기지 않는다.

## 꼭 지킬 것
- git 명령은 사용자가 지시할 때만. 프로젝트 루트의 `.git`은 지우지 않는다.
- 사용자의 프로젝트 파일(SVN 저장소)은 수정하지 않는다.
- 8단계(모바일)는 시작 전에 확인받는다.

## 아직 확인하지 못한 것
- 실제 키보드의 한글 조합 입력, 마우스 복사·붙여넣기, NSIS 설치 파일.
- PowerShell 7, bash가 없는 WSL 배포판, 폴더 선택 창에서 실제로 고르기(열림만 확인).
- 실제 Claude Code·Codex에 첫 지시가 전달되는지(가짜 프로그램으로만 확인). 앱을 두 번 열었을 때의 화면.
