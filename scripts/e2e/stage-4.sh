#!/usr/bin/env bash
# 4단계 실행 확인: 훅 상태·컨텍스트·승인·알림. Git Bash에서 실행.
# 필요: agent-browser, node, curl.exe, 먼저 `npx tauri build --no-bundle`. 실행 중인 Lmux가 없어야 한다.
# 화면을 건드리면 창이 최소화돼 실패한다. 실행 중에는 손대지 않는다.
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
export OUT="${OUT:-${TMPDIR:-/tmp}/lmux-e2e}/stage-4"
rm -rf "$OUT"; mkdir -p "$OUT/data" "$OUT/work"
mkdir -p "$OUT/home/bin"

# ── 가짜 claude ────────────────────────────────────────────────────────────────
# --settings <파일>에서 URL base를 추출해 훅을 순서대로 보낸다.
# UserPromptSubmit → (2초 대기) → PermissionRequest → stdin 대기 → Stop → SessionEnd
cat > "$OUT/home/bin/claude" << 'FAKE_CLAUDE'
#!/usr/bin/env bash
SETTINGS=""
while [[ $# -gt 0 ]]; do
  [[ "$1" == "--settings" ]] && { SETTINGS="$2"; shift 2; } || shift
done
[[ -z "$SETTINGS" ]] && { read -r _; exit 0; }

URL_BASE=$(node -pe "
  try {
    const f = require('fs').readFileSync(process.argv[1], 'utf8');
    const s = JSON.parse(f);
    const cmd = s.hooks.UserPromptSubmit[0].hooks[0].command;
    const m = cmd.match(/http:\\/\\/127\\.0\\.0\\.1:[^\\/]+\\/event\\/[^\\/]+\\/[^\\/]+/);
    m ? m[0] : ''
  } catch(e) { '' }
" "$SETTINGS" 2>/dev/null)

[[ -z "$URL_BASE" ]] && { read -r _; exit 0; }

fire() {
  local event="$1" body="$2"
  printf '%s' "$body" | curl.exe -sf -m 3 -X POST --data-binary @- "${URL_BASE}/${event}" >/dev/null 2>&1 || true
}

sleep 0.3
fire "UserPromptSubmit" '{"hook_event_name":"UserPromptSubmit"}'
sleep 2.0  # Working 상태 확인 창
fire "PermissionRequest" '{"hook_event_name":"PermissionRequest","tool_use_id":"tu_test","tool_name":"Bash"}'
sleep 0.3
read -r _CHOICE  # session_approve 가 "1\r" 등을 보낸다
sleep 0.3
fire "Stop" '{"hook_event_name":"Stop"}'
sleep 0.3
fire "SessionEnd" '{"hook_event_name":"SessionEnd"}'
exit 0
FAKE_CLAUDE
chmod +x "$OUT/home/bin/claude"

# bash 프로필: 가짜 claude를 PATH 앞에 두기
printf 'export PATH="$HOME/bin:$PATH"\n' > "$OUT/home/.bash_profile"
FAKE_HOME="$(cygpath -w "$OUT/home")"
FOUND=$(HOME="$FAKE_HOME" "$(cygpath -u "$PROGRAMFILES")/Git/bin/bash.exe" --login -i -c 'command -v claude' 2>/dev/null)
[[ $FOUND == */home/bin/claude ]] || { echo "중단: 가짜 claude가 먼저 잡히지 않음 ($FOUND)"; exit 1; }

# ── 헬퍼 ──────────────────────────────────────────────────────────────────────
S=lmux-e2e4
ab() { agent-browser --session $S "$@"; }
js() { ab eval "$1"; }
win() { powershell -NoProfile -ExecutionPolicy Bypass -File ./win.ps1 "$@"; }
button() { ab find role button click --name "$1" --exact >/dev/null; }
shot() { ab screenshot "$(cygpath -w "$OUT/$1.png")" >/dev/null; }
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1"; FAILED=1; fi; }
# 패널 헤더에서 상태 텍스트 (세션1)
state() { js "[...document.querySelectorAll('.panel .ph')].find(h=>h.querySelector('.name')?.textContent==='세션1')?.querySelector('.state')?.textContent??'없음'" | tr -d '"'; }
# 승인 단추 행 존재 여부
has_approve() { js "String(!!document.querySelector('.approve-row'))" | tr -d '"'; }
# 사이드바 첫 세션의 점 클래스
dot_class() { js "document.querySelector('.sess .dot')?.className??'없음'" | tr -d '"'; }
# 패널 대기 테두리
is_waiting_panel() { js "String(!!document.querySelector('.panel.panel-waiting'))" | tr -d '"'; }

launch() {
  HOME="$FAKE_HOME" LMUX_DATA_DIR="$(cygpath -w "$OUT/data")" \
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9336" \
    "$ROOT/src-tauri/target/release/Lmux.exe" >"$OUT/app.log" 2>&1 &
  sleep 5
  ab --cdp 9336 snapshot -i >/dev/null
  win -Show 5 >/dev/null
}

# ── 준비 ──────────────────────────────────────────────────────────────────────
echo '== 준비: 프로젝트·세션 추가'
launch
button '프로젝트 추가'; sleep 1
ab fill '#np-path' "$(cygpath -w "$OUT/work")" >/dev/null; sleep 1
button '추가하고 세션 열기'; sleep 1
ab fill '#ns-name' '세션1' >/dev/null; sleep 0.5
# bash shell 선택 (Git Bash 또는 bash)
js "[...document.querySelectorAll('.segment')].find(l => l.textContent.trim().toLowerCase().includes('bash')).querySelector('input').click()" >/dev/null
ab select '#ns-agent' claude >/dev/null
button '세션 열기'; sleep 1; button '닫기'; sleep 1

check '초기: 승인 단추 없음' '[[ $(has_approve) == false ]]'
shot 00-before-start

# ── 세션 시작 → UserPromptSubmit → Working ────────────────────────────────────
echo '== 세션 시작 → Working 상태'
js "document.querySelector('.term-error button')?.click()" >/dev/null
# bash 시작 + .bash_profile + claude 시작 + 0.3s delay + UserPromptSubmit = 약 2.5초
sleep 2.5
check '사이드바 점: working' '[[ $(dot_class) == *"working"* ]]'
check '상태 텍스트: 작업 중' '[[ $(state) == "작업 중" ]]'
check 'Working 중 승인 단추 없음' '[[ $(has_approve) == false ]]'
shot 01-working

# ── PermissionRequest → Waiting, 승인 단추 ────────────────────────────────────
echo '== PermissionRequest → Waiting 상태'
# fake claude는 2초 후 PermissionRequest 전송
sleep 2.5
check '사이드바 점: waiting' '[[ $(dot_class) == *"waiting"* ]]'
check '상태 텍스트: 승인 대기' '[[ $(state) == "승인 대기" ]]'
check '승인 단추 나타남' '[[ $(has_approve) == true ]]'
check '패널 주황 테두리' '[[ $(is_waiting_panel) == true ]]'
shot 02-waiting

# ── 틀린 토큰 테스트 ──────────────────────────────────────────────────────────
echo '== 틀린 토큰 → 404(상태 변화 없음)'
SESSION_ID=$(node -pe "try{JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')).sessions[0]?.id||''}catch(e){''}" "$(cygpath -w "$OUT/data/state.json")" 2>/dev/null | tr -d '"')
SETTINGS_FILE="$OUT/data/hooks/${SESSION_ID}.json"
URL_BASE=$(node -pe "
  try {
    const f = require('fs').readFileSync(process.argv[1], 'utf8');
    const s = JSON.parse(f);
    const cmd = s.hooks.UserPromptSubmit[0].hooks[0].command;
    const m = cmd.match(/http:\\/\\/127\\.0\\.0\\.1:[^\\/]+\\/event\\/[^\\/]+\\/[^\\/]+/);
    m ? m[0] : ''
  } catch(e) { '' }
" "$(cygpath -w "$SETTINGS_FILE")" 2>/dev/null | tr -d '"')

if [[ -n "$URL_BASE" ]]; then
  # 토큰 부분을 WRONG으로 교체
  WRONG_URL="${URL_BASE%/*}/WRONG-TOKEN"
  WRONG_STATUS=$(printf '{}' | curl.exe -s -o /dev/null -w "%{http_code}" -m 3 -X POST --data-binary @- "${WRONG_URL}/UserPromptSubmit" 2>/dev/null)
  check '틀린 토큰 → 404' '[[ $WRONG_STATUS == 404 ]]'
  sleep 0.5
  check '틀린 토큰 후 상태 변화 없음' '[[ $(state) == "승인 대기" ]]'
else
  echo "SKIP 틀린 토큰 테스트 (URL_BASE 추출 실패)"
fi

# ── 승인 클릭 → Stop → Done ───────────────────────────────────────────────────
echo '== 승인 클릭 → Done 상태'
button '1 · 예'; sleep 2
check '승인 후 단추 사라짐' '[[ $(has_approve) == false ]]'
check '완료 상태' '[[ $(state) == "완료" ]]'
check '사이드바 점: done' '[[ $(dot_class) == *"done"* ]]'
shot 03-done

echo '== 완료 후 승인 단추 없음(오래된 클릭 불가)'
check '완료 후 승인 단추 없음' '[[ $(has_approve) == false ]]'

# ── 세션 삭제 → 설정 파일 삭제 ──────────────────────────────────────────────
echo '== 세션 삭제 → 설정 파일 삭제됨'
js "document.querySelector('.panel .ph [aria-label=\"세션 삭제\"]')?.click()" >/dev/null; sleep 1
button '삭제'; sleep 1
HOOKS_DIR="$OUT/data/hooks"
check '설정 파일 삭제됨' '[[ -z "$(ls "$HOOKS_DIR" 2>/dev/null)" ]]'

# ── 오류 없음 ────────────────────────────────────────────────────────────────
echo '--- console'; ab console; echo '--- errors'; ab errors
check '화면 오류 없음' '[[ -z $(ab errors) ]]'

win -Close | tee "$OUT/close1.txt"
check '종료' 'grep -q "exited: True" "$OUT/close1.txt"'
ab close >/dev/null

# ── 회귀 ─────────────────────────────────────────────────────────────────────
echo '== 회귀: stage-2.sh·stage-3.sh'
bash stage-2.sh && bash stage-3.sh

echo "screenshots: $OUT"
if [ -z "$FAILED" ]; then echo '=== 모두 통과'; else echo '=== 실패 있음'; exit 1; fi
