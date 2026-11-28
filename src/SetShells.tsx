type Shell = { id: string; name: string };

type Props = {
  shells: Shell[];
};

export default function SetShells({ shells }: Props) {
  return (
    <div>
      <h2 style={{ margin: "0 0 20px", fontSize: 18, fontWeight: 700 }}>셸 프로필</h2>
      <div className="settings-card">
        {shells.length === 0 ? (
          <div className="settings-item" style={{ color: "var(--dim)", justifyContent: "center" }}>
            감지된 셸이 없어요.
          </div>
        ) : (
          shells.map((shell) => (
            <div key={shell.id} className="settings-item">
              <span
                style={{
                  width: 10,
                  height: 10,
                  borderRadius: "50%",
                  background: "var(--done)",
                  flexShrink: 0,
                }}
              />
              <span style={{ flex: 1 }}>{shell.name}</span>
              <span
                className="mono"
                style={{ fontSize: 12, color: "var(--muted)" }}
              >
                {shell.id}
              </span>
            </div>
          ))
        )}
      </div>
      <p style={{ marginTop: 16, fontSize: 13, color: "var(--dim)", lineHeight: 1.6 }}>
        시스템에서 감지된 셸이에요. 커스텀 셸 프로필은 다음 버전에서 추가할 예정이에요.
      </p>
    </div>
  );
}
