export type ClipboardAction = "copy" | "paste" | null;

type Key = Pick<KeyboardEvent, "type" | "code" | "ctrlKey" | "altKey" | "shiftKey">;

/** Windows 터미널 관례: Ctrl+C는 선택이 있을 때만 복사(없으면 인터럽트), Ctrl+V는 붙여넣기. */
export function clipboardAction(e: Key, hasSelection: boolean): ClipboardAction {
  if (e.type !== "keydown" || !e.ctrlKey || e.altKey) return null;
  if (e.code === "KeyC" && hasSelection) return "copy";
  if (e.code === "KeyV") return "paste";
  return null;
}

/**
 * xterm이 터미널 질의에 자동으로 보내는 응답인지. 과거 출력을 다시 그리는 동안에는
 * 이것만 버리고 사용자가 친 입력은 그대로 보낸다.
 */
export function isTerminalReply(data: string): boolean {
  // 처음부터 끝까지 응답 하나여야 한다. OSC(`ESC ]`)와 DCS(`ESC P`)는 끝맺음까지 있어야 한다.
  return /^\x1b(?:\[(?:[?>=]?[\d;]*(?:\$y|[Rcnt])|[IO])|\]\d[^\x07\x1b]*(?:\x07|\x1b\\)|P[^\x1b]*\x1b\\)$/.test(
    data,
  );
}

export type LayoutShortcut =
  | { type: "split"; dir: "row" | "col" }
  | { type: "focus"; index: number }
  | { type: "maximize" }
  | { type: "leave" }
  | { type: "swap"; index: number };

/** 앱이 가져가는 분할 단축키. `Ctrl+\`는 셸에 보내지 않는다. */
export function layoutShortcut(e: Key & { metaKey?: boolean }): LayoutShortcut | null {
  if (e.type !== "keydown" || !e.ctrlKey || e.altKey || e.metaKey) return null;
  if (e.code === "Backslash") return { type: "split", dir: e.shiftKey ? "col" : "row" };
  if (e.code === "Enter") return e.shiftKey ? { type: "maximize" } : null;
  // 터미널은 Tab을 셸에 보내므로, 터미널 밖으로 나가는 키가 따로 필요하다.
  if (e.code === "F6") return e.shiftKey ? null : { type: "leave" };
  const digit = /^Digit([1-4])$/.exec(e.code);
  if (!digit) return null;
  const index = Number(digit[1]) - 1;
  // Ctrl+Shift+N: 초점 패널의 세션을 N번 패널과 바꾼다.
  return e.shiftKey ? { type: "swap", index } : { type: "focus", index };
}
