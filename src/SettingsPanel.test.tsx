// @vitest-environment jsdom
import { vi, test, expect, beforeEach, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import SettingsPanel from "./SettingsPanel";
import { DEFAULT_SETTINGS } from "./api";

const mockSettings = { ...DEFAULT_SETTINGS };

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === "get_settings") return Promise.resolve(mockSettings);
    if (cmd === "save_settings") return Promise.resolve();
    return Promise.resolve();
  }),
}));

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
});

test("renders settings nav items", async () => {
  render(<SettingsPanel onClose={() => {}} />);
  expect(screen.getByRole("button", { name: "셸 프로필" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "알림" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "정보 · 업데이트" })).toBeTruthy();
});

test("renders initial screen heading", async () => {
  render(<SettingsPanel onClose={() => {}} />);
  expect(screen.getByRole("heading", { name: "셸 프로필" })).toBeTruthy();
});
