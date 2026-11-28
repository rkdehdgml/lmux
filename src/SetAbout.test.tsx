// @vitest-environment jsdom
import { test, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import SetAbout from "./SetAbout";

afterEach(() => cleanup());

test("renders app version", () => {
  render(<SetAbout />);
  expect(screen.getByText(/v0\.1\.0/)).toBeTruthy();
});

test("renders license type", () => {
  render(<SetAbout />);
  expect(screen.getByText("MIT")).toBeTruthy();
});
