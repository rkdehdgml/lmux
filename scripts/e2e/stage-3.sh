#!/usr/bin/env bash
# 3단계 실행 확인: 화면 분할(단추·단축키·끌어다 놓기·경계선·저장·재시작). Git Bash에서 실행.
# 필요: agent-browser, node, 먼저 `npx tauri build --no-bundle`. 실행 중인 Lmux가 없어야 한다.
# 임시 데이터 폴더와 임시 프로젝트만 쓰고 에이전트는 띄우지 않는다. 실행 중에는 앱 창을 건드리지 않는다.
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
export OUT="${OUT:-${TMPDIR:-/tmp}/lmux-e2e}/stage-3"
rm -rf "$OUT"; mkdir -p "$OUT/data" "$OUT/work"
S=lmux-e2e3
ab() { agent-browser --session $S "$@"; }
js() { ab eval "$1"; }
win() { powershell -NoProfile -ExecutionPolicy Bypass -File ./win.ps1 "$@"; }
button() { ab find role button click --name "$1" --exact >/dev/null; }
shot() { ab screenshot "$(cygpath -w "$OUT/$1.png")" >/dev/null; }
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1 [$(order)]"; FAILED=1; fi; }
# 패널 번호 순서의 세션 이름. 빈 패널은 "-".
order() { js "[...document.querySelectorAll('[data-pane]')].sort((a,b)=>a.dataset.pane-b.dataset.pane).map(p=>p.querySelector('.name')?.textContent??'-').join(',')" | tr -d '"'; }
focused() { js "document.querySelector('.pane.focused .name')?.textContent??'-'" | tr -d '"'; }
xterms() { js 'String(document.querySelectorAll(".xterm").length)' | tr -d '"'; }
saved() { node -pe "const s=require(process.argv[1]); $1" "$(cygpath -w "$OUT/data/state.json")"; }
key() { ab press "$1" >/dev/null; sleep 1; }
# 터미널에 친다. `keyboard type`은 터미널 밖에 초점이 있으면 초점을 터미널로 옮기므로 초점 확인에는 쓰지 않는다.
type_line() { ab keyboard type "$1" >/dev/null; ab press Enter >/dev/null; sleep 1; }
# 지금 초점이 있는 곳에 키를 하나씩 보낸다(초점을 옮기지 않는다).
press_line() { local i c; for ((i = 0; i < ${#1}; i++)); do c=${1:i:1}; [[ $c == ' ' ]] && c=Space; ab press "$c" >/dev/null; done; ab press Enter >/dev/null; sleep 1; }
# 키 입력이 초점 패널의 터미널로 가는 상태인지.
in_terminal() { js 'String(!!document.activeElement.closest(".pane.focused .xterm"))' | tr -d '"'; }
# JS 식이 가리키는 요소 안의 상대 위치를 화면 좌표로.
at() { js "(()=>{const r=($1).getBoundingClientRect(); return [Math.round(r.x+r.width*${2:-0.5}), Math.round(r.y+r.height*${3:-0.5})].join(' ')})()" | tr -d '"'; }
sess() { echo "[...document.querySelectorAll('.sess')].find(b=>b.querySelector('.sess-name').textContent==='$1')"; }
header() { echo "[...document.querySelectorAll('.pane .ph')].find(h=>h.querySelector('.name').textContent==='$1').querySelector('.dot')"; }
pane() { echo "document.querySelector('[data-pane=\"$1\"]')"; }
grab() { ab mouse move $1 >/dev/null; ab mouse down >/dev/null; }
move() { ab mouse move $(($1 + 12)) $2 >/dev/null; ab mouse move $1 $2 >/dev/null; sleep 1; }
release() { ab mouse up >/dev/null; sleep 1; }
# 초점이 있는 요소.
active() { js '(document.activeElement.getAttribute("aria-label")||document.activeElement.className)+"@"+(document.activeElement.closest("[data-pane]")?.dataset.pane??"-")' | tr -d '"'; }
click_at() { grab "$(at "$1")"; release; }
preview() { js 'document.querySelector(".drop-preview")?.textContent??""' | tr -d '"'; }

launch() {
  LMUX_DATA_DIR="$(cygpath -w "$OUT/data")" WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9334' \
    "$ROOT/src-tauri/target/release/Lmux.exe" >"$OUT/app.log" 2>&1 &
  sleep 5
  ab --cdp 9334 snapshot -i >/dev/null
  win -Show 5 >/dev/null
}
sidebar_new() { js "document.querySelector('.btn.wide').click()" >/dev/null; sleep 1; }
new_session() { # 이름
  sidebar_new
  ab fill '#ns-name' "$1" >/dev/null
  js "[...document.querySelectorAll('.segment')].find(l => l.textContent.trim().startsWith('cmd')).querySelector('input').click()" >/dev/null
  ab select '#ns-agent' none >/dev/null
  button '세션 열기'; sleep 4
}
# 지금 초점 셸이 어느 세션인지 파일에 남긴다.
who() { rm -f "$OUT/work/$1"; [[ $(in_terminal) == true ]] || { echo NOFOCUS; return; }; type_line "echo %ME%> $1"; sleep 1; tr -d ' \r\n' < "$OUT/work/$1" 2>/dev/null; }

echo '== 준비: 프로젝트와 첫 세션'
launch
button '프로젝트 추가'; sleep 1
ab fill '#np-path' "$(cygpath -w "$OUT/work")" >/dev/null; sleep 1
button '추가하고 세션 열기'; sleep 1; button '닫기'; sleep 1
new_session 'A'; type_line 'set ME=A'
check '첫 세션' '[[ $(order) == "A" && $(xterms) == 1 ]]'
press_line 'set ME=Z'
check '키를 하나씩 보내도 셸에 입력됨(아래 확인의 전제)' '[[ $(who k0.txt) == "Z" ]]'
type_line 'set ME=A'

echo '== 단축키 분할과 입력 대상'
key 'Control+\'
check 'Ctrl+\ 오른쪽 분할, 빈 패널에 초점' '[[ $(order) == "A,-" && $(focused) == "-" ]]'
press_line 'set ME=L'
check '빈 패널에서 친 글자 뒤에도 초점 유지' '[[ $(focused) == "-" && $(in_terminal) == false ]]'
key 'Control+1'
check '빈 패널에서 친 글자는 이전 셸로 가지 않음' '[[ $(who k-leak.txt) == "A" ]]'
new_session 'B'; type_line 'set ME=B'
check '새 세션은 빈 패널에 열림' '[[ $(order) == "A,B" && $(focused) == "B" && $(xterms) == 2 ]]'
key 'Control+1'
check 'Ctrl+1 패널 이동' '[[ $(focused) == "A" && $(who k1.txt) == "A" ]]'
key 'Control+2'
check 'Ctrl+2 패널 이동' '[[ $(who k2.txt) == "B" ]]'
key 'Control+Shift+\'
check 'Ctrl+Shift+\ 아래 분할' '[[ $(order) == "A,B,-" && $(js "document.querySelectorAll(\".divider.col\").length") == 1 ]]'
new_session 'C'; type_line 'set ME=C'
check '패널 3개' '[[ $(order) == "A,B,C" && $(xterms) == 3 ]]'
shot 01-three
key 'Control+Shift+Enter'
check '최대화' '[[ $(xterms) == 1 && $(order) == "C" ]]'
shot 02-maximized
key 'Control+Shift+Enter'
check '최대화 해제' '[[ $(order) == "A,B,C" && $(xterms) == 3 ]]'
sidebar_new; key 'Control+\'; button '닫기'; sleep 1
check '대화 상자가 열려 있으면 단축키 무시' '[[ $(order) == "A,B,C" ]]'

echo '== 초점'
click_at "$(header A)"
check '다른 패널 머리글을 누르면 그 터미널에 입력' '[[ $(focused) == "A" && $(who f1.txt) == "A" ]]'
click_at "$(header A)"
check '같은 패널 머리글을 눌러도 터미널에 입력' '[[ $(who f2.txt) == "A" ]]'
js "($(pane 1)).querySelector('[aria-label=\"세션 삭제\"]').focus()" >/dev/null; sleep 1
check '다른 패널 단추에 초점을 주면 빼앗지 않음' '[[ $(active) == "세션 삭제@1" && $(focused) == "B" ]]'
key 'Control+1'; key 'Control+F6'
check 'Ctrl+F6으로 터미널 밖으로' '[[ $(active) == "3개@-" ]]'
for _ in 1 2 3 4 5 6 7 8 9 10; do [[ $(active) == '패널 크기 조절'* ]] && break; key 'Tab'; done
check 'Tab으로 경계선에 닿음' '[[ $(active) == "패널 크기 조절"* ]]'
key 'Control+F6'
check 'Ctrl+F6으로 터미널에 돌아옴' '[[ $(who f3.txt) == "A" ]]'

echo '== 레이아웃 단추: 터미널을 다시 만들지 않음'
js "document.querySelectorAll('.xterm').forEach(x=>x.dataset.mark='1')" >/dev/null
marked() { js 'String(document.querySelectorAll(".xterm[data-mark]").length)' | tr -d '"'; }
button '4개'; sleep 1
check '4개' '[[ $(order) == "A,B,C,-" && $(marked) == 3 ]]'
key 'Control+\'
check '5번째 패널은 거절' '[[ $(order) == "A,B,C,-" && $(js "document.querySelector(\".banner\")?.textContent") == *"4개까지"* ]]'
button '좌우 ↔ 상하'; sleep 1
check '좌우 ↔ 상하' '[[ $(js "document.querySelectorAll(\".divider.col\").length") == 1 && $(marked) == 3 ]]'
shot 03-four-transposed
button '좌우 ↔ 상하'; sleep 1
key 'Control+2'; button '1개'; sleep 1
check '1개는 보던 세션을 남김' '[[ $(order) == "B" && $(xterms) == 1 ]]'
check '숨긴 실행 중 세션 표시' '[[ $(js "($(sess A)).innerText") == *"숨김"* ]]'
button '2개'; sleep 1; js "($(sess A)).click()" >/dev/null; sleep 2
check '빈 패널에 세션 열기' '[[ $(order) == "B,A" ]]'

echo '== 경계선'
rm -f "$OUT/work/before.txt"
grab "$(at "document.querySelector('.divider')")"; read -r X Y <<< "$(at "document.querySelector('.divider')")"; move $((X - 200)) $Y; release
check '끌어서 크기 조절(저장됨)' '[[ $(saved "s.layout.ratio < 0.45 && s.layout.ratio >= 0.15") == true ]]'
ab dblclick '.divider' >/dev/null; sleep 1
check '더블클릭하면 반반' '[[ $(saved "s.layout.ratio") == 0.5 ]]'
ab focus '.divider' >/dev/null; key ArrowLeft
check '방향키로 크기 조절' '[[ $(saved "s.layout.ratio.toFixed(2)") == 0.48 ]]'
grab "$(at "document.querySelector('.divider')")"; move 0 $Y; release
check '최소 크기에서 멈춤' '[[ $(saved "s.layout.ratio") == 0.15 ]]'
ab dblclick '.divider' >/dev/null; sleep 1
grab "$(at "document.querySelector('.divider')")"; read -r X Y <<< "$(at "document.querySelector('.divider')")"; move $((X - 150)) $Y
check '끄는 동안 비율 안내' '[[ $(js "document.querySelector(\".divider-tip\")?.textContent") == *"더블클릭하면 반반"* ]]'
shot 03b-divider
key Escape; release
check 'Esc는 크기 조절만 취소' '[[ $(saved "s.layout.ratio") == 0.5 && $(js "String(!!document.querySelector(\".divider-tip\"))") == *false* ]]'

echo '== 끌어다 놓기'
grab "$(at "$(sess C)")"; read -r X Y <<< "$(at "$(pane 0)" 0.95 0.5)"; move $X $Y
check '놓을 영역 미리 표시' '[[ $(preview) == *"오른쪽으로 분할"* ]]'
shot 04-drop-preview
release
check '사이드바에서 끌어 오른쪽 분할' '[[ $(order) == "B,C,A" && $(preview) == "" ]]'
grab "$(at "$(header A)")"; read -r X Y <<< "$(at "$(pane 0)")"; move $X $Y
check '자리 바꾸기 안내' '[[ $(preview) == *"자리를 바꿔요"* ]]'
release
check '머리글을 끌어 자리 바꾸기' '[[ $(order) == "A,C,B" ]]'
grab "$(at "$(header A)")"; read -r X Y <<< "$(at "$(pane 2)" 0.5 0.95)"; move $X $Y; release
check '아래로 옮기기(두 번 보이지 않음)' '[[ $(order) == "C,B,A" && $(xterms) == 3 ]]'
check '옮긴 터미널은 다시 만들지 않고 바로 입력' '[[ $(who k4.txt) == "A" ]]'
grab "$(at "$(header C)")"; read -r X Y <<< "$(at "$(pane 1)")"; move $X $Y
SEEN=$(preview); key Escape; release
check 'Esc로 취소' '[[ -n $SEEN && $(order) == "C,B,A" && $(preview) == "" ]]'
check '취소하면 터미널에 입력' '[[ $(in_terminal) == true ]]'
key 'Control+1'; FROM="$(at "$(sess A)")"
grab "$FROM"; move 140 20; release
check '패널 밖에 놓으면 그대로' '[[ $(order) == "C,B,A" && $(focused) == "C" ]]'
grab "$FROM"; read -r X Y <<< "$(at "$(pane 1)")"; move $X $Y; key Escape; read -r X Y <<< "$FROM"; move $X $Y; release
check '취소한 자리에서 놓아도 클릭은 실행 안 됨' '[[ $(order) == "C,B,A" && $(focused) == "C" ]]'
js "($(sess B)).dataset.x='1'" >/dev/null
grab "$(at "$(sess B)")"; release
check '움직이지 않으면 클릭' '[[ $(focused) == "B" && $(order) == "C,B,A" ]]'
key 'Control+Shift+1'
check 'Ctrl+Shift+1 자리 바꾸기(초점은 세션을 따라감)' '[[ $(order) == "B,C,A" && $(focused) == "B" && $(xterms) == 3 ]]'
key 'Control+Shift+2'
check 'Ctrl+Shift+2로 되돌림' '[[ $(order) == "C,B,A" ]]'
check '끌어 옮긴 뒤에도 입력이 맞는 셸로' '[[ $(who k3.txt) == "B" ]]'
shot 05-after-drag

echo '== 저장·불러오기'
button '저장 · 불러오기 ▾'; sleep 1
ab fill '#layout-name' '작업 화면' >/dev/null; button '지금 화면 저장'; sleep 1
check '저장됨' '[[ $(saved "s.savedLayouts[0].name") == "작업 화면" ]]'
button '1개'; sleep 1
button '저장 · 불러오기 ▾'; sleep 1; shot 06-saved-dialog; button '작업 화면 불러오기'; sleep 2
check '불러오면 배치 복원' '[[ $(order) == "C,B,A" && $(xterms) == 3 ]]'
button '작업 화면 ▾'; sleep 1; button '작업 화면 삭제'; sleep 1
check '삭제는 한 번 더 확인' '[[ $(saved "s.savedLayouts.length") == 1 ]]'
button '작업 화면 삭제 확인'; sleep 1
check '저장한 화면 삭제' '[[ $(saved "s.savedLayouts.length") == 0 ]]'
ab fill '#layout-name' '다시 저장' >/dev/null; button '지금 화면 저장'; sleep 1

echo '== 세션 삭제와 다시 열기'
js "[...document.querySelectorAll('.pane .ph')].find(h=>h.querySelector('.name').textContent==='A').querySelector('[aria-label=\"세션 삭제\"]').click()" >/dev/null; sleep 1
button '삭제'; sleep 1
check '삭제한 세션의 패널은 비움' '[[ $(order) == "C,B,-" ]]'
win -Close | tee "$OUT/close1.txt"
check '종료와 자식 정리' 'grep -q "exited: True" "$OUT/close1.txt" && grep -q "survivors: 0" "$OUT/close1.txt"'
ab close >/dev/null
launch
check '다시 열면 배치 복원' '[[ $(order) == "C,B,-" ]]'
check '자동 시작 없음' '[[ $(xterms) == 0 ]]'
shot 07-reopened
js "($(pane 0)).querySelector('.term-error button').click()" >/dev/null; sleep 4
check '시작 단추' '[[ $(xterms) == 1 ]]'
button '1개'; sleep 1
button '저장 · 불러오기 ▾'; sleep 1
button '다시 저장 불러오기'; sleep 2
check '지운 세션이 남은 저장 화면도 열림' '[[ $(order) == "C,B,-" && $(js "document.body.innerText") == *"다시 저장 ▾"* ]]'

echo '--- console'; ab console; echo '--- errors'; ab errors
check '화면 오류 없음' '[[ -z $(ab errors) ]]'
win -Close | tee "$OUT/close2.txt"
check '종료와 자식 정리(2)' 'grep -q "exited: True" "$OUT/close2.txt" && grep -q "survivors: 0" "$OUT/close2.txt"'
ab close >/dev/null
echo "screenshots: $OUT"
if [ -z "$FAILED" ]; then echo '=== 모두 통과'; else echo '=== 실패 있음'; exit 1; fi
