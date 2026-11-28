import { listen } from "@tauri-apps/api/event";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  type AccountSummary,
  type Project,
  type SessionRecord,
  type SessionStatePayload,
  type Shell,
  type StoredState,
} from "./api";
import AccountsPanel from "./AccountsPanel";
import ChangesPanel from "./ChangesPanel";
import { Dialog } from "./Dialog";
import { Logo } from "./icons";
import { layoutShortcut } from "./keys";
import { LayoutBar, LayoutDialog } from "./LayoutBar";
import {
  drop,
  leaves,
  MAX_PANES,
  roomFor,
  type Node,
  preset,
  prune,
  resize,
  show,
  split,
  transpose,
  type View,
} from "./layout";
import { type RunState, recordExit, sessionStatus, type SessionHookState } from "./model";
import { NewProject } from "./NewProject";
import { NewSession } from "./NewSession";
import { PaneGrid } from "./PaneGrid";
import { SessionPanel } from "./SessionPanel";
import { Sidebar, shellName } from "./Sidebar";
import { latestOnly } from "./serial";
import { useMinimized } from "./useMinimized";
import { useSessionDrag } from "./useSessionDrag";

type Exit = { id: string; run: number; code: number };
type Modal =
  | { type: "project" }
  | { type: "session"; projectId?: string }
  | { type: "delete-session"; session: SessionRecord }
  | { type: "remove-project"; project: Project }
  | { type: "layouts" }
  | { type: "accounts" }
  | { type: "changes"; project: Project };

/** 패널을 나눠도 두 쪽이 쓸 만한 크기인지. 아니면 안내를 띄운다. */
function fits(index: number, dir: "row" | "col"): boolean {
  const box = document.querySelector(`[data-pane="${index}"]`)?.getBoundingClientRect();
  return !box || roomFor(dir, box.width, box.height);
}
const TOO_SMALL = "패널이 너무 작아 더 나눌 수 없어요";

const EMPTY: View = { tree: { type: "pane", session: null }, focus: 0 };
const saveTree = latestOnly(api.setLayout);

