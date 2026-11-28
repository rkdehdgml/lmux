import type { CSSProperties, PointerEvent } from "react";
import type { Project, SessionRecord, Shell, StoredState } from "./api";
import { CloseIcon, FolderIcon, PlusIcon } from "./icons";
import { KindBadge } from "./KindBadge";
import {
  contextColor,
  contextPercent,
  groupByProject,
  hookStatusText,
  type RunState,
  type SessionHookState,
  sessionStatus,
  statusText,
} from "./model";

type Props = {
  state: StoredState;
  runs: Record<string, RunState>;
  hooks: Record<string, SessionHookState>;
  shells: Shell[];
  /** 초점 패널의 세션. */
  selected?: string;
  /** 지금 패널에 보이는 세션. */
  visible: string[];
  /** 끌고 있는 세션. */
  dragging?: string;
  /** 두 칸 배치에서 세션이 보이는 쪽. */
  places: Record<string, string>;
  /** 세션을 끌어 패널에 놓는다. */
  onDragStart: (session: string) => (event: PointerEvent) => void;
  onSelect: (session: SessionRecord) => void;
  onAddProject: () => void;
  onNewSession: (projectId?: string) => void;
  onRemoveProject: (project: Project) => void;
};

export function shellName(shells: Shell[], id: string) {
  return shells.find((s) => s.id === id)?.name ?? id;
}

export function Sidebar(props: Props) {
  const { state, runs, hooks, shells, selected, visible } = props;
  return (
    <aside className="sidebar" aria-label="프로젝트와 세션">
      <div className="side-head">
        <span>프로젝트</span>
        <button className="btn ghost icon-btn" aria-label="프로젝트 추가" onClick={props.onAddProject}>
          <PlusIcon />
        </button>
      </div>
      <div className="side-list">
        {groupByProject(state).map(({ project, sessions }) => (
          <div className="project" key={project.id}>
            <div className="project-head" title={project.path}>
              <FolderIcon />
              <span className="project-name">{project.name}</span>
              <button
                className="btn ghost icon-btn quiet"
                aria-label={`${project.name} 새 세션`}
                onClick={() => props.onNewSession(project.id)}
              >
                <PlusIcon />
              </button>
              <button
                className="btn ghost icon-btn quiet"
                aria-label={`${project.name} 등록 해제`}
                onClick={() => props.onRemoveProject(project)}
              >
                <CloseIcon />
              </button>
              <KindBadge kind={project.kind} />
            </div>
            {sessions.map((session) => {
              const status = sessionStatus(runs[session.id]);
              const hook = hooks[session.id];
              const live = hook?.live;
              const ctx = hook?.context;
              const pct = ctx ? contextPercent(ctx) : undefined;
              const dotClass = live ? live.type : status;
              const shown = visible.includes(session.id);
              const subText = hookStatusText(live) ?? statusText(runs[session.id]);
              return (
                <button
                  key={session.id}
                  className={`sess ${shown ? "on" : ""} ${props.dragging === session.id ? "dragged" : ""}`}
                  aria-current={session.id === selected ? "true" : undefined}
                  onClick={() => props.onSelect(session)}
                  onPointerDown={props.onDragStart(session.id)}
                >
                  <span className={`dot ${dotClass}`} />
                  <span className="sess-text">
                    <span className="sess-name">{session.name}</span>
                    <span className="sess-sub muted">
                      {props.dragging === session.id ? (
                        "끌어서 패널에 놓기"
                      ) : (
                        <>
                          {subText} ·{" "}
                          {props.places[session.id] ??
                            (status === "running" && !shown ? "숨김" : shellName(shells, session.shell))}
                        </>
                      )}
                    </span>
                    {ctx !== undefined && pct !== undefined && (
                      <span className="sess-ctx">
                        <span
                          className={`ctx-bar ctx-${contextColor(pct)}`}
                          style={{ "--pct": `${pct}%` } as CSSProperties}
                        />
                        <span className={`ctx-pct ctx-${contextColor(pct)}`}>
                          {pct}%{ctx.estimated && " 추정"}
                        </span>
                      </span>
                    )}
                  </span>
                </button>
              );
            })}
          </div>
        ))}
      </div>
      {state.sessions.some((s) => !visible.includes(s.id) && sessionStatus(runs[s.id]) === "running") && (
        <p className="side-note dim">숨긴 세션도 계속 돌아가요. 화면만 그리지 않아요.</p>
      )}
      <button
        className="btn primary wide"
        onClick={() => props.onNewSession()}
        disabled={state.projects.length === 0}
      >
        <PlusIcon />새 세션
      </button>
    </aside>
  );
}
