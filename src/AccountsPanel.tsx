import type { AccountSummary } from "./api";

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) {
    const k = Math.round(n / 1_000);
    return k >= 1_000 ? (n / 1_000_000).toFixed(1) + "M" : k + "K";
  }
  return String(n);
}

type Props = {
  usages: AccountSummary[];
  tickSecs: number;
  onClose: () => void;
  onRefresh: () => void;
};

export default function AccountsPanel({ usages, tickSecs, onClose, onRefresh }: Props) {
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, "0");
  const mm = String(now.getMinutes()).padStart(2, "0");
  const ss = String(now.getSeconds()).padStart(2, "0");

  return (
    <div className="accounts-overlay" role="dialog" aria-modal="true" aria-label="계정 · 사용량">
      <div className="accounts-panel">
        <div className="accounts-header">
          <div className="accounts-title-block">
            <h1 className="accounts-title">계정 · 사용량</h1>
            <p className="accounts-subtitle muted">
              각 CLI가 남긴 로컬 로그로 계산해요. 계정은 설정 폴더 단위로 등록돼요.
            </p>
          </div>
          <span className="accounts-tick mono muted">
            마지막 갱신 {hh}:{mm}:{ss} · {tickSecs}초 후 갱신
          </span>
          <button className="btn" onClick={onRefresh} aria-label="지금 갱신">
            <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M20 11a8 8 0 1 0-2.3 5.7" />
              <path d="M20 4v7h-7" />
            </svg>
            지금 갱신
          </button>
          <button className="btn accounts-close" onClick={onClose} aria-label="닫기">
            <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="accounts-grid">
          {usages.map((acc) => {
            const total5h =
              acc.usage.fiveHour.input +
              acc.usage.fiveHour.cacheCreation +
              acc.usage.fiveHour.cacheRead;
            const pct5h =
              acc.limits.fiveHour > 0
                ? Math.min(100, Math.round((total5h / acc.limits.fiveHour) * 100))
                : 0;

            const totalWeekly =
              acc.usage.sevenDay.input +
              acc.usage.sevenDay.cacheCreation +
              acc.usage.sevenDay.cacheRead;
            const pctWeekly =
              acc.limits.weekly > 0
                ? Math.min(100, Math.round((totalWeekly / acc.limits.weekly) * 100))
                : 0;

            const avatarChar = acc.name.charAt(0);

            return (
              <section className="acc-card" key={acc.name}>
                <div className="acc-card-top">
                  <div className="acc-avatar">{avatarChar}</div>
                  <div className="acc-name-block">
                    <h2 className="acc-name">{acc.name}</h2>
                    <div className="acc-sub muted">
                      Claude Code{acc.limits.estimated ? " · 추정 한도" : ""}
                    </div>
                  </div>
                </div>

                {acc.paths.length > 0 && (
                  <div className="acc-paths mono muted">{acc.paths.join("\n")}</div>
                )}

                <div className="acc-bar-row">
                  <div className="row">
                    <span>5시간 한도</span>
                    <span className="mono">{pct5h}% · {formatTokens(total5h)} / {formatTokens(acc.limits.fiveHour)}</span>
                  </div>
                  <div className="bar">
                    <span style={{ width: `${pct5h}%`, background: "#5B9DFF" }} />
                  </div>
                </div>

                <div className="acc-bar-row">
                  <div className="row">
                    <span>주간 한도</span>
                    <span className="mono">{pctWeekly}% · {formatTokens(totalWeekly)} / {formatTokens(acc.limits.weekly)}</span>
                  </div>
                  <div className="bar">
                    <span style={{ width: `${pctWeekly}%`, background: "#5B9DFF" }} />
                  </div>
                </div>

                <div className="acc-stats">
                  <div className="acc-stat">
                    <div className="muted">오늘 입력</div>
                    <div className="mono acc-stat-val">{formatTokens(acc.usage.today.input)}</div>
                  </div>
                  <div className="acc-stat">
                    <div className="muted">오늘 출력</div>
                    <div className="mono acc-stat-val">{formatTokens(acc.usage.today.output)}</div>
                  </div>
                  <div className="acc-stat">
                    <div className="muted">캐시 읽기</div>
                    <div className="mono acc-stat-val">
                      {formatTokens(acc.usage.today.cacheRead)}
                    </div>
                  </div>
                </div>
              </section>
            );
          })}

          {usages.length === 0 && (
            <p className="accounts-empty muted">등록된 계정이 없어요.</p>
          )}
        </div>

        <div className="accounts-footer muted">
          <span>갱신 주기</span>
          <select className="btn accounts-interval" aria-label="갱신 주기" defaultValue="30">
            <option value="30">30초</option>
            <option value="60">1분</option>
            <option value="300">5분</option>
          </select>
          <span>· 한도 80%를 넘으면 알림</span>
        </div>
      </div>
    </div>
  );
}
