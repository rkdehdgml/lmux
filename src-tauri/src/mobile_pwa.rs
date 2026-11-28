//! 모바일 PWA 페이지 — 바이너리에 포함된 HTML/JS/JSON 상수.

pub const PAIR_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 기기 연결</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  body{font-family:system-ui,sans-serif;background:#0d0d0d;color:#e8e8e2;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}
  .card{background:#1a1a1a;border-radius:16px;padding:32px;max-width:360px;width:100%;text-align:center}
  h1{font-size:1.3rem;margin:0 0 24px}
  #code{font-size:2.5rem;letter-spacing:.3em;color:#a3e635;font-weight:700;margin:16px 0}
  #status{font-size:.9rem;color:#9ca3af;margin-top:16px}
  .err{color:#f87171}
</style>
</head>
<body>
<div class="card">
  <h1>Lmux 기기 연결</h1>
  <p id="status">PC에서 표시한 6자리 코드를 확인해 주세요</p>
  <div id="code">------</div>
  <p id="msg"></p>
</div>
<script>
const params = new URLSearchParams(location.search);
const token = params.get('token') || '';
async function begin() {
  const r = await fetch('/pair/begin', {
    method:'POST',
    headers:{'content-type':'application/json'},
    body:JSON.stringify({token, deviceName: navigator.userAgent.split(' ').slice(-1)[0]})
  });
  const data = await r.json();
  if (!r.ok) { document.getElementById('msg').textContent = data.error; return; }
  document.getElementById('code').textContent = data.code6;
  poll(data.pairId);
}
async function poll(id) {
  for (let i=0;i<120;i++) {
    await new Promise(r=>setTimeout(r,3000));
    const r = await fetch('/pair/status/'+id);
    const d = await r.json();
    if (d.status==='approved') {
      localStorage.setItem('device_token', d.deviceToken);
      location.href = '/pwa/sessions';
      return;
    }
    if (d.status==='rejected') { document.getElementById('msg').textContent='PC에서 거부했어요'; return; }
  }
  document.getElementById('msg').textContent='시간이 초과됐어요. 다시 QR을 스캔해 주세요';
}
begin();
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const SESSIONS_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 세션</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  *{box-sizing:border-box}
  body{font-family:system-ui,sans-serif;background:#0d0d0d;color:#e8e8e2;margin:0;padding-bottom:60px}
  h1{font-size:1.1rem;padding:16px;margin:0;border-bottom:1px solid #2a2a2a}
  .list{padding:8px}
  .item{background:#1a1a1a;border-radius:12px;padding:16px;margin-bottom:8px;cursor:pointer;display:flex;align-items:center;gap:12px}
  .item:active{background:#252525}
  .dot{width:8px;height:8px;border-radius:50%;background:#6b7280;flex-shrink:0}
  .dot.running{background:#a3e635}
  .name{font-size:.95rem;font-weight:600}
  .proj{font-size:.8rem;color:#9ca3af;margin-top:2px}
  .nav{position:fixed;bottom:0;left:0;right:0;background:#1a1a1a;border-top:1px solid #2a2a2a;display:flex;height:56px}
  .nav a{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#9ca3af;text-decoration:none;font-size:.7rem;gap:2px}
  .nav a.active{color:#a3e635}
</style>
</head>
<body>
<h1>세션</h1>
<div id="list" class="list"><p style="color:#6b7280;padding:16px">불러오는 중…</p></div>
<nav class="nav">
  <a href="/pwa/sessions" class="active">세션</a>
  <a href="/pwa/files">파일</a>
  <a href="/pwa/alerts">알림</a>
</nav>
<script>
const token = localStorage.getItem('device_token') || '';
function esc(s){return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');}
async function load() {
  const r = await fetch('/api/sessions', {headers:{'Authorization':'Bearer '+token}});
  if (!r.ok) { location.href='/pwa/pair'; return; }
  const sessions = await r.json();
  const el = document.getElementById('list');
  if (!sessions.length) { el.innerHTML='<p style="color:#6b7280;padding:16px">세션이 없어요</p>'; return; }
  el.innerHTML = sessions.map(s=>
    `<div class="item" onclick="open_session('${s.id}')">
      <div class="dot ${s.running?'running':''}"></div>
      <div><div class="name">${esc(s.name)}</div><div class="proj">${esc(s.projectName)}</div></div>
    </div>`
  ).join('');
}
function open_session(id) { location.href='/pwa/terminal/'+id; }
load();
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const TERMINAL_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 터미널</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  *{box-sizing:border-box}
  body{font-family:'Courier New',monospace;background:#0d0d0d;color:#e8e8e2;margin:0;display:flex;flex-direction:column;height:100dvh}
  #hdr{background:#1a1a1a;padding:12px 16px;font-size:.85rem;border-bottom:1px solid #2a2a2a;display:flex;align-items:center;gap:8px}
  #hdr button{background:none;border:none;color:#9ca3af;cursor:pointer;font-size:1rem;padding:0 4px}
  #out{flex:1;overflow-y:auto;padding:12px;white-space:pre-wrap;word-break:break-all;font-size:.82rem;line-height:1.5}
  #inp{background:#1a1a1a;border-top:1px solid #2a2a2a;display:flex;padding:8px}
  #inp input{flex:1;background:#252525;border:none;color:#e8e8e2;padding:8px 12px;border-radius:8px;font-family:inherit;font-size:.9rem;outline:none}
  #inp button{background:#a3e635;color:#0d0d0d;border:none;padding:8px 14px;border-radius:8px;margin-left:8px;font-weight:700;cursor:pointer}
</style>
</head>
<body>
<div id="hdr"><button onclick="history.back()">←</button><span id="sid"></span></div>
<div id="out"></div>
<div id="inp">
  <input id="text" placeholder="명령 입력…" autocomplete="off" autocorrect="off" autocapitalize="none" spellcheck="false">
  <button onclick="send()">보내기</button>
</div>
<script>
const token = localStorage.getItem('device_token') || '';
const sid = location.pathname.split('/').pop();
document.getElementById('sid').textContent = sid;
let run = 0;
const out = document.getElementById('out');
const dec = new TextDecoder();

async function getSessions() {
  const r = await fetch('/api/sessions', {headers:{'Authorization':'Bearer '+token}});
  if (!r.ok) return;
  const sessions = await r.json();
  const s = sessions.find(s=>s.id===sid);
  if (s) { run = s.run || 0; }
}

function appendText(text) {
  out.textContent += text;
  out.scrollTop = out.scrollHeight;
}

function connect() {
  const es = new EventSource('/api/terminal/'+sid+'/stream', {
    headers: { Authorization: 'Bearer '+token }
  });
  // EventSource doesn't support custom headers natively — use fetch stream
  fetchStream();
}

async function fetchStream() {
  const r = await fetch('/api/terminal/'+sid+'/stream', {
    headers:{'Authorization':'Bearer '+token}
  });
  if (!r.ok) { appendText('[연결 실패]\n'); return; }
  const reader = r.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  while (true) {
    const {done, value} = await reader.read();
    if (done) break;
    buf += decoder.decode(value, {stream:true});
    const lines = buf.split('\n');
    buf = lines.pop();
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const b64 = line.slice(6);
        const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
        appendText(dec.decode(bytes, {stream:true}));
      }
    }
  }
  // 서버가 5분 타임아웃으로 연결을 끊은 경우 자동 재연결
  getSessions().then(()=>setTimeout(fetchStream, 1500));
}

async function send() {
  const inp = document.getElementById('text');
  const text = inp.value;
  if (!text) return;
  const r = await fetch('/api/terminal/'+sid+'/input', {
    method:'POST',
    headers:{'Authorization':'Bearer '+token,'content-type':'application/json'},
    body:JSON.stringify({text:text+'\n', run})
  });
  if (r.ok) {
    inp.value = '';
  } else if (r.status === 403) {
    appendText('[권한 없음: 입력 권한이 필요합니다]\n');
  } else if (r.status === 409) {
    const d = await r.json().catch(()=>({}));
    if (d.staleRun && d.currentRun) { run = d.currentRun; }
    appendText('[세션이 재시작됐습니다. 다시 시도하세요]\n');
  } else {
    appendText('[전송 실패: '+r.status+']\n');
  }
}
document.getElementById('text').addEventListener('keydown', e=>{if(e.key==='Enter')send();});
getSessions().then(()=>connect());
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const FILES_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 파일</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  *{box-sizing:border-box}
  body{font-family:system-ui,sans-serif;background:#0d0d0d;color:#e8e8e2;margin:0;padding-bottom:60px}
  h1{font-size:1.1rem;padding:16px;margin:0;border-bottom:1px solid #2a2a2a}
  select{width:100%;background:#1a1a1a;color:#e8e8e2;border:1px solid #2a2a2a;padding:12px 16px;font-size:.9rem;border-radius:0;border-left:none;border-right:none}
  .breadcrumb{padding:8px 16px;font-size:.8rem;color:#6b7280;border-bottom:1px solid #2a2a2a}
  .list{padding:8px}
  .item{display:flex;align-items:center;gap:10px;padding:10px 12px;border-radius:10px;cursor:pointer}
  .item:active{background:#1a1a1a}
  .ico{font-size:1.2rem}
  .nm{font-size:.9rem}
  .sz{font-size:.75rem;color:#6b7280;margin-top:1px}
  .lock{font-size:.75rem;color:#f97316;margin-left:4px}
  .nav{position:fixed;bottom:0;left:0;right:0;background:#1a1a1a;border-top:1px solid #2a2a2a;display:flex;height:56px}
  .nav a{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#9ca3af;text-decoration:none;font-size:.7rem;gap:2px}
  .nav a.active{color:#a3e635}
</style>
</head>
<body>
<h1>파일</h1>
<select id="proj"></select>
<div class="breadcrumb" id="bread">/</div>
<div id="list" class="list"></div>
<nav class="nav">
  <a href="/pwa/sessions">세션</a>
  <a href="/pwa/files" class="active">파일</a>
  <a href="/pwa/alerts">알림</a>
</nav>
<script>
const token = localStorage.getItem('device_token')||'';
let projId='', curPath='';
function esc(s){return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');}
function jsStr(s){return s.replace(/\\/g,'\\\\').replace(/'/g,"\\'");}
async function loadProjects() {
  const r = await fetch('/api/files',{headers:{'Authorization':'Bearer '+token}});
  if(!r.ok){location.href='/pwa/pair';return;}
  const ps = await r.json();
  const sel = document.getElementById('proj');
  sel.innerHTML = ps.map(p=>`<option value="${esc(p.id)}">${esc(p.name)}</option>`).join('');
  sel.onchange = ()=>{ projId=sel.value; curPath=''; loadDir(); };
  projId = ps[0]?.id||'';
  loadDir();
}
async function loadDir() {
  const url = '/api/files/'+projId+(curPath?'?path='+encodeURIComponent(curPath):'');
  const r = await fetch(url,{headers:{'Authorization':'Bearer '+token}});
  if(!r.ok) return;
  const items = await r.json();
  document.getElementById('bread').textContent = '/'+curPath;
  document.getElementById('list').innerHTML = (curPath?
    `<div class="item" onclick="goUp()"><div class="ico">📁</div><div><div class="nm">..</div></div></div>`:'')+
    items.sort((a,b)=>b.isDir-a.isDir||a.name.localeCompare(b.name)).map(it=>
      `<div class="item" data-name="${esc(it.name)}" data-isdir="${it.isDir}" onclick="open_item(this)">
        <div class="ico">${it.isDir?'📁':'📄'}</div>
        <div><div class="nm">${esc(it.name)}</div>
        ${!it.isDir?`<div class="sz">${it.sizeBytes!=null?Math.ceil(it.sizeBytes/1024)+'KB':''}</div>`:''}
        </div></div>`
    ).join('');
}
function goUp(){ curPath=curPath.split('/').slice(0,-1).join('/'); loadDir(); }
function open_item(el){
  const name=el.dataset.name, isDir=el.dataset.isdir==='true';
  if(isDir){curPath=(curPath?curPath+'/':'')+name;loadDir();}
  else{
    const path=(curPath?curPath+'/':'')+name;
    location.href='/pwa/fileview?proj='+projId+'&path='+encodeURIComponent(path);
  }
}
loadProjects();
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const ALERTS_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 알림</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  *{box-sizing:border-box}
  body{font-family:system-ui,sans-serif;background:#0d0d0d;color:#e8e8e2;margin:0;padding-bottom:60px}
  h1{font-size:1.1rem;padding:16px;margin:0;border-bottom:1px solid #2a2a2a}
  .list{padding:8px}
  .item{background:#1a1a1a;border-radius:10px;padding:12px;margin-bottom:6px}
  .time{font-size:.75rem;color:#6b7280}
  .action{font-size:.85rem;color:#a3e635;font-weight:600;margin:4px 0 2px}
  .target{font-size:.8rem;color:#9ca3af}
  .nav{position:fixed;bottom:0;left:0;right:0;background:#1a1a1a;border-top:1px solid #2a2a2a;display:flex;height:56px}
  .nav a{flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#9ca3af;text-decoration:none;font-size:.7rem;gap:2px}
  .nav a.active{color:#a3e635}
</style>
</head>
<body>
<h1>알림 / 접속 기록</h1>
<div id="list" class="list"><p style="color:#6b7280;padding:8px">불러오는 중…</p></div>
<nav class="nav">
  <a href="/pwa/sessions">세션</a>
  <a href="/pwa/files">파일</a>
  <a href="/pwa/alerts" class="active">알림</a>
</nav>
<script>
const token = localStorage.getItem('device_token')||'';
function esc(s){return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');}
async function load() {
  const r = await fetch('/api/alerts',{headers:{'Authorization':'Bearer '+token}});
  if(!r.ok){location.href='/pwa/pair';return;}
  const items = await r.json();
  const el = document.getElementById('list');
  if(!items.length){el.innerHTML='<p style="color:#6b7280;padding:8px">기록이 없어요</p>';return;}
  el.innerHTML = [...items].reverse().map(it=>
    `<div class="item">
      <div class="time">${new Date(it.timestampMs).toLocaleString('ko')}</div>
      <div class="action">${esc(it.action)}</div>
      <div class="target">${esc(it.target)}</div>
    </div>`
  ).join('');
}
load();
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const FILEVIEW_HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Lmux — 파일 보기</title>
<link rel="manifest" href="/pwa/manifest.json">
<style>
  *{box-sizing:border-box}
  body{font-family:'Courier New',monospace;background:#0d0d0d;color:#e8e8e2;margin:0;display:flex;flex-direction:column;height:100dvh}
  #hdr{background:#1a1a1a;padding:12px 16px;font-size:.85rem;border-bottom:1px solid #2a2a2a;display:flex;align-items:center;gap:8px}
  #hdr button{background:none;border:none;color:#9ca3af;cursor:pointer;font-size:1rem;padding:0 4px}
  #path{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#9ca3af;font-size:.8rem}
  #content{flex:1;overflow:auto;padding:12px;white-space:pre;font-size:.82rem;line-height:1.5}
  .err{color:#f87171;white-space:normal}
</style>
</head>
<body>
<div id="hdr">
  <button onclick="history.back()">&#8592;</button>
  <div id="path"></div>
</div>
<pre id="content">불러오는 중…</pre>
<script>
const p = new URLSearchParams(location.search);
const proj = p.get('proj')||'', path = p.get('path')||'';
const token = localStorage.getItem('device_token')||'';
document.getElementById('path').textContent = path;
async function load() {
  if(!proj||!path){document.getElementById('content').textContent='파라미터 없음';return;}
  const r = await fetch('/api/files/'+proj+'/content?path='+encodeURIComponent(path),
    {headers:{'Authorization':'Bearer '+token}});
  const el = document.getElementById('content');
  if(r.status===401){location.href='/pwa/pair';return;}
  if(!r.ok){const d=await r.json();el.className='err';el.textContent=d.error||'오류';return;}
  const d = await r.json();
  el.textContent = d.content;
}
load();
if('serviceWorker' in navigator) navigator.serviceWorker.register('/pwa/sw.js');
</script>
</body>
</html>"#;

pub const MANIFEST_JSON: &str = r###"{
  "name": "Lmux Mobile",
  "short_name": "Lmux",
  "start_url": "/pwa/sessions",
  "display": "standalone",
  "background_color": "#0d0d0d",
  "theme_color": "#1a1a1a",
  "icons": []
}"###;

pub const SW_JS: &str = r#"
// Lmux PWA Service Worker
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', e => e.waitUntil(clients.claim()));
self.addEventListener('push', e => {
  const data = e.data ? e.data.json() : {};
  e.waitUntil(self.registration.showNotification(
    data.title || 'Lmux',
    { body: data.body || '', icon: '/pwa/icon.png' }
  ));
});
"#;
