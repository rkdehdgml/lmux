import type { StoredState } from "./api";

/** 저장하지 않는 실행 상태. */
export type RunState = {
  started?: { run: number; osBuild: number };
  exit?: { run: number; code: number };
  error?: string;
};

export type Status = "idle" | "running" | "exited" | "failed";

/** 지금 실행이 끝났을 때의 종료 코드. */
export function exitCode(state?: RunState): number | undefined {
  return state?.started && state.exit?.run === state.started.run ? state.exit.code : undefined;
}

export function sessionStatus(state?: RunState): Status {
  if (state?.error) return "failed";
  if (!state?.started) return "idle";
  return exitCode(state) === undefined ? "running" : "exited";
}

export function statusText(state?: RunState): string {
  switch (sessionStatus(state)) {
    case "idle":
      return "시작 전";
    case "running":
      return "실행 중";
    case "exited":
      return `종료됨 (코드 ${exitCode(state)})`;
    case "failed":
      return "시작 실패";
  }
}

/** 이전 실행의 종료 알림이 늦게 와도 새 실행의 기록을 덮어쓰지 않는다. */
export function recordExit(state: RunState, run: number, code: number): RunState {
  return state.exit && state.exit.run > run ? state : { ...state, exit: { run, code } };
}

export function groupByProject(state: StoredState) {
  return state.projects.map((project) => ({
    project,
    sessions: state.sessions.filter((s) => s.projectId === project.id),
  }));
}

export function runningIn(state: StoredState, runs: Record<string, RunState>, projectId: string) {
  return state.sessions.filter(
    (s) => s.projectId === projectId && sessionStatus(runs[s.id]) === "running",
  );
}
