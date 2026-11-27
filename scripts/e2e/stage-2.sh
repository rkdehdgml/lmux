#!/usr/bin/env bash
# 2단계 실행 확인: 임시 데이터 폴더와 임시 프로젝트로 릴리스 exe를 조작한다. Git Bash에서 실행.
# 필요: agent-browser, node, 먼저 `npx tauri build --no-bundle`. 실행 중인 Lmux가 없어야 한다.
# 실제 claude·codex는 띄우지 않는다. 임시 HOME의 .bash_profile이 가짜 claude를 PATH 앞에 두고,
# Git Bash 세션에서만 claude를 고른다. 가짜가 먼저 잡히지 않으면 중단한다.
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
export OUT="${OUT:-${TMPDIR:-/tmp}/lmux-e2e}/stage-2"
rm -rf "$OUT"; mkdir -p "$OUT/data" "$OUT/일감 폴더/.svn" "$OUT/git-proj/.git" "$OUT/plain"
mkdir -p "$OUT/home/bin"
printf '#!/bin/sh\nprintf "[%%s]" "$#" "$@" >> claude-args.txt\necho >> claude-args.txt\n' > "$OUT/home/bin/claude"
chmod +x "$OUT/home/bin/claude"
# 시작 파일이 입력을 읽어도 에이전트 명령을 가져가면 안 된다.
printf 'export PATH="$HOME/bin:$PATH"\nread -t 2 -r TAKEN; echo "$TAKEN" >> "$HOME/taken.txt"\n' > "$OUT/home/.bash_profile"
FAKE_HOME="$(cygpath -w "$OUT/home")"
FOUND=$(HOME="$FAKE_HOME" "$(cygpath -u "$PROGRAMFILES")/Git/bin/bash.exe" --login -i -c 'command -v claude' 2>/dev/null)
[[ $FOUND == */home/bin/claude ]] || { echo "중단: 가짜 claude가 먼저 잡히지 않음 ($FOUND)"; exit 1; }
rm -f "$OUT/home/taken.txt"
S=lmux-e2e2
ab() { agent-browser --session $S "$@"; }
js() { ab eval "$1"; }
line() {
  # 실행 중에 사람이 창을 최소화하면 터미널이 해제된다. 상태를 남기고 복원한다.
  if [[ $(js 'String(document.querySelectorAll(".xterm").length)') == *0* ]]; then
    echo "알림: 터미널 없음 -> $(win -Show 9 | head -1) (복원 전 상태는 최소화 여부 확인)"; sleep 2
  fi
  ab focus '.xterm-helper-textarea' >/dev/null; ab keyboard type "$1" >/dev/null; ab press Enter >/dev/null
}
win() { powershell -NoProfile -ExecutionPolicy Bypass -File ./win.ps1 "$@"; }
button() { ab find role button click --name "$1" >/dev/null; }
shot() { ab screenshot "$(cygpath -w "$OUT/$1.png")" >/dev/null; }
choose_shell() { js "[...document.querySelectorAll('.segment')].find(l => l.textContent.trim().startsWith('$1')).querySelector('input').click()" >/dev/null; }
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1"; FAILED=1; fi; }
sidebar() { js 'document.querySelector(".sidebar").innerText.replace(/\n+/g," | ")'; }

launch() {
  HOME="$FAKE_HOME" LMUX_DATA_DIR="$(cygpath -w "$OUT/data")" \
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333' \
    "$ROOT/src-tauri/target/release/Lmux.exe" >"$OUT/app.log" 2>&1 &
  sleep 5
  ab --cdp 9333 snapshot -i >/dev/null
  win -Show 5 >/dev/null
}

add_project() { # 폴더
  button '프로젝트 추가'; sleep 1
  ab fill '#np-path' "$(cygpath -w "$OUT/$1")" >/dev/null; sleep 1
  echo "감지: $(js 'document.querySelector(".detected")?.innerText.replace(/\n+/g," ")')"
}

echo '== 빈 상태'
launch
check '빈 목록' '[[ $(js "document.body.innerText") == *"작업할 폴더를 추가해 주세요"* ]]'
shot 01-empty

echo '== 폴더 선택 창(열리는지만 확인하고 닫는다)'
button '프로젝트 추가'; sleep 1
button '찾아보기'; sleep 2
win -CloseDialog | tee "$OUT/dialog.txt"
check '폴더 선택 창이 열림' 'grep -q "dialog closed: 1" "$OUT/dialog.txt"'
sleep 1; button '닫기'; sleep 1

echo '== SVN 프로젝트 + cmd(셸만)'
add_project '일감 폴더'; shot 02-new-project
button '추가하고 세션 열기'; sleep 1
ab fill '#ns-name' '첫 세션' >/dev/null
shot 03-new-session
choose_shell 'cmd'; ab select '#ns-agent' none >/dev/null
button '세션 열기'; sleep 4
line 'echo first> first.txt'; sleep 2
check 'cmd에서 한글 폴더 시작' 'grep -q first "$OUT/일감 폴더/first.txt"'

