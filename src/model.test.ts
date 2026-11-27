import { expect, test } from "vitest";
import type { StoredState } from "./api";
import { exitCode, groupByProject, recordExit, runningIn, sessionStatus, statusText } from "./model";

const started = { started: { run: 2, osBuild: 1 } };

test("상태는 지금 실행의 종료만 반영한다", () => {
  expect(sessionStatus(undefined)).toBe("idle");
  expect(sessionStatus(started)).toBe("running");
  expect(sessionStatus({ ...started, exit: { run: 1, code: 0 } })).toBe("running");
  expect(sessionStatus({ ...started, exit: { run: 2, code: 0 } })).toBe("exited");
  expect(sessionStatus({ error: "없음" })).toBe("failed");
  expect(exitCode({ ...started, exit: { run: 2, code: 9 } })).toBe(9);
  expect(exitCode({ ...started, exit: { run: 1, code: 9 } })).toBeUndefined();
});

test("상태 글자", () => {
  expect(statusText(undefined)).toBe("시작 전");
  expect(statusText(started)).toBe("실행 중");
  expect(statusText({ ...started, exit: { run: 2, code: 9 } })).toBe("종료됨 (코드 9)");
  expect(statusText({ error: "x" })).toBe("시작 실패");
});

test("늦게 온 이전 실행의 종료는 무시한다", () => {
  const exited = recordExit(started, 2, 3);
  expect(recordExit(exited, 1, 0)).toBe(exited);
  // 시작 응답보다 종료 알림이 먼저 올 수 있다.
  expect(recordExit({}, 4, 1).exit).toEqual({ run: 4, code: 1 });
});

const state: StoredState = {
  projects: ["a", "b"].map((id) => ({
    id,
    name: id,
    path: `D:\\${id}`,
    kind: "folder" as const,
    defaultShell: "cmd",
    defaultAgent: "none" as const,
  })),
  sessions: [
    ["s1", "b"],
    ["s2", "a"],
    ["s3", "a"],
  ].map(([id, projectId]) => ({
    id,
    projectId,
    name: id,
    shell: "cmd",
    agent: "none" as const,
    customCommand: "",
  })),
  layout: null,
  savedLayouts: [],
};

test("프로젝트 순서대로 세션을 묶고, 세션 없는 프로젝트도 보인다", () => {
  const groups = groupByProject({ ...state, sessions: state.sessions.slice(1) });
  expect(groups.map((g) => [g.project.id, g.sessions.map((s) => s.id)])).toEqual([
    ["a", ["s2", "s3"]],
    ["b", []],
  ]);
});

test("같은 프로젝트에서 실행 중인 세션만 고른다", () => {
  const runs = { s1: started, s2: started, s3: { ...started, exit: { run: 2, code: 0 } } };
  expect(runningIn(state, runs, "a").map((s) => s.id)).toEqual(["s2"]);
});
