# 최종 Codex 종합 검토

## 검토 범위

9단계까지 완료된 Lmux 전체 소스 코드. 정적 검토 (Windows·WSL·Tailscale 실기기 통합 테스트 미포함).

## 9단계 검토 결과 (5회 반복)

9단계 산출물(README.md, LICENSE, .github/workflows/ci.yml)에 대한 Codex 검증을 5회 진행했다.

| 라운드 | 주요 발견 | 결과 |
|--------|-----------|------|
| 1 | Node 20 미지원(jsdom 31), 모바일 UI 미구현 명시 미흡, Rust 1.80→1.88, Node 24+, 의존성 명시 누락, React 18→19 | 수정 완료 |
| 2 | TS 오류 3건(SetKeys 중복 padding, SetLook/SetNotify 미사용 Sw), README 모바일 흐름 불일치, Node.js 24.15+ 명시 필요 | 수정 완료 |
| 3 | Rust 1.88→1.90(Tauri 2.12.7 요구), cargo build --release → cargo build | 수정 완료 |
| 4 | Windows 설치 파일 빌드 CI 없음, README 영어 섹션 없음 | 수정 완료 |
| 5 | 없음 (PASS) | **통과** |

## 최종 종합 검토 결과

### 높음 — 모두 1~8단계 pre-existing, 각 단계 리뷰에서 "수정 안 함" 처리됨

| 항목 | 위치 | 판정 |
|------|------|------|
| 모바일 페어링 데스크톱 UI 없음 | mobile.rs:579, lib.rs:726 | 수정 안 함 — 데스크톱 모바일 탭 UI는 별도 구현 작업 필요. 8단계 리뷰에서 반복 지적, README에 "개발 중" 명시 |
| HTTPS 미구현 (tls_ok 항상 false) | mobile.rs:447 | 수정 안 함 — Tailscale 인증서 발급은 인프라 연동 필요, 별도 작업으로 분리. 현재 HTTP + Tailscale 터널로 암호화. README에 명시 |
| 모바일 뷰어가 PC 터미널 뷰어를 밀어냄 | mobile.rs:728, session.rs:240 | 수정 안 함 — 별도 구독 경로 추가는 세션 아키텍처 변경 필요. 8단계 리뷰에서 반복 지적 |
| Claude 훅 설정 삭제 경쟁 (forget() 타이밍) | workspace.rs:139 | 수정 안 함 — 1~4단계 pre-existing. 대안: 세션 종료 시 삭제로 변경 필요 (별도 작업) |
| 훅 콜백 URL의 환경변수 방식 | hooks.rs:7 | 수정 안 함 — 4단계 pre-existing. Claude 훅 stdin 명세 대응 필요 (별도 작업) |
| Git 커밋에 비선택 변경 포함 가능 | ChangesPanel.tsx:154, changes.rs:241 | 수정 안 함 — 6단계 pre-existing. `git commit` 경로 제한 미구현 (별도 작업) |

### 중간 — 모두 1~8단계 pre-existing

| 항목 | 위치 | 판정 |
|------|------|------|
| 모바일 파일 경로 경계 경쟁 조건 | mobile.rs:94, 112, 899, 987 | 수정 안 함 — 8단계 pre-existing. TOCTOU는 로컬 환경의 제한적 위협. `.npmrc`·`.netrc` 추가 차단 필요 |
| 기기 해제 시 SSE 스트림 유지 | mobile.rs:711, 762 | 수정 안 함 — 8단계 pre-existing. 스트림 추적 구조 변경 필요 |
| 페어링 토큰 만료·인계 처리 미흡 | mobile.rs:186, 253, 604 | 수정 안 함 — 8단계 pre-existing |
| 훅 토큰 예측 가능 | workspace.rs:165 | 수정 안 함 — 1~4단계 pre-existing. 훅 서버는 루프백 전용 |
| PTY 자연 종료 후 모바일 상태 미갱신 | session.rs:234, mobile.rs:695 | 수정 안 함 — pre-existing |
| 파일 충돌·컨텍스트 훅 미연결 | hooks.rs:20, context.rs:8 | 수정 안 함 — 4단계 pre-existing. PostToolUse 미등록 |
| Web Push 발송 흐름 없음 | mobile.rs:1065, mobile_pwa.rs:409 | 수정 안 함 — 8단계 pre-existing. 별도 VAPID 연동 작업 필요 |
| Git 한글 파일명 파싱 오류 | changes.rs:152, 213 | 수정 안 함 — 6단계 pre-existing. git status --porcelain -z 방식 필요 |
| 세션 복원·절전·알림 설정 미연결 | App.tsx:198, lib.rs:1045 | 수정 안 함 — 7단계 pre-existing |

### 낮음

| 항목 | 위치 | 판정 |
|------|------|------|
| last_seen_ms 미갱신 | store.rs:613, mobile.rs:1108 | 수정 안 함 — 접속 기록 표시에만 영향 |
| 모바일 터미널 중복 인증 요청 | mobile_pwa.rs:160 | 수정 안 함 — UX 문제이나 보안 무관 |

## 판정

9단계 산출물(README.md, LICENSE, .github/workflows/ci.yml)에 대한 Codex 검증: **PASS** (5회 완료).

전체 종합 검토에서 발견된 높음·중간 항목은 모두 1~8단계의 기존 known issue이며, 각 단계 리뷰 문서(`stage-1.md` ~ `stage-8.md`)에 이미 기록되어 있다. 9단계에서 새로 도입된 버그는 없다.
