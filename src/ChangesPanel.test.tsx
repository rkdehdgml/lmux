// @vitest-environment jsdom
import { vi, test, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ChangesPanel from "./ChangesPanel";
import type { Project } from "./api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    kind: "svn",
    revision: null,
    branch: null,
    files: [],
    addCount: 0,
    delCount: 0,
  }),
}));

const proj: Project = {
  id: "p1",
  name: "gov-portal",
  path: "C:/proj",
  kind: "svn",
  defaultShell: "pwsh",
  defaultAgent: "claude",
};

test("renders project name in header", () => {
  render(<ChangesPanel project={proj} sessions={[]} onClose={() => {}} />);
  expect(screen.getByText("변경 사항 · gov-portal")).toBeTruthy();
});

test("shows loading state initially", () => {
  render(<ChangesPanel project={proj} sessions={[]} onClose={() => {}} />);
  expect(screen.getByRole("status")).toBeTruthy();
});
