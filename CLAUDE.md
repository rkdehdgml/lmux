# Lmux 작업 규칙 (요약)

전체 지시문: `docs/spec.md` (원본). 이 파일과 다르면 `docs/spec.md`가 우선한다.

## 제품
Windows용 가벼운 AI 코딩 에이전트 멀티플렉서. Tauri 2 + React + TypeScript + Vite, 백엔드 Rust(portable-pty), 터미널 xterm.js. 에디터·내장 브라우저·자동화 대시보드는 넣지 않는다. 대화·문서는 한국어.

## 절대 규칙
- **git 명령은 사용자가 지시할 때만**: init·clone·add·commit·push·브랜치 등 전부. 2026-11-27 사용자 지시로 `https://github.com/rkdehdgml/lmux.git`에 커밋·푸시(장소 이동용). 그 지시는 그 1건에 대한 것이다. force push 금지.
  - 예외: Lmux의 Git 기능 테스트는 임시로 만든 테스트용 저장소에서만 한다.
  - 스캐폴드 도구가 **새로 만든** `.git`만 지운다. **프로젝트 루트의 `.git`은 실제 저장소이므로 절대 지우지 않는다.**
- 사용자의 프로젝트 파일(SVN 저장소)은 절대 수정하지 않는다.
- Claude 훅 설정을 사용자 설정에 넣어야 하면 먼저 백업하고 알린다(우선 `--settings` 주입).
- UAC가 필요한 설치는 실행 직전에 알린다.
- **8단계(모바일)는 시작 전에 멈추고 확인받는다.**
- 꼭 필요한 것만 묻고, 나머지는 정해서 `docs/decisions.md`에 이유와 함께 기록한다.

## 단계 진행 순서 (매 단계)
1 구현 → 2 빌드 → 3 테스트 → 4 직접 실행 확인 → 5 Codex 교차검증 → 6 수정 → 7 재검증(빌드·테스트·실행 + Codex 재검토) → 8 `backups/stage-N.zip`(node_modules·target·dist 제외) 후 다음 단계.

- Codex는 **사용자가 Orca에 열어 둔 Codex 세션**(이 프로젝트 폴더)에 보낸다(`orca-cli` 스킬). `/status`에서 Read Only 확인, 전송 전후 파일 해시 비교. 수정은 Claude가 한다.
- 열어 둔 세션이 없으면 사용자에게 알린 뒤 `codex exec --sandbox read-only --skip-git-repo-check`.
- 결과는 `docs/reviews/stage-N.md`, 항목마다 "수정함 / 수정 안 함(이유)".
- 높음·중간 항목이 없으면 통과. **횟수 한도 없이** 통과할 때까지 반복한다(2026-11-27 사용자 지시, `docs/spec.md`의 3회 규칙을 대체).
- 작업은 superpowers 스킬을 활용한다(2026-11-27 사용자 지시).
- `docs/progress.md`에 단계별 기록(한 일, 바뀐 파일, 검증 결과, 남은 문제).

## 단계
1 PowerShell 패널 1개 / 2 셸 프로필·프로젝트·사이드바·다중 세션 / 3 화면 분할 / 4 훅 상태·컨텍스트·승인·알림 / 5 계정 사용량 / 6 SVN·Git 변경·충돌 경고·워크트리 / 7 설정 6화면·트레이·절전·복원 / 8 모바일(확인 후) / 9 README·LICENSE·Actions. 마지막에 Codex 종합 검토(`docs/reviews/final.md`) 후 보고하고 멈춘다.

## 디자인
`docs/mockups/*.dc.html`의 색·간격·구성을 그대로 따른다. 색·글꼴은 `docs/spec.md` 4절.
