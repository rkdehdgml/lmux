type ShortcutRow = {
  label: string;
  keys: string[];
  conflict?: string;
};

type Section = { title: string; rows: ShortcutRow[] };

const SECTIONS: Section[] = [
  {
    title: "세션",
    rows: [
      { label: "새 세션", keys: ["Ctrl+N"] },
      { label: "세션 닫기", keys: ["Ctrl+W"] },
      { label: "다음 · 이전 세션", keys: ["Ctrl+Tab", "Ctrl+Shift+Tab"] },
      { label: "입력 기다리는 세션으로 바로 가기", keys: ["Ctrl+J"] },
    ],
  },
  {
    title: "화면 분할",
    rows: [
      { label: "오른쪽으로 분할", keys: ["Ctrl+\\"] },
      { label: "아래로 분할", keys: ["Ctrl+Shift+\\"] },
      { label: "패널로 이동", keys: ["Ctrl+1 ~ 4"] },
      { label: "지금 패널 크게 보기 · 되돌리기", keys: ["Ctrl+Shift+Enter"] },
      { label: "패널 크기 똑같이 맞추기", keys: ["Ctrl+Shift+="] },
    ],
  },
  {
    title: "에이전트",
    rows: [
      { label: "승인 · 세션 동안 허용 · 거절", keys: ["Alt+1", "Alt+2", "Alt+3"] },
      { label: "/compact 보내기", keys: ["Ctrl+Shift+K"] },
      { label: "변경 사항 · 커밋 열기", keys: ["Ctrl+Shift+G"] },
    ],
  },
  {
    title: "앱",
    rows: [
      { label: "검색 · 명령", keys: ["Ctrl+K"] },
      { label: "설정", keys: ["Ctrl+,"] },
      {
        label: "사이드바 숨기기 · 보이기",
        keys: ["Ctrl+B"],
        conflict: "Claude Code의 Ctrl+B와 겹쳐요. 터미널에 포커스가 있으면 터미널로 보내요",
      },
    ],
  },
];

export default function SetKeys() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <div style={{ display: "flex", alignItems: "flex-end", gap: 10, flexWrap: "wrap" }}>
        <div style={{ flex: 1 }}>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>단축키</h2>
          <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--muted)" }}>
            터미널 안에서는 Ctrl+C 같은 셸 단축키가 먼저예요. 겹치면 표시해 드려요.
          </p>
        </div>
        <button className="btn">기본값으로</button>
      </div>

      <input
        placeholder="동작이나 키로 찾기 (예: 분할, Ctrl+K)"
        aria-label="단축키 검색"
        style={{
          fontFamily: "inherit", fontSize: 13.5, background: "#14171D", border: 0,
          borderRadius: 10, color: "#E8EAF0", padding: "12px 14px", outline: "none",
        }}
      />

      {SECTIONS.map((sec) => (
        <section key={sec.title}>
          <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>{sec.title}</h3>
          <div className="settings-card" style={{ padding: "0 20px" }}>
            {sec.rows.map((row, i) => (
              <div
                key={row.label}
                style={{
                  display: "flex", alignItems: "center", gap: 12,
                  padding: row.conflict ? "11px 20px" : "11px 0",
                  borderTop: i > 0 ? "1px solid #1F242D" : undefined, fontSize: 13.5,
                  background: row.conflict ? "#221B0E" : undefined,
                  margin: row.conflict ? "0 -20px" : undefined,
                } as React.CSSProperties}
              >
                <span style={{ flex: 1 }}>
                  {row.label}
                  {row.conflict && (
                    <>
                      <br />
                      <span style={{ fontSize: 12, color: "#EBC277" }}>{row.conflict}</span>
                    </>
                  )}
                </span>
                {row.keys.map((k) => (
                  <kbd
                    key={k}
                    className="mono"
                    style={{
                      fontSize: 12, background: "#1F242E", borderRadius: 6,
                      padding: "3px 8px", boxShadow: "inset 0 -1px 0 #0E1014",
                    }}
                  >
                    {k}
                  </kbd>
                ))}
                <button
                  className="btn"
                  disabled
                  style={{ fontSize: 12, border: 0, background: "transparent", color: "#8DBAFF", cursor: "default", opacity: 0.5 }}
                >
                  변경
                </button>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
