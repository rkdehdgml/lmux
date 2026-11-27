import { useState } from "react";
import type { SavedLayout } from "./api";
import { Dialog } from "./Dialog";
import { LayoutIcon, SwapIcon } from "./icons";
import { type Node, type Preset, presetOf } from "./layout";

const PRESETS: { n: Preset; button: string; text: string }[] = [
  { n: 1, button: "1개", text: "1개 · 집중 모드" },
  { n: 2, button: "2개", text: "2개" },
  { n: 3, button: "3개", text: "1 + 2" },
  { n: 4, button: "4개", text: "2 × 2" },
];

type Props = {
  tree: Node;
  maximized: boolean;
  saved: SavedLayout[];
  onPreset: (n: Preset) => void;
  onTranspose: () => void;
  onOpenSaved: () => void;
};

export function LayoutBar({ tree, maximized, saved, ...props }: Props) {
  const current = presetOf(tree);
  const now = JSON.stringify(tree);
  const match = saved.find((s) => JSON.stringify(s.tree) === now);
  const side = current === 3 && tree.type === "split" ? (tree.dir === "row" ? "왼쪽" : "위") : "";
  const text = PRESETS.find((p) => p.n === current)?.text ?? "직접 나눔";
  return (
    <div className="layout-bar">
      <div
        className="layout-presets"
        role="group"
        aria-label="분할 레이아웃"
        title="터미널에서 Ctrl+F6을 누르면 여기로 와요"
      >
        {PRESETS.map((p) => (
          <button
            key={p.n}
            className={`btn ${current === p.n ? "pressed" : "ghost"}`}
            aria-label={p.button}
            aria-pressed={current === p.n}
            onClick={() => props.onPreset(p.n)}
          >
            <LayoutIcon panes={p.n} />
          </button>
        ))}
      </div>
      <button className="btn ghost" onClick={props.onTranspose} disabled={tree.type === "pane"}>
        <SwapIcon />
        좌우 ↔ 상하
      </button>
      <span className="dim layout-name">
        {maximized ? "최대화" : side ? `${text} · ${side} 크게` : text}
      </span>
      <span className="grow" />
      <span className="dim layout-name">저장된 화면</span>
      <button className="btn" onClick={props.onOpenSaved}>
        {match?.name ?? "저장 · 불러오기"} ▾
      </button>
    </div>
  );
}

type DialogProps = {
  saved: SavedLayout[];
  onSave: (name: string) => Promise<void>;
  onLoad: (layout: SavedLayout) => void;
  onDelete: (id: string) => Promise<void>;
  onClose: () => void;
};

export function LayoutDialog({ saved, ...props }: DialogProps) {
  const [name, setName] = useState("");
  // 지우기 전에 한 번 더 누르게 한다.
  const [confirming, setConfirming] = useState<string>();
  const [error, setError] = useState<string>();
  return (
    <Dialog
      title="저장된 화면"
      description="지금 화면 배치와 패널에 연 세션을 이름으로 저장해요. 같은 이름은 덮어써요."
      submit="지금 화면 저장"
      onClose={props.onClose}
      onSubmit={() => props.onSave(name)}
    >
      <div className="field">
        <label htmlFor="layout-name">이름</label>
        <input
          id="layout-name"
          className="input"
          data-autofocus
          maxLength={40}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      {saved.length > 0 && (
        <ul className="layout-list">
          {saved.map((s) => (
            <li key={s.id}>
              <span className="grow">{s.name}</span>
              <button
                type="button"
                className="btn"
                aria-label={`${s.name} 불러오기`}
                onClick={() => props.onLoad(s)}
              >
                불러오기
              </button>
              <button
                type="button"
                className={`btn ${confirming === s.id ? "danger" : "ghost"}`}
                aria-label={`${s.name} ${confirming === s.id ? "삭제 확인" : "삭제"}`}
                onClick={() => {
                  if (confirming !== s.id) return setConfirming(s.id);
                  setError(undefined);
                  props.onDelete(s.id).catch((e) => setError(String(e)));
                }}
              >
                {confirming === s.id ? "삭제 확인" : "삭제"}
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && (
        <div className="dialog-error" role="alert">
          {error}
        </div>
      )}
    </Dialog>
  );
}