echo '== PowerShell(셸만) 시작 폴더'
button '일감 폴더 새 세션'; sleep 1
ab fill '#ns-name' 'ps 세션' >/dev/null
js "[...document.querySelectorAll('.segment')].find(l => /PowerShell/.test(l.textContent)).querySelector('input').click()" >/dev/null
ab select '#ns-agent' none >/dev/null
button '세션 열기'; sleep 6
line '[IO.File]::WriteAllText("$PWD\m.txt", "ps-$PWD"); "psmarker-" + "visible"'; sleep 2
check 'PowerShell 시작 폴더' 'grep -q "일감 폴더" "$OUT/일감 폴더/m.txt"'
shot 04-powershell

echo '== Git 프로젝트 + cmd + 직접 입력 에이전트'
add_project 'git-proj'; button '추가하고 세션 열기'; sleep 1
ab fill '#ns-name' 'cmd 세션' >/dev/null
choose_shell 'cmd'
ab select '#ns-agent' custom >/dev/null
ab fill 'input[aria-label="실행할 명령"]' 'echo custom-started> custom.txt' >/dev/null
check '실행 중 세션 경고 없음(다른 프로젝트)' '[[ $(js "String(!!document.querySelector(\".warning\"))") == *false* ]]'
button '세션 열기'; sleep 4
check '직접 입력 명령 실행' 'grep -q custom-started "$OUT/git-proj/custom.txt"'
line 'cd> m.txt & echo cmd-%OS%>> m.txt & echo 한글cmd>> m.txt'; sleep 2
check 'cmd 시작 폴더와 한글' 'grep -q "git-proj" "$OUT/git-proj/m.txt" && grep -q "한글cmd" "$OUT/git-proj/m.txt"'
shot 05-cmd

echo '== 같은 프로젝트의 두 번째 세션 경고 + Git Bash'
button 'git-proj 새 세션'; sleep 1
check '같은 폴더 경고' '[[ $(js "document.querySelector(\".warning\")?.innerText") == *"cmd 세션"* ]]'
ab fill '#ns-name' 'bash 세션' >/dev/null
choose_shell 'Git Bash'; ab select '#ns-agent' none >/dev/null
button '세션 열기'; sleep 5
line '{ pwd -W; uname -o; echo 한글bash; } > bash.txt'; sleep 2
check 'Git Bash 시작 폴더' 'grep -q "git-proj" "$OUT/git-proj/bash.txt" && grep -q Msys "$OUT/git-proj/bash.txt"'
shot 06-gitbash

echo '== 일반 폴더 + WSL'
add_project 'plain'; button '추가하고 세션 열기'; sleep 1
HAS_WSL=$(js 'String([...document.querySelectorAll(".segment")].some(l => l.textContent.includes("WSL")))')
if [[ $HAS_WSL == *true* ]]; then
  ab fill '#ns-name' 'wsl 세션' >/dev/null
  choose_shell 'WSL'; ab select '#ns-agent' none >/dev/null
  button '세션 열기'; sleep 8
  line '{ pwd; uname -s; } > wsl.txt'; sleep 3
  check 'WSL 시작 폴더' 'grep -q "/plain" "$OUT/plain/wsl.txt" && grep -q Linux "$OUT/plain/wsl.txt"'
  shot 07-wsl
else
  button '닫기'; echo 'SKIP WSL(배포판 없음)'
fi
echo "사이드바: $(sidebar)"

echo '== 세션 전환: 숨긴 동안의 출력과 이전 화면 유지, 입력이 섞이지 않음'
js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('cmd 세션')).click()" >/dev/null; sleep 1
line 'ping -n 4 127.0.0.1 >nul & echo hidden-done'
for name in 'ps 세션' 'bash 세션' 'ps 세션'; do
  js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('$name')).click()" >/dev/null; sleep 1
done
check '터미널은 하나만 렌더링' '[[ $(js "String(document.querySelectorAll(\".xterm\").length)") == *1* ]]'
shot 08-back-to-powershell
sleep 4
js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('cmd 세션')).click()" >/dev/null; sleep 2
shot 09-back-to-cmd

echo '== 첫 지시 전달(Git Bash의 가짜 claude, 입력을 읽는 시작 파일)'
button '일감 폴더 새 세션'; sleep 1
check '새 세션 창은 이름 칸에 포커스' '[[ $(js "document.activeElement.id") == *ns-name* ]]'
ab fill '#ns-name' '지시 세션 - 이름이 아주아주아주아주아주 길어도 사이드바 폭은 그대로여야 한다' >/dev/null
choose_shell 'Git Bash'; ab select '#ns-agent' claude >/dev/null; sleep 1
ab fill 'textarea[aria-label="첫 지시"]' '고쳐 줘 a&echo INJECTED "따옴표" $(id) %PATH%' >/dev/null
button '세션 열기'; sleep 8
ARGS="$OUT/일감 폴더/claude-args.txt"
EXPECTED='[1][고쳐 줘 a&echo INJECTED "따옴표" $(id) %PATH%]'
check '첫 지시가 인수 하나로 전달됨' '[[ $(head -1 "$ARGS") == "$EXPECTED" ]]'
shot 05b-first-prompt
check '긴 이름에도 사이드바 폭 유지' '[[ $(js "String(document.querySelector(\".sidebar\").offsetWidth)") == *280* ]]'
check '시작 파일은 명령을 가져가지 못함' '[[ -s "$OUT/home/taken.txt" && -z $(tr -d "[:space:]" < "$OUT/home/taken.txt") ]]'
sleep 3
line 'echo after-agent > after.txt'; sleep 2
check '에이전트가 끝나면 셸을 계속 씀' 'grep -q after-agent "$OUT/일감 폴더/after.txt"'
check '첫 지시는 한 번만' '[[ $(wc -l < "$ARGS") -eq 1 ]]'
check '첫 지시는 저장 안 함' '! grep -q INJECTED "$OUT/data/state.json"'

