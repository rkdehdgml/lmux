import { expect, test } from "vitest";
import { clipboardAction, isTerminalReply, layoutShortcut } from "./keys";

const key = (code: string, mods: Partial<KeyboardEvent> = {}) => ({
  type: "keydown",
  code,
  ctrlKey: true,
  altKey: false,
  shiftKey: false,
  ...mods,
});

test("Ctrl+C는 선택이 있을 때만 복사", () => {
  expect(clipboardAction(key("KeyC"), true)).toBe("copy");
  expect(clipboardAction(key("KeyC"), false)).toBeNull();
});

test("Ctrl+V는 붙여넣기, 다른 조합은 터미널로", () => {
  expect(clipboardAction(key("KeyV"), false)).toBe("paste");
  expect(clipboardAction(key("KeyV", { altKey: true }), false)).toBeNull();
  expect(clipboardAction(key("KeyV", { type: "keyup" }), false)).toBeNull();
  expect(clipboardAction(key("KeyD"), true)).toBeNull();
});

test("터미널 자동 응답만 골라낸다", () => {
  const replies = ["\x1b[1;1R", "\x1b[?1;2c", "\x1b[>0;276;0c", "\x1b[0n", "\x1b[?2004;1$y"];
  const more = ["\x1b]10;rgb:ff/ff/ff\x1b\\", "\x1bP1$r0m\x1b\\", "\x1b[I", "\x1b[O"];
  for (const data of [...replies, ...more]) expect(isTerminalReply(data), data).toBe(true);
  const typed = ["a", "\r", "한글", "\x1b", "\x1b[A", "\x1bOP", "\x1b[15~", "\x1b[200~x\x1b[201~"];
  // Alt+], Alt+Shift+P, 끝맺음이 없는 붙여넣기는 응답이 아니다.
  typed.push("\x1b]", "\x1bP", "\x1b]붙여넣은 글");
  // 응답 모양으로 시작해도 뒤에 글이 이어지면 붙여넣기다.
  typed.push("\x1b[1;1R뒤에 글", "\x1b[0n abc");
  typed.push("\x1b]10;a\x07글\x1b]11;b\x07", "\x1bP1\x1b\\글\x1bP2\x1b\\");
  for (const data of typed) expect(isTerminalReply(data), data).toBe(false);
});

test("분할 단축키", () => {
  expect(layoutShortcut(key("Backslash"))).toEqual({ type: "split", dir: "row" });
  expect(layoutShortcut(key("Backslash", { shiftKey: true }))).toEqual({
    type: "split",
    dir: "col",
  });
  expect(layoutShortcut(key("Digit1"))).toEqual({ type: "focus", index: 0 });
  expect(layoutShortcut(key("Digit4"))).toEqual({ type: "focus", index: 3 });
  expect(layoutShortcut(key("Enter", { shiftKey: true }))).toEqual({ type: "maximize" });
  expect(layoutShortcut(key("F6"))).toEqual({ type: "leave" });
  expect(layoutShortcut(key("Digit3", { shiftKey: true }))).toEqual({ type: "swap", index: 2 });
});

test("그 밖의 조합은 터미널로", () => {
  expect(layoutShortcut(key("Backslash", { ctrlKey: false }))).toBeNull();
  // AltGr은 Ctrl+Alt로 들어온다.
  expect(layoutShortcut(key("Backslash", { altKey: true }))).toBeNull();
  expect(layoutShortcut(key("Digit1", { metaKey: true }))).toBeNull();
  expect(layoutShortcut(key("Digit5", { shiftKey: true }))).toBeNull();
  expect(layoutShortcut(key("Digit5"))).toBeNull();
  expect(layoutShortcut(key("Enter"))).toBeNull();
  // F6 단독은 셸 프로그램(htop, mc)이 쓴다.
  expect(layoutShortcut(key("F6", { ctrlKey: false }))).toBeNull();
  expect(layoutShortcut(key("Backslash", { type: "keyup" }))).toBeNull();
});
