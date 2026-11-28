import type { CSSProperties, PointerEventHandler } from "react";
import { AGENTS, type Project, type SessionRecord } from "./api";
import ConflictBanner from "./ConflictBanner";
import { TrashIcon } from "./icons";
import {
  contextColor,
  contextPercent,
  exitCode,
  formatContext,
  hookStatusText,
  type RunState,
  type SessionHookState,
  sessionStatus,
  statusText,
} from "./model";
import { TerminalView } from "./TerminalView";

type Props = {
  project: Project;
  session: SessionRecord;
  shell: string;
  run?: RunState;
  hook?: SessionHookState;
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
  onApprove: (choice: 1 | 2 | 3) => void;
  conflictFiles?: string[];
  onOpenChanges?: () => void;
};

export function SessionPanel({ project, session, run, hook, ...props }: Props) {
  const status = sessionStatus(run);
  const live = hook?.live;
  const ctx = hook?.context;
  const pct = ctx ? contextPercent(ctx) : undefined;
  const color = pct !== undefined ? contextColor(pct) : undefined;
  const isWaiting = live?.type === "waiting";
  const agent =
    session.agent === "custom"
      ? session.customCommand
      : AGENTS.find((a) => a.id === session.agent)?.name;
  return (
    <section
      className={`panel${isWaiting ? " panel-waiting" : ""}`}
      aria-label={session.name}
    >
      <div className="ph" onPointerDown={props.onDragStart}>
        <div className="ph-text">
          <div className="ph-title">
            <span className={`dot ${live ? live.type : status}`} />
            <span className="name">{session.name}</span>
            <span className={`state ${live ? live.type : status}`}>
              {hookStatusText(live) ?? statusText(run)}
            </span>
            {ctx !== undefined && pct !== undefined && (
              <span className="ctx-ring" title={`${formatContext(ctx)}${ctx.estimated ? " (추정)" : ""}`}>
                <svg viewBox="0 0 32 32" className="ctx-ring-svg">
                  <circle className="ctx-ring-bg" cx="16" cy="16" r="13" />
                  <circle
                    className={`ctx-ring-fill ctx-${color}`}
                    cx="16" cy="16" r="13"
                    style={{ "--pct": `${pct}` } as CSSProperties}
                  />
                </svg>
                <span className={`ctx-ring-num ctx-${color}`}>
                  {formatContext(ctx)}{ctx.estimated && <span className="ctx-est"> 추정</span>}
                </span>
              </span>
            )}
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
      <ConflictBanner files={props.conflictFiles ?? []} onOpenChanges={props.onOpenChanges ?? (() => {})} />
      {isWaiting && (
        <div className="approve-row" role="group" aria-label="승인 선택">
          <button className="btn approve" onClick={() => props.onApprove(1)}>1 · 예</button>
          <button className="btn approve" onClick={() => props.onApprove(2)}>2 · 세션 동안 허용</button>
          <button className="btn approve ghost" onClick={() => props.onApprove(3)}>3 · 아니요</button>
        </div>
      )}
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
