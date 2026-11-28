import { expect, test } from "vitest";
import type { StoredState } from "./api";
import {
  contextColor,
  contextPercent,
  exitCode,
  formatContext,
  groupByProject,
  hookStatusText,
  recordExit,
  runningIn,
  sessionStatus,
  statusText,
  type HookContext,
  type HookLive,
} from "./model";

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

test("훅 상태가 있으면 그 상태 글자를 쓰고, 없으면 undefined를 돌려준다", () => {
  expect(hookStatusText(undefined)).toBeUndefined();
  expect(hookStatusText({ type: "working" } satisfies HookLive)).toBe("작업 중");
  expect(hookStatusText({ type: "waiting", request: "tool123" } satisfies HookLive)).toBe("승인 대기");
  expect(hookStatusText({ type: "done" } satisfies HookLive)).toBe("완료");
});

test("컨텍스트 색 경계: 50% 이상 주황, 90% 이상 빨강", () => {
  expect(contextColor(0)).toBe("normal");
  expect(contextColor(49)).toBe("normal");
  expect(contextColor(50)).toBe("warn");
  expect(contextColor(89)).toBe("warn");
  expect(contextColor(90)).toBe("danger");
  expect(contextColor(100)).toBe("danger");
});

test("컨텍스트 퍼센트 계산", () => {
  const ctx = (used: number, size: number): HookContext => ({ used, size, estimated: false });
  expect(contextPercent(ctx(100000, 200000))).toBe(50);
  expect(contextPercent(ctx(180000, 200000))).toBe(90);
  expect(contextPercent(ctx(0, 200000))).toBe(0);
  expect(contextPercent(ctx(168000, 200000))).toBe(84);
});

test("컨텍스트 크기 표기: K 단위, 반올림", () => {
  const ctx = (used: number, size: number): HookContext => ({ used, size, estimated: false });
  expect(formatContext(ctx(168000, 200000))).toBe("168K/200K");
  expect(formatContext(ctx(500, 1000))).toBe("500/1K");
  expect(formatContext(ctx(1500, 2000))).toBe("2K/2K");
});