export default function App() {
  const [state, setState] = useState<StoredState>();
  // WSL 확인이 끝나기 전에는 나머지 셸만 담고 `complete`가 false다.
  const [{ shells, complete }, setShells] = useState<{ shells: Shell[]; complete: boolean }>({
    shells: [],
    complete: false,
  });
  const [runs, setRuns] = useState<Record<string, RunState>>({});
  const [hooks, setHooks] = useState<Record<string, SessionHookState>>({});
  const [view, setView] = useState<View>(EMPTY);
  const [maximized, setMaximized] = useState(false);
  const [focusTick, setFocusTick] = useState(0);
  // 저장 파일에 있는 배치. 읽은 직후에는 다시 저장하지 않는다.
  const savedTree = useRef<Node>(EMPTY.tree);
  const [modal, setModal] = useState<Modal>();
  const [notice, setNotice] = useState<string>();
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const starting = useRef(new Set<string>());
  const minimized = useMinimized();
  const [usages, setUsages] = useState<AccountSummary[]>([]);
  const [tickSecs, setTickSecs] = useState(30);
  const [conflicts, setConflicts] = useState<Map<string, string[]>>(new Map());

  const patch = useCallback((id: string, change: (run: RunState) => RunState) => {
    setRuns((all) => ({ ...all, [id]: change(all[id] ?? {}) }));
  }, []);
  const onExited = useCallback(
    (id: string, run: number, code: number) => patch(id, (r) => recordExit(r, run, code)),
    [patch],
  );

  const start = useCallback(
    async (id: string) => {
      if (starting.current.has(id)) return;
      starting.current.add(id);
      setBusy((b) => ({ ...b, [id]: true }));
      try {
        const { run, os_build } = await api.start(id);
        patch(id, (r) => ({ exit: r.exit, started: { run, osBuild: os_build } }));
        // 새 실행이 시작될 때 이전 훅 상태를 지운다.
        setHooks((all) => {
          const { [id]: _, ...rest } = all;
          return rest;
        });
      } catch (error) {
        patch(id, () => ({ error: String(error) }));
      } finally {
        starting.current.delete(id);
        setBusy((b) => ({ ...b, [id]: false }));
      }
    },
    [patch],
  );

  const restart = async (id: string) => {
    if (starting.current.has(id)) return;
    // 끝내는 동안 목록에서 골라도 이전 실행에 다시 붙지 않게 막는다.
    starting.current.add(id);
    setBusy((b) => ({ ...b, [id]: true }));
    patch(id, () => ({}));
    try {
      await api.kill(id);
    } catch (error) {
      setBusy((b) => ({ ...b, [id]: false }));
      throw error;
    } finally {
      starting.current.delete(id);
    }
    await start(id);
  };

  useEffect(() => {
    const unlistenHook = listen<SessionStatePayload>("session-state", ({ payload }) => {
      setHooks((all) => ({
        ...all,
        [payload.id]: {
          run: payload.run,
          live: payload.live ?? undefined,
          context: payload.context ?? undefined,
        },
      }));
    });
    const unlisten = listen<Exit>("session-exit", ({ payload }) =>
      onExited(payload.id, payload.run, payload.code),
    );
    // 종료 이벤트를 놓치지 않도록 구독한 뒤에 목록을 읽는다.
    unlisten
      .then(api.appState)
      .then(({ state, warning }) => {
        const ids = state.sessions.map((s) => s.id);
        const tree = prune(state.layout ?? EMPTY.tree, ids);
        savedTree.current = tree;
        setView({ tree, focus: 0 });
        setState(state);
        if (warning) setNotice(warning);
      })
      .catch((error) => setNotice(String(error)));
    const failed = (error: unknown) => setNotice(String(error));
    api.shells(false).then(
      (shells) => setShells((now) => (now.complete ? now : { shells, complete: false })),
      failed,
    );
    api.shells(true).then((shells) => setShells({ shells, complete: true }), failed);
    return () => {
      void unlistenHook.then((off) => off());
      void unlisten.then((off) => off());
    };
  }, [onExited]);

  useEffect(() => {
    api.accountList().then(setUsages).catch(() => {});
    const unlisten = listen<void>("usage-tick", () => {
      setTickSecs(30);
      api.accountList().then(setUsages).catch(() => {});
    });
    const countdown = setInterval(() => setTickSecs((s) => Math.max(0, s - 1)), 1000);
    return () => {
      void unlisten.then((f) => f());
      clearInterval(countdown);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<{ sessionId: string; files: string[] }>("conflict-detected", ({ payload }) => {
      setConflicts((prev) => {
        const next = new Map(prev);
        next.set(payload.sessionId, payload.files);
        return next;
      });
    });
    return () => { void unlisten.then((f) => f()); };
  }, []);

  useEffect(() => {
    if (view.tree === savedTree.current) return;
    savedTree.current = view.tree;
    saveTree(view.tree).catch((error) => setNotice(`화면 배치를 저장하지 못했어요: ${error}`));
  }, [view.tree]);

  const focusPane = useCallback((index: number) => {
    setView((v) => (v.focus === index ? v : { ...v, focus: index }));
  }, []);
  const { drag, begin } = useSessionDrag(
    view.tree,
    (session, target) => {
      const edge = target.zone === "center" ? undefined : target.zone;
      const dir = edge && (edge === "left" || edge === "right" ? "row" : "col");
      const crowded = dir && !fits(target.pane, dir);
      if (crowded) setNotice(TOO_SMALL);
      const next = crowded ? null : drop(view, session, target.pane, target.zone);
      if (next) {
        setMaximized(false);
        setView(next);
        if (sessionStatus(runs[session]) === "idle") void start(session);
      }
      setFocusTick((n) => n + 1);
    },
    // 끌기를 시작하며 옮겨 간 초점을 터미널로 되돌린다.
    () => setFocusTick((n) => n + 1),
  );

  // 터미널보다 먼저 받아, 앱 단축키는 셸에 보내지 않는다.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "G" && state && !document.querySelector("dialog[open]")) {
        e.preventDefault();
        e.stopPropagation();
        const sessionId = leaves(view.tree)[view.focus];
        const session = state.sessions.find((s) => s.id === sessionId);
        const project = state.projects.find((p) => p.id === session?.projectId);
        if (project) setModal({ type: "changes", project });
        return;
      }
      const shortcut = layoutShortcut(e);
      // 저장한 배치를 읽기 전에는 바꾸지 않는다.
      if (!shortcut || !state || document.querySelector("dialog[open]")) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      const count = leaves(view.tree).length;
      if (shortcut.type === "leave" && document.activeElement?.closest(".pane")) {
        // 레이아웃 단추로 나간다. 거기서 Tab으로 경계선·사이드바에 갈 수 있다.
        // 직접 나눈 배치에는 눌린 프리셋이 없으므로 첫 단추로 간다.
        (
          document.querySelector<HTMLElement>('.layout-bar [aria-pressed="true"]') ??
          document.querySelector<HTMLElement>(".layout-bar button")
        )?.focus();
        return;
      }
      if (shortcut.type === "leave") {
        // 밖에서 다시 누르면 초점 패널로 돌아온다.
      } else if (shortcut.type === "maximize") {
        if (count > 1) setMaximized((m) => !m);
      } else if (shortcut.type === "swap") {
        const mine = leaves(view.tree)[view.focus];
        if (shortcut.index >= count || !mine || shortcut.index === view.focus) return;
        const next = drop(view, mine, shortcut.index, "center");
        if (next) setView(next);
      } else if (shortcut.type === "focus") {
        if (shortcut.index >= count) return;
        focusPane(shortcut.index);
      } else {
        if (!fits(view.focus, shortcut.dir)) return setNotice(TOO_SMALL);
        const next = split(view, shortcut.dir);
        if (!next) return setNotice(`패널은 ${MAX_PANES}개까지 나눌 수 있어요`);
        setMaximized(false);
        setView(next);
      }
      setFocusTick((n) => n + 1);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [state, view, focusPane]);

  if (!state) {
    return (
      <div className="app">
        <div className="empty muted">{notice ?? "불러오는 중"}</div>
      </div>
    );
  }

  const open = (id: string) => {
    setView((v) => show(v, id));
    setFocusTick((n) => n + 1);
    if (sessionStatus(runs[id]) === "idle") void start(id);
  };
  const visible = leaves(view.tree).filter((s) => s !== null);
  const selected = leaves(view.tree)[view.focus] ?? undefined;
  // 두 칸일 때는 사이드바에 어느 쪽에 보이는지 적는다.
  const places: Record<string, string> = {};
  if (view.tree.type === "split" && leaves(view.tree).length === 2) {
    const sides = view.tree.dir === "row" ? ["왼쪽", "오른쪽"] : ["위", "아래"];
    leaves(view.tree).forEach((id, i) => {
      if (id) places[id] = sides[i];
    });
  }
  const arrange = (next: (v: View) => View) => {
    setMaximized(false);
    setView(next);
    setFocusTick((n) => n + 1);
  };
  const close = () => {
    setModal(undefined);
    setFocusTick((n) => n + 1);
  };
  const forget = (ids: string[], next: StoredState) => {
    setState(next);
    setRuns((all) => Object.fromEntries(Object.entries(all).filter(([id]) => !ids.includes(id))));
    setHooks((all) => Object.fromEntries(Object.entries(all).filter(([id]) => !ids.includes(id))));
    const left = next.sessions.map((s) => s.id);
    setView((v) => ({ ...v, tree: prune(v.tree, left) }));
  };
  const pane = (id: string | null, focused: boolean) => {
    const session = state.sessions.find((s) => s.id === id);
    const project = state.projects.find((p) => p.id === session?.projectId);
    if (!session || !project) {
      const none = state.projects.length === 0;
      return (
        <div className="empty">
          <p className="muted">
            {none ? "작업할 폴더를 추가해 주세요" : "세션을 고르거나 새로 열어 주세요"}
          </p>
          <button
            className="btn primary"
            onClick={() => setModal(none ? { type: "project" } : { type: "session" })}
          >
            {none ? "프로젝트 추가" : "새 세션"}
          </button>
        </div>
      );
    }
    const conflictFiles = conflicts.get(session.id) ?? [];
    return (
      <SessionPanel
        project={project}
        session={session}
        shell={shellName(shells, session.shell)}
        run={runs[session.id]}
        hook={hooks[session.id]}
        hidden={minimized}
        busy={busy[session.id] ?? false}
        focused={focused}
        onDragStart={begin(session.id)}
        onStart={() => void start(session.id)}
        onExited={onExited}
        onRestart={() => restart(session.id).catch((error) => setNotice(String(error)))}
        onDelete={() => setModal({ type: "delete-session", session })}
        onApprove={(choice) => {
          const h = hooks[session.id];
          const run = runs[session.id]?.started?.run;
          if (!h || !run || h.live?.type !== "waiting") return;
          api
            .approve(session.id, run, h.live.request, choice)
            .catch((error) => setNotice(String(error)));
        }}
        conflictFiles={conflictFiles}
        onOpenChanges={() => setModal({ type: "changes", project })}
      />
    );
  };

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <Logo />
          Lmux
        </div>
        {usages.length > 0 && (
          <div
            className="usage-cluster"
            role="button"
            tabIndex={0}
            onClick={() => setModal({ type: "accounts" })}
            onKeyDown={(e) => e.key === "Enter" && setModal({ type: "accounts" })}
            aria-label="계정 사용량 패널 열기"
          >
            {usages.map((acc, i) => {
              const total =
                acc.usage.fiveHour.input +
                acc.usage.fiveHour.cacheCreation +
                acc.usage.fiveHour.cacheRead;
              const pct =
                acc.limits.fiveHour > 0
                  ? Math.min(100, Math.round((total / acc.limits.fiveHour) * 100))
                  : 0;
              return (
                <Fragment key={acc.name}>
                  {i > 0 && <span className="usage-sep" />}
                  <span className="usage-label">{acc.name}</span>
                  <span className="usage-mini">
                    <span style={{ width: `${pct}%` }} />
                  </span>
                  <span className="usage-pct mono">{pct}%</span>
                </Fragment>
              );
            })}
            <span className="usage-tick mono">· {tickSecs}초</span>
          </div>
        )}
      </header>
      {notice && (
        <div className="banner" role="alert">
          <span className="grow">{notice}</span>
          <button className="btn ghost" onClick={() => setNotice(undefined)}>
            닫기
          </button>
        </div>
      )}
      <div className="workspace">
        <Sidebar
          state={state}
          runs={runs}
          hooks={hooks}
          shells={shells}
          selected={selected}
          visible={visible}
          onDragStart={begin}
          dragging={drag?.session}
          places={places}
          onSelect={(s) => open(s.id)}
          onAddProject={() => setModal({ type: "project" })}
          onNewSession={(projectId) => setModal({ type: "session", projectId })}
          onRemoveProject={(project) => setModal({ type: "remove-project", project })}
        />
        <main className="main">
          <LayoutBar
            tree={view.tree}
            maximized={maximized}
            saved={state.savedLayouts.map((s) => ({
              ...s,
              tree: prune(
                s.tree,
                state.sessions.map((x) => x.id),
              ),
            }))}
            onPreset={(n) => arrange((v) => preset(v, n))}
            onTranspose={() => arrange((v) => ({ ...v, tree: transpose(v.tree) }))}
            onOpenSaved={() => setModal({ type: "layouts" })}
          />
          <PaneGrid
            view={view}
            maximized={maximized}
            focusTick={focusTick}
            drag={drag}
            onFocus={focusPane}
            onResize={(path, ratio) =>
              setView((v) => ({ ...v, tree: resize(v.tree, path, ratio) }))
            }
          >
            {pane}
          </PaneGrid>
        </main>
      </div>

      {modal?.type === "project" && (
        <NewProject
          shells={shells}
          onClose={close}
          onAdded={({ id, state }) => {
            setState(state);
            setModal({ type: "session", projectId: id });
          }}
        />
      )}
      {modal?.type === "session" && state.projects.length > 0 && (
        <NewSession
          state={state}
          runs={runs}
          shells={shells}
          shellsComplete={complete}
          projectId={modal.projectId}
          onClose={close}
          onCreated={({ id, state }) => {
            setState(state);
            close();
            open(id);
          }}
        />
      )}
      {modal?.type === "layouts" && (
        <LayoutDialog
          saved={state.savedLayouts}
          onClose={close}
          onSave={async (name) => {
            setState(await api.saveLayout(name, view.tree));
            close();
            setFocusTick((n) => n + 1);
          }}
          onLoad={(layout) => {
            close();
            const ids = state.sessions.map((s) => s.id);
            arrange(() => ({ tree: prune(layout.tree, ids), focus: 0 }));
          }}
          onDelete={async (id) => setState(await api.deleteLayout(id))}
        />
      )}
      {modal?.type === "delete-session" && (
        <Dialog
          title="세션 삭제"
          description={`"${modal.session.name}" 세션의 셸을 끝내고 목록에서 지워요. 폴더의 파일은 그대로예요.`}
          submit="삭제"
          danger
          onClose={close}
          onSubmit={async () => {
            forget([modal.session.id], await api.deleteSession(modal.session.id));
            close();
          }}
        />
      )}
      {modal?.type === "accounts" && (
        <AccountsPanel
          usages={usages}
          tickSecs={tickSecs}
          onClose={close}
          onRefresh={() => {
            setTickSecs(30);
            api.accountList().then(setUsages).catch(() => {});
          }}
        />
      )}
      {modal?.type === "changes" && (
        <ChangesPanel
          project={modal.project}
          sessions={state.sessions.filter((s) => s.projectId === modal.project.id)}
          onClose={close}
        />
      )}
      {modal?.type === "remove-project" && (
        <Dialog
          title="프로젝트 등록 해제"
          description={`"${modal.project.name}"과 그 세션을 목록에서 지우고 셸을 끝내요. 폴더의 파일은 지우지 않아요.`}
          submit="등록 해제"
          danger
          onClose={close}
          onSubmit={async () => {
            const ids = state.sessions
              .filter((s) => s.projectId === modal.project.id)
              .map((s) => s.id);
            forget(ids, await api.removeProject(modal.project.id));
            close();
          }}
        />
      )}
    </div>
  );
}
