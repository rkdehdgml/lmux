import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { AGENTS, type Agent, api, type Created, type Kind, type Shell } from "./api";
import { Dialog } from "./Dialog";
import { KIND_NAMES, KindBadge } from "./KindBadge";

type Props = { shells: Shell[]; onClose: () => void; onAdded: (created: Created) => void };

export function NewProject({ shells, onClose, onAdded }: Props) {
  const [path, setPath] = useState("");
  const [name, setName] = useState<string>();
  const [shell, setShell] = useState("powershell");
  const [agent, setAgent] = useState<Agent>("claude");
  const [found, setFound] = useState<{ kind: Kind; name: string }>();

  // 입력하는 동안에는 늦게 온 이전 경로의 결과를 버린다.
  useEffect(() => {
    let stale = false;
    setFound(undefined);
    const timer = setTimeout(() => {
      if (!path.trim()) return;
      api.detect(path).then(
        (result) => !stale && setFound(result),
        () => {},
      );
    }, 250);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [path]);

  const browse = async () => {
    const folder = await open({ directory: true, title: "프로젝트 폴더" });
    if (folder) setPath(folder);
  };

  return (
    <Dialog
      title="프로젝트 추가"
      description="작업할 폴더만 고르면 돼요. Git이 아니어도 괜찮아요."
      submit="추가하고 세션 열기"
      onClose={onClose}
      onSubmit={async () => {
        onAdded(await api.addProject({ name: name ?? "", path, shell, agent }));
      }}
    >
      <div className="field">
        <label htmlFor="np-path">프로젝트 폴더</label>
        <div className="row">
          <input
            id="np-path"
            className="input mono grow"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="D:\work\project"
            required
            data-autofocus
          />
          <button type="button" className="btn" onClick={browse}>
            찾아보기
          </button>
        </div>
        {found && (
          <div className="detected" role="status">
            <KindBadge kind={found.kind} />
            <span className="grow">{KIND_NAMES[found.kind].description}</span>
            <span className="ok">감지됨</span>
          </div>
        )}
        <span className="hint muted">Git · SVN · 일반 폴더를 자동으로 알아봐요.</span>
      </div>
      <div className="field">
        <label htmlFor="np-name">표시 이름</label>
        <input
          id="np-name"
          className="input"
          value={name ?? found?.name ?? ""}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="cols">
        <div className="field">
          <label htmlFor="np-shell">기본 셸</label>
          <select id="np-shell" className="input" value={shell} onChange={(e) => setShell(e.target.value)}>
            {shells.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </div>
        <div className="field">
          <label htmlFor="np-agent">기본 에이전트</label>
          <select
            id="np-agent"
            className="input"
            value={agent}
            onChange={(e) => setAgent(e.target.value as Agent)}
          >
            {AGENTS.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </div>
      </div>
      <p className="info muted">한 폴더에서 세션을 여러 개 열 수 있어요.</p>
    </Dialog>
  );
}
