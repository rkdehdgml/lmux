import { useState } from "react";
import { AGENTS, type Agent, api, type Created, type Shell, type StoredState } from "./api";
import { Dialog } from "./Dialog";
import { KIND_NAMES } from "./KindBadge";
import { type RunState, runningIn } from "./model";

type Props = {
  state: StoredState;
  runs: Record<string, RunState>;
  shells: Shell[];
  shellsComplete: boolean;
  projectId?: string;
  onClose: () => void;
  onCreated: (created: Created) => void;
};

export function NewSession({ state, runs, shells, onClose, onCreated, ...props }: Props) {
  const find = (id?: string) => state.projects.find((p) => p.id === id) ?? state.projects[0];
  const initial = find(props.projectId);
  const [projectId, setProjectId] = useState(initial.id);
  const [name, setName] = useState("");
  const [prompt, setPrompt] = useState("");
  const [shell, setShell] = useState(initial.defaultShell);
  const [agent, setAgent] = useState<Agent>(initial.defaultAgent);
  const [command, setCommand] = useState("");
  const project = find(projectId);
  const takesPrompt = agent === "claude" || agent === "codex";
  const others = runningIn(state, runs, projectId);
  // WSL 확인이 실패할 수 있으므로 목록에 없어도 저장한 기본 셸을 다른 셸로 바꾸지 않는다.
  const missing = props.shellsComplete && !shells.some((s) => s.id === shell);

  return (
    <Dialog
      title="새 세션"
      description="프로젝트 폴더에서 에이전트를 바로 띄워요"
      submit="세션 열기"
      onClose={onClose}
      onSubmit={async () => {
        onCreated(
          await api.createSession({
            projectId,
            name,
            shell,
            agent,
            customCommand: command,
            prompt: takesPrompt ? prompt : "",
          }),
        );
      }}
    >
      <div className="field">
        <label htmlFor="ns-project">프로젝트</label>
        <select
          id="ns-project"
          className="input"
          value={projectId}
          onChange={(e) => {
            const next = find(e.target.value);
            setProjectId(next.id);
            setShell(next.defaultShell);
            setAgent(next.defaultAgent);
          }}
        >
          {state.projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name} · {KIND_NAMES[p.kind].badge} · {p.path}
            </option>
          ))}
        </select>
        <span className="hint dim">
          프로젝트 폴더에서 바로 작업해요 · <span className="mono">{project.path}</span>
        </span>
      </div>
      <div className="field">
        <label htmlFor="ns-name">{takesPrompt ? "세션 이름 · 첫 지시" : "세션 이름"}</label>
        <input
          id="ns-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="비워 두면 번호로 이름을 붙여요"
          data-autofocus
        />
        {takesPrompt && (
          <>
            <textarea
              className="input"
              rows={2}
              aria-label="첫 지시"
              maxLength={4000}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
            />
            <span className="hint dim">비워 두면 빈 세션으로 열려요. 줄바꿈은 공백으로 전달해요</span>
          </>
        )}
      </div>
      <fieldset className="field">
        <legend>셸</legend>
        <div className="segments">
          {shells.map((s) => (
            <label key={s.id} className={`segment ${s.id === shell ? "on" : ""}`}>
              <input type="radio" name="shell" checked={s.id === shell} onChange={() => setShell(s.id)} />
              {s.name}
            </label>
          ))}
        </div>
        {!props.shellsComplete && <span className="hint dim">WSL 배포판을 확인하는 중이에요</span>}
        {missing && (
          <span className="hint dim">
            저장한 셸({shell})을 지금은 찾지 못했어요. 다른 셸을 고르거나 그대로 열어 보세요
          </span>
        )}
      </fieldset>
      <div className="field">
        <label htmlFor="ns-agent">에이전트</label>
        <select
          id="ns-agent"
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
        {agent === "custom" && (
          <input
            className="input mono"
            aria-label="실행할 명령"
            placeholder="셸에서 실행할 명령"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            required
          />
        )}
      </div>
      {others.length > 0 && (
        <p className="warning" role="status">
          이 폴더에서 이미 "{others[0].name}" 세션이 실행 중이에요
          {others.length > 1 && ` (외 ${others.length - 1}개)`}. 같은 파일을 함께 고치지 않게 주의하세요.
        </p>
      )}
    </Dialog>
  );
}
