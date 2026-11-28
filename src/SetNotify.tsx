import type { AppSettings } from "./api";

type Props = {
  settings: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
};

const ROWS: { label: string; pc: keyof AppSettings; phone?: null; sound?: null }[] = [
  { label: "에이전트가 입력이나 승인을 기다릴 때", pc: "notifyWaiting" },
  { label: "작업이 끝났을 때", pc: "notifyDone" },
  { label: "오류로 멈췄을 때", pc: "notifyError" },
  { label: "두 세션이 같은 파일을 고쳤을 때", pc: "notifyConflict" },
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

export default function SetNotify({ settings, onChange }: Props) {
  return (
    <div>
      <div style={{ display: "flex", alignItems: "flex-end", gap: 10, flexWrap: "wrap", marginBottom: 20 }}>
        <div style={{ flex: 1 }}>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>알림</h2>
          <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--muted)" }}>
            어떤 일이 생겼을 때 어디로 알려드릴지 정해요.
          </p>
        </div>
        <button className="btn">테스트 알림 보내기</button>
      </div>

      <section style={{ marginBottom: 22 }}>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>알림 받을 일</h3>
        <div className="settings-card" style={{ padding: "4px 10px", overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13.5 }}>
            <thead>
              <tr>
                <th style={{ padding: "12px 10px", textAlign: "left", fontSize: 12, color: "#A0A8B8", fontWeight: 500 }}>상황</th>
                <th style={{ padding: "12px 10px", textAlign: "center", fontSize: 12, color: "#A0A8B8", fontWeight: 500 }}>PC 알림</th>
                <th style={{ padding: "12px 10px", textAlign: "center", fontSize: 12, color: "#A0A8B8", fontWeight: 500 }}>폰 알림</th>
                <th style={{ padding: "12px 10px", textAlign: "center", fontSize: 12, color: "#A0A8B8", fontWeight: 500 }}>소리</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((row) => (
                <tr key={row.pc} style={{ borderTop: "1px solid #1F242D" }}>
                  <td style={{ padding: "12px 10px", fontWeight: 400 }}>{row.label}</td>
                  <td style={{ padding: "12px 10px", textAlign: "center" }}>
                    <input
                      type="checkbox"
                      checked={settings[row.pc] as boolean}
                      onChange={(e) => onChange({ [row.pc]: e.target.checked } as Partial<AppSettings>)}
                      style={{ accentColor: "#6EA8FF", width: 16, height: 16 }}
                      aria-label={`${row.label} PC`}
                    />
                  </td>
                  <td style={{ padding: "12px 10px", textAlign: "center" }}>
                    <input type="checkbox" disabled style={{ accentColor: "#6EA8FF", width: 16, height: 16, opacity: 0.4 }} aria-label={`${row.label} 폰`} />
                  </td>
                  <td style={{ padding: "12px 10px", textAlign: "center" }}>
                    <input type="checkbox" disabled style={{ accentColor: "#6EA8FF", width: 16, height: 16, opacity: 0.4 }} aria-label={`${row.label} 소리`} />
                  </td>
                </tr>
              ))}
              <tr style={{ borderTop: "1px solid #1F242D" }}>
                <td style={{ padding: "12px 10px" }}>
                  컨텍스트가{" "}
                  <select
                    className="settings-input"
                    style={{ padding: "3px 6px" }}
                    aria-label="컨텍스트 기준"
                    value={settings.notifyContextThreshold}
                    onChange={(e) => onChange({ notifyContextThreshold: Number(e.target.value) })}
                  >
                    <option value={70}>70%</option>
                    <option value={80}>80%</option>
                    <option value={90}>90%</option>
                  </select>{" "}
                  넘을 때
                </td>
                <td style={{ padding: "12px 10px", textAlign: "center" }}>
                  <input
                    type="checkbox"
                    checked={settings.notifyContext}
                    onChange={(e) => onChange({ notifyContext: e.target.checked })}
                    style={{ accentColor: "#6EA8FF", width: 16, height: 16 }}
                    aria-label="컨텍스트 PC"
                  />
                </td>
                <td style={{ padding: "12px 10px", textAlign: "center" }}>
                  <input type="checkbox" disabled style={{ accentColor: "#6EA8FF", width: 16, height: 16, opacity: 0.4 }} aria-label="컨텍스트 폰" />
                </td>
                <td style={{ padding: "12px 10px", textAlign: "center" }}>
                  <input type="checkbox" disabled style={{ accentColor: "#6EA8FF", width: 16, height: 16, opacity: 0.4 }} aria-label="컨텍스트 소리" />
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>폰 알림 규칙</h3>
        <div className="settings-card">
          <div className="settings-item" style={{ opacity: 0.5 }}>
            <span style={{ flex: 1, fontSize: 13, color: "#8A93A5" }}>
              폰 알림 설정은 Stage 8(모바일 연동) 단계에서 추가돼요.
            </span>
          </div>
        </div>
      </section>
    </div>
  );
}
