export default function SetAbout() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 22 }}>
      {/* Header card */}
      <div style={{ display: "flex", alignItems: "center", gap: 18, background: "#14171D", borderRadius: 14, padding: 22, flexWrap: "wrap" }}>
        <span style={{ width: 56, height: 56, borderRadius: 15, background: "#2F6FE4", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
          <svg viewBox="0 0 24 24" aria-hidden="true" style={{ width: 28, height: 28, fill: "none", stroke: "#fff", strokeWidth: 1.8, strokeLinecap: "round", strokeLinejoin: "round" }}>
            <rect x={3} y={3} width={8} height={8} rx={2} />
            <rect x={13} y={3} width={8} height={8} rx={2} />
            <rect x={3} y={13} width={8} height={8} rx={2} />
            <rect x={13} y={13} width={8} height={8} rx={2} />
          </svg>
        </span>
        <div style={{ flex: 1, minWidth: 200 }}>
          <h2 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>Lmux</h2>
          <div style={{ fontSize: 13, color: "#A0A8B8", marginTop: 3 }}>
            가벼운 AI 에이전트 멀티플렉서 · <span className="mono">v0.1.0</span>
          </div>
          <div style={{ fontSize: 12.5, color: "#4CC79A", marginTop: 6 }}>✓ 최신 버전이에요 · 방금 확인</div>
        </div>
        <button disabled className="btn" title="다음 버전에서 추가돼요">업데이트 확인</button>
      </div>

      {/* 업데이트 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>업데이트</h3>
        <div className="settings-card">
          <div className="settings-item" style={{ opacity: 0.5 }}>
            <span style={{ flex: 1 }}>자동으로 내려받고 다음 실행 때 설치</span>
            <button disabled className="settings-sw on" role="switch" aria-checked={true} aria-label="자동 업데이트" title="다음 버전에서 추가돼요"><span /></button>
          </div>
          <div className="settings-item" style={{ opacity: 0.5 }}>
            <span style={{ flex: 1 }}>
              업데이트 채널
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>베타는 새 기능을 먼저 받지만 불안정할 수 있어요</span>
            </span>
            <div aria-label="채널 (다음 버전)" style={{ display: "flex", gap: 2, background: "#0E1014", borderRadius: 8, padding: 3 }}>
              <button disabled className="btn" style={{ padding: "5px 12px", background: "#262C38" }}>안정</button>
              <button disabled className="btn" style={{ padding: "5px 12px", background: "transparent", color: "#A0A8B8" }}>베타</button>
            </div>
          </div>
          <div className="settings-item" style={{ opacity: 0.5 }}>
            <span style={{ flex: 1 }}>에이전트가 일하는 중에는 설치 미루기</span>
            <button disabled className="settings-sw on" role="switch" aria-checked={true} aria-label="작업 중 설치 미루기" title="다음 버전에서 추가돼요"><span /></button>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>변경 기록</span>
            <span style={{ fontSize: 13, color: "#A0A8B8" }}>GitHub Releases에서 보기</span>
          </div>
        </div>
      </section>

      {/* 오픈소스 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>오픈소스</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>저장소</span>
            <span className="mono" style={{ fontSize: 13, color: "#A0A8B8" }}>github.com/[아이디]/lmux</span>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>라이선스</span>
            <span style={{ color: "#A0A8B8" }}>MIT</span>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>사용한 오픈소스</span>
            <span style={{ fontSize: 13, color: "#A0A8B8" }}>Tauri, xterm.js 외 목록 보기</span>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>버그 신고 · 기능 제안</span>
            <span style={{ fontSize: 13, color: "#A0A8B8" }}>GitHub Issues 열기</span>
          </div>
        </div>
      </section>

      {/* 진단 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>진단</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              익명 사용 통계 보내기
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>기본은 꺼짐이에요. 켜도 경로, 파일, 터미널 내용은 보내지 않아요</span>
            </span>
            <button className="settings-sw" role="switch" aria-checked={false} aria-label="익명 통계"><span /></button>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              진단 로그 내보내기
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>이슈에 첨부할 수 있게 경로와 토큰을 가린 로그를 만들어요</span>
            </span>
            <button className="btn">내보내기</button>
          </div>
        </div>
      </section>
    </div>
  );
}
