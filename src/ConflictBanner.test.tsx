// @vitest-environment jsdom
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ConflictBanner from "./ConflictBanner";

test("shows conflict file count", () => {
  render(<ConflictBanner files={["a.rs", "b.rs"]} onOpenChanges={() => {}} />);
  expect(screen.getByText(/2개/)).toBeTruthy();
});

test("does not render when no conflicts", () => {
  const { container } = render(<ConflictBanner files={[]} onOpenChanges={() => {}} />);
  expect(container.firstChild).toBeNull();
});
