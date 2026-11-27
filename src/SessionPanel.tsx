import type { PointerEventHandler } from "react";
import { AGENTS, type Project, type SessionRecord } from "./api";
import { TrashIcon } from "./icons";
import { exitCode, type RunState, sessionStatus, statusText } from "./model";
import { TerminalView } from "./TerminalView";

type Props = {
  project: Project;
  session: SessionRecord;
  shell: string;
  run?: RunState;
  /** 창이 최소화되면 터미널을 만들지 않는다. */
  hidden: boolean;
  busy: boolean;
  focused: boolean;
  /** 머리글을 끌어 다른 패널에 놓는다. */
  onDragStart: PointerEventHandler;
  onStart: () => void;
  onExited: (id: string, run: number, code: number) => void;
  onRestart: () => void;
  onDelete: () => void;
};

export function SessionPanel({ project, session, run, ...props }: Props) {
  const status = sessionStatus(run);
  const agent =
    session.agent === "custom"
      ? session.customCommand
      : AGENTS.find((a) => a.id === session.agent)?.name;
  return (
    <section className="panel" aria-label={session.name}>
      <div className="ph" onPointerDown={props.onDragStart}>
        <div className="ph-text">
          <div className="ph-title">
            <span className={`dot ${status}`} />
            <span className="name">{session.name}</span>
            <span className={`state ${status}`}>{statusText(run)}</span>
          </div>
          <div className="ph-sub dim" title={project.path}>
            {project.name} · <span className="mono">{project.path}</span> · {props.shell} · {agent}
          </div>
        </div>
        {(status === "failed" || exitCode(run) !== undefined) && (
          <button className="btn primary" onClick={props.onRestart} disabled={props.busy}>
            다시 시작
          </button>
        )}
        <button className="btn ghost icon-btn" aria-label="세션 삭제" onClick={props.onDelete}>
          <TrashIcon />
        </button>
      </div>
      {run?.error ? (
        <div className="term-error" role="alert">
          셸을 시작하지 못했어요: {run.error}
        </div>
      ) : run?.started && !props.hidden ? (
        <TerminalView
          key={`${session.id}:${run.started.run}`}
          sessionId={session.id}
          run={run.started.run}
          osBuild={run.started.osBuild}
          focused={props.focused}
          onExited={(r, code) => props.onExited(session.id, r, code)}
        />
      ) : (
        <div className="term-error dim">
          {run?.started ? (
            ""
          ) : props.busy ? (
            "시작하는 중"
          ) : (
            <button className="btn primary" onClick={props.onStart}>
              시작
            </button>
          )}
        </div>
      )}
    </section>
  );
}
