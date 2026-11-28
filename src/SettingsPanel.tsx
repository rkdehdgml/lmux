import { useCallback, useEffect, useRef, useState } from "react";
import { api, type AppSettings, DEFAULT_SETTINGS } from "./api";
import SetShells from "./SetShells";
import SetNotify from "./SetNotify";
import SetLook from "./SetLook";
import SetPerf from "./SetPerf";
import SetKeys from "./SetKeys";
import SetAbout from "./SetAbout";

type Props = {
  onClose: () => void;
  shells?: { id: string; name: string }[];
};

type Tab = "shells" | "notify" | "look" | "perf" | "keys" | "about";

const NAV_ITEMS: { id: Tab; label: string }[] = [
  { id: "shells", label: "셸 프로필" },
  { id: "notify", label: "알림" },
  { id: "look", label: "화면" },
  { id: "perf", label: "성능" },
  { id: "keys", label: "단축키" },
  { id: "about", label: "정보 · 업데이트" },
];

export default function SettingsPanel({ onClose, shells = [] }: Props) {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [activeTab, setActiveTab] = useState<Tab>("shells");
  const [saving, setSaving] = useState(false);
  const loaded = useRef(false);

  useEffect(() => {
    if (loaded.current) return;
    loaded.current = true;
    api.getSettings().then(setSettings).catch(() => {});
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.preventDefault(); onClose(); }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const patch = useCallback((update: Partial<AppSettings>) => {
    setSettings((s) => ({ ...s, ...update }));
  }, []);

  const handleSave = async () => {
    setSaving(true);
    try {
      await api.saveSettings(settings);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="settings-overlay" role="dialog" aria-label="설정">
      <div className="settings-panel">
        {/* Left nav */}
        <nav className="settings-nav" aria-label="설정 항목">
          <div style={{ fontSize: 14, fontWeight: 700, padding: "4px 12px 14px", color: "var(--text)" }}>
            설정
          </div>
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              className={`settings-nav-item${activeTab === item.id ? " active" : ""}`}
              onClick={() => setActiveTab(item.id)}
            >
              {item.label}
            </button>
          ))}
          <div style={{ flex: 1 }} />
          <button
            className="btn primary"
            style={{ marginTop: 12 }}
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "저장 중…" : "저장"}
          </button>
        </nav>

        {/* Right content */}
        <div className="settings-content">
          <button
            className="btn"
            style={{
              position: "absolute",
              top: 12,
              right: 12,
              background: "transparent",
              padding: "6px 8px",
              fontSize: 18,
              lineHeight: 1,
              color: "var(--muted)",
            }}
            onClick={onClose}
            aria-label="닫기"
          >
            ✕
          </button>

          {activeTab === "shells" && (
            <SetShells shells={shells} />
          )}
          {activeTab === "notify" && (
            <SetNotify settings={settings} onChange={patch} />
          )}
          {activeTab === "look" && (
            <SetLook settings={settings} onChange={patch} />
          )}
          {activeTab === "perf" && (
            <SetPerf settings={settings} onChange={patch} />
          )}
          {activeTab === "keys" && <SetKeys />}
          {activeTab === "about" && <SetAbout />}
        </div>
      </div>
    </div>
  );
}
