import { describe, expect, it } from "vitest";
import {
  drop,
  dropZone,
  geometry,
  leaves,
  type Node,
  preset,
  presetOf,
  prune,
  ratioRange,
  resize,
  roomFor,
  show,
  split,
  transpose,
  type View,
  zoneAt,
} from "./layout";

const pane = (session: string | null): Node => ({ type: "pane", session });
const row = (first: Node, second: Node, ratio = 0.5): Node => ({
  type: "split",
  dir: "row",
  ratio,
  first,
  second,
});
const col = (first: Node, second: Node, ratio = 0.5): Node => ({
  type: "split",
  dir: "col",
  ratio,
  first,
  second,
});
const view = (tree: Node, focus = 0): View => ({ tree, focus });

describe("preset", () => {
  it("builds the four layouts and recognizes them", () => {
    for (const n of [1, 2, 3, 4] as const) {
      const next = preset(view(pane("a")), n);
      expect(leaves(next.tree)).toHaveLength(n);
      expect(leaves(next.tree)[0]).toBe("a");
      expect(presetOf(next.tree)).toBe(n);
    }
  });

  it("keeps the focused session when panes are reduced", () => {
    const four = view(row(col(pane("a"), pane("b")), col(pane("c"), pane("d"))), 3);
    const one = preset(four, 1);
    expect(leaves(one.tree)).toEqual(["d"]);
    const two = preset(four, 2);
    expect(leaves(two.tree)).toEqual(["d", "a"]);
    expect(two.focus).toBe(0);
  });

  it("keeps the order and focus when panes are added", () => {
    const next = preset(view(row(pane("a"), pane("b")), 1), 4);
    expect(leaves(next.tree)).toEqual(["a", "b", null, null]);
    expect(next.focus).toBe(1);
  });

  it("is still recognized after resizing but not after other changes", () => {
    expect(presetOf(row(pane("a"), pane("b"), 0.7))).toBe(2);
    expect(presetOf(col(pane("a"), pane("b")))).toBe(2);
    expect(presetOf(row(row(pane("a"), pane("b")), pane("c")))).toBeNull();
  });

  it("tells apart layouts with the same tree shape but other directions", () => {
    const two = row(pane("a"), pane("b"));
    expect(presetOf(row(two, two))).toBeNull();
    expect(presetOf(row(pane("a"), two))).toBeNull();
    expect(presetOf(col(pane("a"), two))).toBe(3);
    expect(presetOf(col(two, two))).toBe(4);
  });
});

describe("show", () => {
  it("focuses a session that is already visible", () => {
    const start = view(row(pane("a"), pane("b")));
    expect(show(start, "b")).toEqual({ tree: start.tree, focus: 1 });
  });

  it("fills the focused pane if empty, then any empty pane, then replaces", () => {
    expect(show(view(row(pane(null), pane(null)), 1), "a")).toEqual(
      view(row(pane(null), pane("a")), 1),
    );
    expect(show(view(row(pane("a"), pane(null))), "b")).toEqual(
      view(row(pane("a"), pane("b")), 1),
    );
    expect(show(view(row(pane("a"), pane("b")), 1), "c")).toEqual(
      view(row(pane("a"), pane("c")), 1),
    );
  });
});

describe("split", () => {
  it("adds an empty pane beside or below the focused one and focuses it", () => {
    expect(split(view(pane("a")), "row")).toEqual(view(row(pane("a"), pane(null)), 1));
    expect(split(view(row(pane("a"), pane("b"))), "col")).toEqual(
      view(row(col(pane("a"), pane(null)), pane("b")), 1),
    );
  });

  it("refuses a fifth pane", () => {
    expect(split(preset(view(pane("a")), 4), "row")).toBeNull();
  });
});

