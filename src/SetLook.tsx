import type { AppSettings } from "./api";

type Props = {
  settings: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
};

const FONTS = ["JetBrains Mono", "D2Coding", "Cascadia Code", "Consolas"];
const CURSOR_OPTS: { value: string; label: string }[] = [
  { value: "block", label: "블록" },
  { value: "underline", label: "밑줄" },
  { value: "bar", label: "세로줄" },
];
const ACCENT_COLORS = [
  { value: "blue", hex: "#2F6FE4", label: "파랑" },
  { value: "purple", hex: "#7C5CE0", label: "보라" },
  { value: "green", hex: "#1E9E73", label: "초록" },
  { value: "orange", hex: "#C8563F", label: "주황" },
];

function Sw({ on, label, onChange }: { on: boolean; label: string; onChange: () => void }) {
  return (
    <button
      className={`settings-sw${on ? " on" : ""}`}
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={onChange}
    >
      <span />
    </button>
  );
}

export default function SetLook({ settings, onChange }: Props) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 22 }}>
      <div>
        <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>화면</h2>
        <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--muted)" }}>
          테마, 글꼴, 패널에 보일 정보를 정해요.
        </p>
      </div>

      {/* 테마 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>테마</h3>
        <div role="radiogroup" aria-label="테마" style={{ display: "grid", gridTemplateColumns: "repeat(3,minmax(0,1fr))", gap: 10 }}>
          {[
            { value: "dark", label: "다크" },
            { value: "light", label: "라이트" },
            { value: "system", label: "Windows 설정 따라가기" },
          ].map((t) => (
            <button
              key={t.value}
              role="radio"
              aria-checked={settings.theme === t.value}
              onClick={() => onChange({ theme: t.value })}
              style={{
                display: "flex", flexDirection: "column", gap: 8, padding: 10, borderRadius: 12,
                background: "#14171D", cursor: "pointer", fontSize: 13, border: 0,
                color: "#E8EAF0", fontFamily: "inherit", textAlign: "left",
                boxShadow: settings.theme === t.value ? "inset 0 0 0 2px #2F6FE4" : undefined,
              }}
            >
              <span style={{
                display: "flex", height: 72, borderRadius: 8, padding: 8, gap: 6,
                background: t.value === "light" ? "#F4F5F7" : "#0E1014",
              }}>
                {t.value !== "system" && <>
                  <span style={{ width: "30%", background: t.value === "light" ? "#FFFFFF" : "#14171D", borderRadius: 4 }} />
                  <span style={{ flex: 1, background: t.value === "light" ? "#E9EBEF" : "#0B0D10", borderRadius: 4 }} />
                </>}
              </span>
              {t.label}
            </button>
          ))}
        </div>
      </section>

      {/* 기본 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>기본</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>강조색</span>
            <div aria-label="강조색 (다음 버전)" style={{ display: "flex", gap: 8 }}>
              {ACCENT_COLORS.map((c) => (
                <button
                  key={c.value}
                  disabled
                  aria-label={c.label}
                  title="다음 버전에서 추가돼요"
                  style={{
                    width: 30, height: 30, borderRadius: "50%", border: 0, cursor: "not-allowed",
                    background: c.hex, opacity: 0.5,
                  }}
                />
              ))}
            </div>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>언어</span>
            <select
              className="settings-input"
              aria-label="언어"
              value={settings.language}
              onChange={(e) => onChange({ language: e.target.value })}
            >
              <option value="ko">한국어</option>
              <option value="en">English</option>
            </select>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              밀도
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>조밀하게 하면 패널 헤더와 목록 간격이 줄어요</span>
            </span>
            <div role="radiogroup" aria-label="밀도" style={{ display: "flex", gap: 2, background: "#0E1014", borderRadius: 8, padding: 3 }}>
              {[{ value: "normal", label: "기본" }, { value: "compact", label: "조밀" }].map((d) => (
                <button
                  key={d.value}
                  className="btn"
                  role="radio"
                  aria-checked={settings.density === d.value}
                  onClick={() => onChange({ density: d.value })}
                  style={{
                    padding: "5px 12px",
                    background: settings.density === d.value ? "#262C38" : "transparent",
                    color: settings.density === d.value ? "#E8EAF0" : "#A0A8B8",
                  }}
                >
                  {d.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </section>

      {/* 터미널 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>터미널</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>글꼴</span>
            <select
              className="settings-input"
              aria-label="글꼴"
              value={settings.terminalFont}
              onChange={(e) => onChange({ terminalFont: e.target.value })}
            >
              {FONTS.map((f) => <option key={f} value={f}>{f}</option>)}
            </select>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>크기 · 줄 간격</span>
            <input
              className="settings-input mono"
              style={{ width: 60 }}
              type="number"
              min={8} max={24}
              value={settings.terminalFontSize}
              onChange={(e) => onChange({ terminalFontSize: Number(e.target.value) })}
              aria-label="크기"
            />
            <span style={{ color: "#8A93A5" }}>px</span>
            <input
              className="settings-input mono"
              style={{ width: 60 }}
              type="number"
              min={1.0} max={3.0} step={0.1}
              value={settings.terminalLineHeight}
              onChange={(e) => onChange({ terminalLineHeight: Number(e.target.value) })}
              aria-label="줄 간격"
            />
          </div>
          <div className="settings-item" style={{ display: "block" }}>
            <div
              className="mono"
              style={{
                background: "#0B0D10", borderRadius: 9, padding: "12px 14px",
                fontSize: settings.terminalFontSize, lineHeight: settings.terminalLineHeight,
                color: "#D5D9E0", whiteSpace: "pre-wrap",
              }}
            >
              <span style={{ color: "#4CC79A" }}>⏺</span>{" "}Update(apply.jsp){"\n"}
              <span style={{ color: "#8A93A5" }}>  ⎿  Updated apply.jsp with 8 additions</span>{"\n"}
              ⏺ 미리보기: 한글과 English가 섞여도 줄이 맞아요.
            </div>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>커서 모양</span>
            <select
              className="settings-input"
              aria-label="커서"
              value={settings.cursorShape}
              onChange={(e) => onChange({ cursorShape: e.target.value })}
            >
              {CURSOR_OPTS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </select>
          </div>
        </div>
      </section>
    </div>
  );
}
