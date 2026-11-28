// @vitest-environment jsdom
import { vi, test, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import SetLook from "./SetLook";
import type { AppSettings } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

afterEach(() => cleanup());

const defaults: AppSettings = {
  notifyWaiting: true, notifyDone: true, notifyError: true,
  notifyConflict: true, notifyContext: true, notifyContextThreshold: 80,
  theme: "dark", language: "ko", density: "normal",
  terminalFont: "JetBrains Mono", terminalFontSize: 13, terminalLineHeight: 1.6,
  cursorShape: "block", webgl: true, hideInactive: true, scrollbackLines: 10000,
  preventSleep: true, minimizeToTray: true, restoreSessions: true,
};

test("renders theme heading", () => {
  render(<SetLook settings={defaults} onChange={() => {}} />);
  expect(screen.getByRole("heading", { name: "화면" })).toBeTruthy();
});

test("dark theme radio is checked", () => {
  render(<SetLook settings={defaults} onChange={() => {}} />);
  const darkBtn = screen.getByRole("radio", { name: "다크" });
  expect(darkBtn.getAttribute("aria-checked")).toBe("true");
});