describe("drop", () => {
  it("splits on an edge and puts the session on that side", () => {
    expect(drop(view(pane("a")), "b", 0, "left")).toEqual(view(row(pane("b"), pane("a"))));
    expect(drop(view(pane("a")), "b", 0, "bottom")).toEqual(view(col(pane("a"), pane("b")), 1));
  });

  it("moves a visible session instead of showing it twice", () => {
    const start = view(row(pane("a"), col(pane("b"), pane("c"))));
    expect(drop(start, "a", 2, "right")).toEqual(
      view(col(pane("b"), row(pane("c"), pane("a"))), 2),
    );
  });

  it("swaps on the center and fills an empty pane from any zone", () => {
    expect(drop(view(row(pane("a"), pane("b"))), "a", 1, "center")).toEqual(
      view(row(pane("b"), pane("a")), 1),
    );
    expect(drop(view(row(pane("a"), pane(null))), "c", 1, "left")).toEqual(
      view(row(pane("a"), pane("c")), 1),
    );
  });

  it("does nothing when a session is dropped on its own edge", () => {
    expect(drop(view(row(pane("a"), pane("b"))), "a", 0, "left")).toBeNull();
  });

  it("only replaces when four panes are full, but still moves a visible one", () => {
    const full = preset(view(pane("a")), 4).tree;
    expect(dropZone(full, "z", 0, "left")).toBe("center");
    const filled = row(col(pane("a"), pane("b")), col(pane("c"), pane("d")));
    expect(dropZone(filled, "z", 0, "left")).toBe("center");
    expect(dropZone(filled, "d", 0, "left")).toBe("left");
    expect(leaves(drop(view(filled), "d", 0, "left")!.tree)).toEqual(["d", "a", "b", "c"]);
  });
});

describe("resize, transpose, prune", () => {
  it("changes one split by path and clamps the ratio", () => {
    const start = row(pane("a"), col(pane("b"), pane("c")));
    expect(resize(start, "1", 0.99)).toEqual(row(pane("a"), col(pane("b"), pane("c"), 0.85)));
    expect(resize(start, "", 0)).toEqual(row(pane("a"), col(pane("b"), pane("c")), 0.15));
  });

  it("keeps ratios short so that saved layouts compare equal", () => {
    expect(resize(row(pane("a"), pane("b")), "", 0.4234567891)).toEqual(
      row(pane("a"), pane("b"), 0.4235),
    );
  });

  it("limits a divider so that every pane keeps a minimum size", () => {
    expect(ratioRange(row(pane("a"), col(pane("b"), pane("c"))), "", 1000, 160)).toEqual([
      0.16, 0.84,
    ]);
    // 안쪽에서 다시 나눈 쪽은 그만큼 더 커야 한다.
    const [low, high] = ratioRange(col(col(pane("a"), pane("b")), pane("c")), "", 600, 110);
    expect(low).toBeCloseTo(220 / 600);
    expect(high).toBeCloseTo(1 - 110 / 600);
    // 이미 좁으면 지금보다 더 좁히지만 않는다.
    expect(ratioRange(row(pane("a"), pane("b"), 0.3), "", 200, 160)).toEqual([0.3, 0.3]);
  });

  it("splits only a pane that is big enough for two", () => {
    expect(roomFor("row", 330, 50)).toBe(true);
    expect(roomFor("row", 329, 900)).toBe(false);
    expect(roomFor("col", 50, 230)).toBe(true);
    expect(roomFor("col", 900, 229)).toBe(false);
  });

  it("flips every direction", () => {
    expect(transpose(row(pane("a"), col(pane("b"), pane("c"))))).toEqual(
      col(pane("a"), row(pane("b"), pane("c"))),
    );
  });

  it("empties panes of missing or repeated sessions", () => {
    expect(prune(row(pane("gone"), col(pane("a"), pane("a"))), ["a"])).toEqual(
      row(pane(null), col(pane("a"), pane(null))),
    );
  });
});

describe("geometry", () => {
  it("gives each pane and divider its area", () => {
    const { panes, dividers } = geometry(row(pane("a"), col(pane("b"), pane("c")), 0.6));
    expect(panes).toEqual([
      { x: 0, y: 0, w: 0.6, h: 1 },
      { x: 0.6, y: 0, w: 0.4, h: 0.5 },
      { x: 0.6, y: 0.5, w: 0.4, h: 0.5 },
    ]);
    expect(dividers).toEqual([
      { path: "", dir: "row", ratio: 0.6, area: { x: 0, y: 0, w: 1, h: 1 } },
      { path: "1", dir: "col", ratio: 0.5, area: { x: 0.6, y: 0, w: 0.4, h: 1 } },
    ]);
  });

  it("picks the zone under the pointer", () => {
    expect(zoneAt(0.5, 0.5)).toBe("center");
    expect(zoneAt(0.05, 0.5)).toBe("left");
    expect(zoneAt(0.95, 0.6)).toBe("right");
    expect(zoneAt(0.5, 0.02)).toBe("top");
    expect(zoneAt(0.6, 0.98)).toBe("bottom");
  });
});
