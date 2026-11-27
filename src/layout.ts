/** 화면 배치: 패널을 잎으로 하는 이진 분할 트리. 모든 함수는 새 값을 돌려준다. */
export type Dir = "row" | "col";
export type Node =
  | { type: "pane"; session: string | null }
  | { type: "split"; dir: Dir; ratio: number; first: Node; second: Node };
export type Preset = 1 | 2 | 3 | 4;
export type Zone = "center" | "left" | "right" | "top" | "bottom";
/** `focus`는 `leaves` 순서의 패널 번호. */
export type View = { tree: Node; focus: number };
export type Rect = { x: number; y: number; w: number; h: number };
/** `path`는 뿌리에서 내려가는 길(0 = first, 1 = second). */
export type Divider = { path: string; dir: Dir; ratio: number; area: Rect };

export const MAX_PANES = 4;
/** 경계선을 옮길 때 패널마다 남기는 크기(머리글과 터미널 몇 줄). */
export const MIN_PANE = { row: 160, col: 110 };

/** 가로 `w`·세로 `h`픽셀 패널을 `dir`로 나눠도 두 쪽이 최소 크기를 지키는지. */
export function roomFor(dir: Dir, w: number, h: number): boolean {
  // 나눈 두 패널 사이의 간격(10px)도 필요하다.
  return dir === "row" ? w >= 2 * MIN_PANE.row + 10 : h >= 2 * MIN_PANE.col + 10;
}
export const MIN_RATIO = 0.15;
export const MAX_RATIO = 1 - MIN_RATIO;

const pane = (session: string | null): Node => ({ type: "pane", session });
const join = (dir: Dir, first: Node, second: Node, ratio = 0.5): Node => ({
  type: "split",
  dir,
  ratio,
  first,
  second,
});

export function leaves(tree: Node): (string | null)[] {
  return tree.type === "pane" ? [tree.session] : [...leaves(tree.first), ...leaves(tree.second)];
}

/** `index`번째 패널을 `change`의 결과로 바꾼다. `null`이면 패널을 없애고 형제가 자리를 채운다. */
function atPane(tree: Node, index: number, change: (session: string | null) => Node | null): Node | null {
  if (tree.type === "pane") return change(tree.session);
  const left = leaves(tree.first).length;
  const inFirst = index < left;
  const child = atPane(inFirst ? tree.first : tree.second, inFirst ? index : index - left, change);
  if (child === null) return inFirst ? tree.second : tree.first;
  return inFirst ? { ...tree, first: child } : { ...tree, second: child };
}

function fill(tree: Node, sessions: (string | null)[]): Node {
  let i = 0;
  const walk = (node: Node): Node =>
    node.type === "pane"
      ? pane(sessions[i++] ?? null)
      : { ...node, first: walk(node.first), second: walk(node.second) };
  return walk(tree);
}

function template(n: Preset): Node {
  const empty = pane(null);
  if (n === 1) return empty;
  if (n === 2) return join("row", empty, empty);
  if (n === 3) return join("row", empty, join("col", empty, empty), 0.55);
  return join("row", join("col", empty, empty), join("col", empty, empty));
}

export function preset(view: View, n: Preset): View {
  const current = leaves(view.tree);
  const focused = current[view.focus] ?? null;
  // 패널이 늘면 자리를 그대로 두고, 줄면 빈 패널부터 버린다.
  let sessions = current;
  if (n < current.length) {
    sessions = current.filter((s) => s !== null);
    // 보고 있던 세션은 잘려 나가지 않게 맨 앞으로 옮긴다.
    if (focused !== null && sessions.indexOf(focused) >= n) {
      sessions = [focused, ...sessions.filter((s) => s !== focused)];
    }
  }
  const tree = fill(template(n), sessions);
  const focus = focused === null ? Math.min(view.focus, n - 1) : leaves(tree).indexOf(focused);
  return { tree, focus: Math.max(0, focus) };
}

/** 비율만 다르거나 좌우·상하를 통째로 바꾼 모양이면 그 레이아웃으로 본다. */
export function presetOf(tree: Node): Preset | null {
  const shape = (node: Node): string =>
    node.type === "pane" ? "p" : `${node.dir}(${shape(node.first)}${shape(node.second)})`;
  const now = shape(tree);
  const same = (n: Preset) => [template(n), transpose(template(n))].some((t) => shape(t) === now);
  return ([1, 2, 3, 4] as const).find(same) ?? null;
}

/** 이미 보이면 그 패널로, 아니면 초점 패널(비었을 때) → 빈 패널 → 초점 패널 교체 순서로 연다. */
export function show(view: View, session: string): View {
  const current = leaves(view.tree);
  const visible = current.indexOf(session);
  if (visible >= 0) return { tree: view.tree, focus: visible };
  const empty = current.indexOf(null);
  const focus = current[view.focus] === null || empty < 0 ? view.focus : empty;
  return { tree: atPane(view.tree, focus, () => pane(session))!, focus };
}

export function split(view: View, dir: Dir): View | null {
  if (leaves(view.tree).length >= MAX_PANES) return null;
  const tree = atPane(view.tree, view.focus, (s) => join(dir, pane(s), pane(null)))!;
  return { tree, focus: view.focus + 1 };
}

