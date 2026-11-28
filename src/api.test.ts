import { describe, test, expect } from "vitest";
import type { ChangeSummary } from "./api";

describe("ChangeSummary type", () => {
  test("accepts valid svn summary", () => {
    const s: ChangeSummary = {
      kind: "svn",
      revision: "trunk @ r100",
      branch: null,
      files: [],
      addCount: 0,
      delCount: 0,
    };
    expect(s.kind).toBe("svn");
  });

  test("accepts valid git summary", () => {
    const s: ChangeSummary = {
      kind: "git",
      revision: null,
      branch: "main",
      files: [],
      addCount: 1,
      delCount: 2,
    };
    expect(s.branch).toBe("main");
  });
});
