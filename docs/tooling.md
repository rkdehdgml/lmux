# 작업 도구 안내 (노트북에서 이어서 할 때)

이 프로젝트를 Claude Code로 이어서 작업할 때 **쓰고 있는 스킬·플러그인·도구**다. 플러그인 자체는 저장소에 넣지 않는다(각자의 라이선스·업데이트 때문). 이름과 출처만 적었으니 노트북의 Claude Code에서 설치한다. 설치는 사용자가 확인한 뒤에만 하고, 외부 스킬은 설치 전에 보안 점검을 한다(사용자 전역 규칙).

## 필수에 가까운 것
| 이름 | 출처(마켓플레이스) | 용도 |
|---|---|---|
| `superpowers` | claude-plugins-official (작업 때 7.0.1~7.0.2) | 모든 단계의 작업 방식: `writing-plans`, `executing-plans`, `test-driven-development`, `systematic-debugging`, `verification-before-completion`, `receiving-code-review`. 사용자 지시로 **계속 사용** |
| `codex-crosscheck` | 이 저장소 `tooling/skills/codex-crosscheck/` | 사용자 개인 스킬. 열어 둔 Codex 세션에 읽기 전용 검증을 맡기고 결과를 재검증한다. `~/.claude/skills/`에 복사해 쓴다 |
| `orca-cli` | Orca 앱이 제공 | Codex 세션 조작(`orca terminal send/read/wait`). Orca 앱이 있어야 한다 |

## 있으면 좋은 것
| 이름 | 출처 | 용도 |
|---|---|---|
| `mattpocock-skills` (`grilling`, `/grill-me`) | claude-plugins-official 1.3.1 | 설계 전에 질문으로 점검. 4단계 설계는 이걸로 했다. `/grill-me`는 사용자가 직접 입력해야 한다 |
| `ponytail` | ponytail 5.3.x | 작은 변경·짧은 보고 모드(켜 두고 작업) |
| `rust-analyzer-lsp` | claude-plugins-official | Rust 코드 탐색 |
| `codex` (openai-codex) | openai-codex 1.0.x | Codex 연동. 검토는 열어 둔 Codex 세션을 쓰므로 필수 아님 |

## 도구(스킬이 아니라 프로그램)
- `agent-browser` CLI: 실제 앱(WebView2, 원격 디버깅 포트)을 조작하는 확인 스크립트(`scripts/e2e/*.sh`)가 쓴다. `agent-browser skills get core`로 사용법을 읽는다.
- Codex CLI(Orca에서 열어 둔 세션), Claude Code CLI 2.1 이상(`claude --help`에 `--settings`가 있어야 4단계가 된다).
- 빌드: `docs/handoff.md`의 "노트북에서 알아 둘 것"과 명령.

## 작업 규칙(요약, 자세히는 `CLAUDE.md`)
- 검토는 Codex(열어 둔 세션, Read Only 확인) + 새 Claude 검토 에이전트 2명. 높음·중간이 0이 될 때까지 반복.
- 설계는 `/grill-me`, 계획은 `superpowers:writing-plans`, 구현은 `superpowers:executing-plans` + TDD.
- 사용자 전역 `CLAUDE.md`(브라우저·Orca 규칙)는 개인 설정이라 이 저장소에 없다.