/** 놓았을 때 실제로 적용할 위치. 빈 패널이거나 더 나눌 수 없으면 가운데로 본다. */
export function dropZone(tree: Node, session: string, target: number, zone: Zone): Zone {
  const current = leaves(tree);
  const full = current.length >= MAX_PANES && !current.includes(session);
  return current[target] === null || full ? "center" : zone;
}

export function drop(view: View, session: string, target: number, zone: Zone): View | null {
  const current = leaves(view.tree);
  const source = current.indexOf(session);
  const where = dropZone(view.tree, session, target, zone);
  if (where === "center") {
    // 이미 보이던 세션이면 대상에 있던 것과 자리를 바꾼다.
    const next = [...current];
    if (source >= 0) next[source] = current[target];
    next[target] = session;
    return { tree: fill(view.tree, next), focus: target };
  }
  if (source === target) return null;
  let tree = view.tree;
  let at = target;
  if (source >= 0) {
    tree = atPane(tree, source, () => null)!;
    if (source < at) at -= 1;
  }
  const before = where === "left" || where === "top";
  const dir: Dir = where === "left" || where === "right" ? "row" : "col";
  tree = atPane(tree, at, (s) =>
    before ? join(dir, pane(session), pane(s)) : join(dir, pane(s), pane(session)),
  )!;
  return { tree, focus: before ? at : at + 1 };
}

export function resize(tree: Node, path: string, ratio: number): Node {
  if (tree.type === "pane") return tree;
  if (path === "") {
    // 저장했다가 읽어도 같은 값이 되도록 자릿수를 줄인다.
    const short = Math.round(ratio * 1e4) / 1e4;
    return { ...tree, ratio: Math.min(MAX_RATIO, Math.max(MIN_RATIO, short)) };
  }
  const rest = path.slice(1);
  return path[0] === "0"
    ? { ...tree, first: resize(tree.first, rest, ratio) }
    : { ...tree, second: resize(tree.second, rest, ratio) };
}

/** 안의 모든 패널이 `min` 픽셀을 지키는 데 필요한 길이. */
function need(node: Node, dir: Dir, min: number): number {
  if (node.type === "pane") return min;
  const first = need(node.first, dir, min);
  const second = need(node.second, dir, min);
  return node.dir === dir
    ? Math.max(first / node.ratio, second / (1 - node.ratio))
    : Math.max(first, second);
}

/** `path`의 경계선이 갈 수 있는 비율. `size`는 그 경계선이 나누는 영역의 길이(픽셀). */
export function ratioRange(tree: Node, path: string, size: number, min: number): [number, number] {
  if (tree.type === "pane") return [MIN_RATIO, MAX_RATIO];
  if (path !== "") {
    const child = path[0] === "0" ? tree.first : tree.second;
    return ratioRange(child, path.slice(1), size, min);
  }
  const low = Math.max(MIN_RATIO, need(tree.first, tree.dir, min) / size);
  const high = Math.min(MAX_RATIO, 1 - need(tree.second, tree.dir, min) / size);
  // 창이 작아 이미 좁으면 지금보다 더 좁히지만 않는다.
  return [Math.min(low, tree.ratio), Math.max(high, tree.ratio)];
}

export function transpose(tree: Node): Node {
  if (tree.type === "pane") return tree;
  return join(tree.dir === "row" ? "col" : "row", transpose(tree.first), transpose(tree.second), tree.ratio);
}

/** 지운 세션과 두 번 나오는 세션은 빈 패널로 바꾼다. */
export function prune(tree: Node, ids: string[]): Node {
  const left = new Set(ids);
  return fill(
    tree,
    leaves(tree).map((s) => (s !== null && left.delete(s) ? s : null)),
  );
}

export function geometry(tree: Node): { panes: Rect[]; dividers: Divider[] } {
  const panes: Rect[] = [];
  const dividers: Divider[] = [];
  // 0.4처럼 나눈 값의 오차를 없앤다.
  const round = (v: number) => Math.round(v * 1e6) / 1e6;
  const walk = (node: Node, area: Rect, path: string) => {
    if (node.type === "pane") {
      panes.push(area);
      return;
    }
    dividers.push({ path, dir: node.dir, ratio: node.ratio, area });
    const { x, y, w, h } = area;
    if (node.dir === "row") {
      const first = round(w * node.ratio);
      walk(node.first, { x, y, w: first, h }, `${path}0`);
      walk(node.second, { x: round(x + first), y, w: round(w - first), h }, `${path}1`);
    } else {
      const first = round(h * node.ratio);
      walk(node.first, { x, y, w, h: first }, `${path}0`);
      walk(node.second, { x, y: round(y + first), w, h: round(h - first) }, `${path}1`);
    }
  };
  walk(tree, { x: 0, y: 0, w: 1, h: 1 }, "");
  return { panes, dividers };
}

/** 패널 안의 상대 좌표(0~1). 가운데 절반은 "center", 바깥은 가장 가까운 변. */
export function zoneAt(x: number, y: number): Zone {
  if (x > 0.25 && x < 0.75 && y > 0.25 && y < 0.75) return "center";
  const edges: [Zone, number][] = [
    ["left", x],
    ["right", 1 - x],
    ["top", y],
    ["bottom", 1 - y],
  ];
  return edges.reduce((best, edge) => (edge[1] < best[1] ? edge : best))[0];
}
