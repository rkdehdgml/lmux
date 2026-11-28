# Stage 5 Codex 검토 결과

검토 도구: Codex (GPT-6.2-Sol) — 읽기 전용 세션  
검토 대상: 계정 사용량 30초 갱신 (usage.rs, account.rs, lib.rs, api.ts, App.tsx, AccountsPanel.tsx, styles.css)  
검토 일시: 2026-11-28

---

## 요약 테이블

| # | 항목 | 심각도 | 내용 | 조치 |
|---|------|--------|------|------|
| 1 | timestamp Z-suffix 미검사 | HIGH | `trim_end_matches('Z')`가 Z 없는 문자열도 통과시킴. 소수 부분 `&frac[..3]` 슬라이스가 UTF-8 경계에서 패닉 가능 | **수정함** — `strip_suffix('Z')?`로 Z 필수화, `chars().take(3)` 안전 처리 |
| 2 | ~/.claude 없을 때 처리 | LOW | `aggregate_folder`는 존재하지 않는 폴더에서 `WindowUsage::default()` 반환. 오류 전파 없음 — 의도된 동작 | 수정 안 함(설계) |
| 3 | 타이머·갱신 주기 | HIGH | 앱 종료 후 `emit()` 오류 무시는 의도적 설계. 1분·5분 선택이 실제 주기 미변경 | 수정 안 함 — emit 오류 무시는 올바른 설계. 주기 변경은 7단계(설정) 범위 |
| 4 | camelCase 직렬화 | LOW | `#[serde(rename_all = "camelCase")]` 적용 올바름. fiveHour, cacheCreation 등 TypeScript 타입과 일치 | 수정 안 함(이상 없음) |
| 5 | formatTokens K→M 경계 | MEDIUM | `999_500` → `Math.round(999500/1000)` = 1000 → "1000K" 표시 버그 | **수정함** — `k >= 1_000`일 때 M 포맷으로 전환 |
| 6 | 경로·심볼릭 링크 | HIGH | 집계 시 심볼릭 링크 추적, 절대 경로가 WebView에 노출 | 수정 안 함 — 로컬 데스크톱 도구로 사용자 본인 파일만 대상. 경로 표시는 사용자가 어떤 디렉터리를 감시하는지 알아야 하는 UI 요구사항 |

---

## 상세

### 항목 1 — 수정함

**변경 전:**
```rust
let s = s.trim_end_matches('Z');
```
`trim_end_matches`는 Z가 없어도 통과시키고, 여러 개의 Z도 모두 제거한다.

**변경 후:**
```rust
let s = s.strip_suffix('Z')?;  // Z 없으면 None 반환
```

소수 부분 슬라이스도 `&frac[..3]` (바이트 인덱스, 패닉 가능) →  
`frac.chars().take(3).filter(|c| c.is_ascii_digit()).collect()` (UTF-8 안전) 으로 변경.

**검증:** `returns_none_without_z_suffix` 테스트 추가, RED→GREEN 확인.  
cargo test → 133 passed.

### 항목 5 — 수정함

**변경 전:**
```typescript
if (n >= 1_000) return Math.round(n / 1_000) + "K";
```
`999_500` → `Math.round(999.5)` = 1000 → "1000K"

**변경 후:**
```typescript
if (n >= 1_000) {
  const k = Math.round(n / 1_000);
  return k >= 1_000 ? (n / 1_000_000).toFixed(1) + "M" : k + "K";
}
```
`999_500` → k=1000 → "1.0M"

**검증:** `large K rounds to M boundary` 테스트 추가, RED→GREEN 확인.  
vitest run → 46 passed.

---

## 통과 기준

높음·중간 항목 2개 모두 수정 완료. Codex 검토 통과.