echo '== 전환 직후 입력이 고른 세션에만 간다'
open_by() { js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('$1')).click()" >/dev/null; sleep 1; }
open_by 'ps 세션'; line '[IO.File]::WriteAllText("$PWD\iso.txt", "only-ps")'; sleep 1
open_by 'cmd 세션'; line 'echo only-cmd> iso.txt'; sleep 2
check '각 폴더에 각 셸의 입력만' 'grep -q only-ps "$OUT/일감 폴더/iso.txt" && grep -q only-cmd "$OUT/git-proj/iso.txt" && ! grep -q only-ps "$OUT/git-proj/iso.txt"'

echo '== 종료 상태와 다시 시작'
rm -f "$OUT/git-proj/custom.txt"
line 'exit 7'; sleep 2
check '종료 코드 표시' '[[ $(sidebar) == *"종료됨 (코드 7)"* ]]'
button '다시 시작'; sleep 4
check '다시 시작하면 에이전트 명령 재실행' 'grep -q custom-started "$OUT/git-proj/custom.txt"'

echo '== 세션 삭제(확인 후)'
js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('bash 세션')).click()" >/dev/null; sleep 1
button '세션 삭제'; sleep 1; shot 10-confirm-delete
check '확인 전에는 남아 있음' '[[ $(sidebar) == *"bash 세션"* ]]'
button '삭제'; sleep 1
check '삭제됨' '[[ $(sidebar) != *"bash 세션"* ]]'

echo '== 최소화·복원과 새로고침'
open_cmd() { js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('cmd 세션')).click()" >/dev/null; sleep 2; }
xterms() { js 'String(document.querySelectorAll(".xterm").length)'; }
open_cmd
win -Show 6 >/dev/null; sleep 3
check '최소화 중 터미널 해제' '[[ $(xterms) == *0* ]]'
win -Show 9 >/dev/null; sleep 2
check '복원하면 다시 그림' '[[ $(xterms) == *1* ]]'
rm -f "$OUT/git-proj/custom.txt"
ab eval 'location.reload(), 1' >/dev/null; sleep 5
open_cmd
line 'echo reload-ok> reload.txt'; sleep 2
check '새로고침 뒤 실행 중인 셸에 다시 붙음' 'grep -q reload-ok "$OUT/git-proj/reload.txt"'
check '다시 붙을 때 에이전트 명령을 다시 입력하지 않음' '! test -f "$OUT/git-proj/custom.txt"'

echo '== 앱 닫기와 다시 열기'
shot 11-before-close
win -Close | tee "$OUT/close1.txt"
check '종료와 자식 정리' 'grep -q "exited: True" "$OUT/close1.txt" && grep -q "survivors: 0" "$OUT/close1.txt"'
ab close >/dev/null
rm -rf "$OUT/plain"
launch
check '목록 유지, 자동 시작 없음' '[[ $(sidebar) == *"첫 세션"* && $(sidebar) == *"시작 전"* && $(sidebar) != *"실행 중"* ]]'
shot 12-reopened
if [[ $HAS_WSL == *true* ]]; then
  js "[...document.querySelectorAll('.sess')].find(b => b.innerText.includes('wsl 세션')).click()" >/dev/null; sleep 2
  check '없어진 폴더는 오류' '[[ $(js "document.querySelector(\".term-error\")?.innerText") == *"프로젝트 폴더가 없어요"* ]]'
  shot 13-missing-folder
fi

echo '== 프로젝트 등록 해제(파일은 남는다)'
button 'git-proj 등록 해제'; sleep 1; button '등록 해제'; sleep 1
check '목록에서 제거' '[[ $(sidebar) != *"git-proj"* ]]'
check '파일 유지' 'test -f "$OUT/git-proj/m.txt"'

echo '--- console'; ab console; echo '--- errors'; ab errors
win -Close | tee "$OUT/close2.txt"
check '종료와 자식 정리(2)' 'grep -q "exited: True" "$OUT/close2.txt" && grep -q "survivors: 0" "$OUT/close2.txt"'
ab close >/dev/null
echo "screenshots: $OUT"
if [ -z "$FAILED" ]; then echo '=== 모두 통과'; else echo '=== 실패 있음'; exit 1; fi
