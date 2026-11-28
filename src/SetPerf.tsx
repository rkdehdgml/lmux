import type { AppSettings } from "./api";

type Props = {
  settings: AppSettings;
  onChange: (patch: Partial<AppSettings>) => void;
  sessionCount?: number;
  runningCount?: number;
};

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

export default function SetPerf({ settings, onChange, sessionCount = 0, runningCount = 0 }: Props) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 22 }}>
      <div>
        <h2 style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>성능</h2>
        <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--muted)" }}>
          세션이 많아도 가볍게 돌아가도록 조절해요.
        </p>
      </div>

      {/* 앱 상태 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>지금 앱 상태</h3>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(4,minmax(0,1fr))", gap: 10 }}>
          {[
            { label: "앱 메모리", value: "—", sub: "에이전트 프로세스 제외" },
            { label: "CPU", value: "—", sub: "최근 1분 평균" },
            { label: "세션", value: String(sessionCount), sub: `실행 중 ${runningCount}` },
            { label: "렌더링", value: settings.webgl ? "WebGL" : "Canvas", sub: settings.webgl ? "GPU 가속 켜짐" : "소프트웨어 렌더링" },
          ].map((s) => (
            <div
              key={s.label}
              style={{ background: "#14171D", borderRadius: 12, padding: "14px 16px", display: "flex", flexDirection: "column", gap: 4 }}
            >
              <span style={{ fontSize: 12, color: "#8A93A5" }}>{s.label}</span>
              <span
                className="mono"
                style={{ fontSize: 20, fontWeight: 600, color: s.label === "렌더링" ? "#4CC79A" : undefined }}
              >
                {s.value}
              </span>
              <span style={{ fontSize: 11.5, color: "#8A93A5" }}>{s.sub}</span>
            </div>
          ))}
        </div>
      </section>

      {/* 렌더링 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>렌더링</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              숨긴 세션은 그리지 않기
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>세션은 계속 돌고, 출력만 쌓아 두다가 보일 때 한 번에 그려요</span>
            </span>
            <Sw on={settings.hideInactive} label="숨긴 세션 그리지 않기" onChange={() => onChange({ hideInactive: !settings.hideInactive })} />
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              GPU 가속 (WebGL)
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>끄면 호환성은 좋아지지만 출력이 많을 때 느려질 수 있어요</span>
            </span>
            <Sw on={settings.webgl} label="GPU 가속" onChange={() => onChange({ webgl: !settings.webgl })} />
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>세션당 스크롤백</span>
            <select
              className="settings-input"
              aria-label="스크롤백"
              value={settings.scrollbackLines}
              onChange={(e) => onChange({ scrollbackLines: Number(e.target.value) })}
            >
              <option value={5000}>5,000줄</option>
              <option value={10000}>10,000줄</option>
              <option value={50000}>50,000줄</option>
            </select>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>창이 내려가 있을 때 화면 갱신 멈추기</span>
            <Sw on={true} label="최소화 시 갱신 멈춤" onChange={() => {}} />
          </div>
        </div>
      </section>

      {/* 갱신 주기 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>갱신 주기</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>계정 사용량</span>
            <select className="settings-input" aria-label="사용량 주기" defaultValue="30">
              <option value="30">30초</option>
              <option value="60">1분</option>
              <option value="300">5분</option>
            </select>
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              세션 상태 · 컨텍스트
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>
                Claude Code 훅 이벤트로 바로 반영돼요. 훅이 없는 에이전트만 주기적으로 확인해요
              </span>
            </span>
            <select className="settings-input" aria-label="상태 주기" disabled>
              <option>이벤트 즉시</option>
            </select>
          </div>
        </div>
      </section>

      {/* 실행 */}
      <section>
        <h3 style={{ margin: "0 0 8px", fontSize: 14, fontWeight: 600, color: "#C7CCD6" }}>실행</h3>
        <div className="settings-card">
          <div className="settings-item">
            <span style={{ flex: 1 }}>에이전트가 일하는 동안 절전 모드 막기</span>
            <Sw on={settings.preventSleep} label="절전 막기" onChange={() => onChange({ preventSleep: !settings.preventSleep })} />
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>창을 닫으면 트레이로 (세션 유지)</span>
            <Sw on={settings.minimizeToTray} label="트레이" onChange={() => onChange({ minimizeToTray: !settings.minimizeToTray })} />
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>
              Windows 시작할 때 실행
              <br />
              <span style={{ fontSize: 12, color: "#8A93A5" }}>다음 버전에서 추가돼요</span>
            </span>
            <Sw on={false} label="시작 시 실행" onChange={() => {}} />
          </div>
          <div className="settings-item">
            <span style={{ flex: 1 }}>다시 열면 이전 세션과 화면 배치 복원</span>
            <Sw on={settings.restoreSessions} label="세션 복원" onChange={() => onChange({ restoreSessions: !settings.restoreSessions })} />
          </div>
        </div>
      </section>
    </div>
  );
}
