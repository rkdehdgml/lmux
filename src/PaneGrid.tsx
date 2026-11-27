import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { landFocus } from "./focus";
import {
  type Divider,
  geometry,
  leaves,
  MAX_RATIO,
  MIN_PANE,
  MIN_RATIO,
  ratioRange,
  type Rect,
  resize,
  type View,
  type Zone,
} from "./layout";
import type { DragState } from "./useSessionDrag";

const GAP = 10;
const KEY_STEP = 0.02;
const DROP_TEXT: Record<Zone, string> = {
  center: "여기 놓으면 이 패널에 열어요",
  left: "여기 놓으면 왼쪽으로 분할",
  right: "여기 놓으면 오른쪽으로 분할",
  top: "여기 놓으면 위로 분할",
  bottom: "여기 놓으면 아래로 분할",
};

const percent = (v: number) => `${v * 100}%`;

/** 이웃한 패널 사이에만 간격을 둔다. */
function box({ x, y, w, h }: Rect): CSSProperties {
  const left = x > 0 ? GAP / 2 : 0;
  const top = y > 0 ? GAP / 2 : 0;
  const right = x + w < 0.999999 ? GAP / 2 : 0;
  const bottom = y + h < 0.999999 ? GAP / 2 : 0;
  return {
    left: `calc(${percent(x)} + ${left}px)`,
    top: `calc(${percent(y)} + ${top}px)`,
    width: `calc(${percent(w)} - ${left + right}px)`,
    height: `calc(${percent(h)} - ${top + bottom}px)`,
  };
}

function dividerBox({ dir, ratio, area }: Divider): CSSProperties {
  const outer = box(area);
  return dir === "row"
    ? { ...outer, left: `calc(${percent(area.x + area.w * ratio)} - ${GAP / 2}px)`, width: GAP }
    : { ...outer, top: `calc(${percent(area.y + area.h * ratio)} - ${GAP / 2}px)`, height: GAP };
}

type PaneProps = {
  index: number;
  focused: boolean;
  focusTick: number;
  style: CSSProperties;
  onFocus: (index: number) => void;
  children: ReactNode;
};

function Pane({ index, focused, focusTick, style, onFocus, children }: PaneProps) {
  const ref = useRef<HTMLDivElement>(null);
  // 초점 패널에 터미널이 없으면 패널이 초점을 가져, 친 글자가 이전 터미널로 가지 않게 한다.
  useEffect(() => {
    if (focused) landFocus(ref.current!);
  }, [focused, focusTick]);
  return (
    <div
      ref={ref}
      className={`pane ${focused ? "focused" : ""}`}
      data-pane={index}
      style={style}
      tabIndex={-1}
      role="group"
      aria-label={`패널 ${index + 1}`}
      // 머리글이나 여백을 누르면 브라우저가 패널에 초점을 두므로 터미널로 넘긴다.
      onClick={(e) => {
        if (!(e.target as Element).closest("button, input, .xterm")) landFocus(ref.current!);
      }}
      onPointerDownCapture={() => onFocus(index)}
      onFocusCapture={() => onFocus(index)}
    >
      {children}
    </div>
  );
}

type Props = {
  view: View;
  maximized: boolean;
  /** 같은 패널을 다시 고를 때도 초점을 옮기도록 바뀌는 값. */
  focusTick: number;
  drag?: DragState;
  onFocus: (index: number) => void;
  onResize: (path: string, ratio: number) => void;
  children: (session: string | null, focused: boolean) => ReactNode;
};

export function PaneGrid({ view, maximized, focusTick, drag, ...props }: Props) {
  const grid = useRef<HTMLDivElement>(null);
  // 끌고 있는 경계선과 그동안의 비율. 놓을 때만 저장한다.
  const [held, setHeld] = useState<string>();
  const [live, setLive] = useState<{ path: string; ratio: number }>();
  const holding = useRef<string>(undefined);
  const moved = useRef<{ path: string; ratio: number }>(undefined);
  const tree = live ? resize(view.tree, live.path, live.ratio) : view.tree;
  const { panes, dividers } = geometry(tree);
  const sessions = leaves(tree);
  const shown = sessions
    .map((session, index) => ({ key: session ?? `empty-${index}`, session, index }))
    .filter((p) => !maximized || p.index === view.focus)
    // DOM 순서를 고정해, 배치를 바꿔도 터미널 요소를 옮기지 않는다.
    .sort((a, b) => a.key.localeCompare(b.key));
  const target = drag?.target;
  const swap = target?.zone === "center" && sessions[target.pane] !== null && sessions.includes(drag!.session);

  const stop = () => {
    holding.current = undefined;
    moved.current = undefined;
    setHeld(undefined);
    setLive(undefined);
  };
  // 끄는 중에 단축키로 배치가 바뀌면 경계선이 사라져 놓기 알림이 오지 않는다.
  useEffect(stop, [view.tree, maximized]);
  // Esc는 끌기만 취소하고 셸(에이전트 중단)로 보내지 않는다.
  useEffect(() => {
    if (held === undefined) return;
    const cancel = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      stop();
    };
    window.addEventListener("keydown", cancel, true);
    return () => window.removeEventListener("keydown", cancel, true);
  }, [held]);
  const size = (d: Divider) => {
    const outer = grid.current!.getBoundingClientRect();
    return d.dir === "row" ? outer.width * d.area.w : outer.height * d.area.h;
  };
  const clamp = (d: Divider, ratio: number) => {
    const [low, high] = ratioRange(view.tree, d.path, size(d), MIN_PANE[d.dir]);
    return Math.min(high, Math.max(low, ratio));
  };
  const dragDivider = (d: Divider) => ({
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      if (e.button !== 0) return;
      e.preventDefault();
      e.currentTarget.setPointerCapture(e.pointerId);
      holding.current = d.path;
      setHeld(d.path);
    },
    onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
      if (holding.current !== d.path) return;
      const outer = grid.current!.getBoundingClientRect();
      const ratio =
        d.dir === "row"
          ? ((e.clientX - outer.left) / outer.width - d.area.x) / d.area.w
          : ((e.clientY - outer.top) / outer.height - d.area.y) / d.area.h;
      moved.current = { path: d.path, ratio: clamp(d, ratio) };
      setLive(moved.current);
    },
    onPointerUp: () => {
      if (moved.current) props.onResize(moved.current.path, moved.current.ratio);
      stop();
    },
    onPointerCancel: stop,
    onLostPointerCapture: stop,
    onDoubleClick: () => props.onResize(d.path, 0.5),
    onKeyDown: (e: KeyboardEvent) => {
      const [less, more] = d.dir === "row" ? ["ArrowLeft", "ArrowRight"] : ["ArrowUp", "ArrowDown"];
      if (e.key !== less && e.key !== more) return;
      e.preventDefault();
      props.onResize(d.path, clamp(d, d.ratio + (e.key === more ? KEY_STEP : -KEY_STEP)));
    },
  });

  return (
    <div ref={grid} className={`grid ${sessions.length > 1 && !maximized ? "split" : ""}`}>
      {/* 경계선을 패널보다 앞에 두어, 터미널이 Tab을 가져가도 키보드로 닿게 한다. */}
      {!maximized &&
        dividers.map((d, i) => (
          <div
            key={d.path}
            className={`divider ${d.dir} ${held === d.path ? "held" : ""}`}
            style={dividerBox(d)}
            role="separator"
            aria-label={`패널 크기 조절 ${i + 1}`}
            aria-orientation={d.dir === "row" ? "vertical" : "horizontal"}
            aria-valuenow={Math.round(d.ratio * 100)}
            aria-valuemin={Math.round(MIN_RATIO * 100)}
            aria-valuemax={Math.round(MAX_RATIO * 100)}
            tabIndex={0}
            {...dragDivider(d)}
          >
            {held === d.path && (
              <span className="divider-tip">
                {Math.round(d.ratio * 100)} : {100 - Math.round(d.ratio * 100)} · 더블클릭하면 반반
              </span>
            )}
          </div>
        ))}
      {shown.map(({ key, session, index }) => (
        <Pane
          key={key}
          index={index}
          focused={index === view.focus}
          focusTick={focusTick}
          style={maximized ? { inset: 0 } : box(panes[index])}
          onFocus={props.onFocus}
        >
          {props.children(session, index === view.focus)}
          {target?.pane === index && (
            <div className={`drop-preview ${target.zone}`}>
              {swap ? "여기 놓으면 자리를 바꿔요" : DROP_TEXT[target.zone]}
            </div>
          )}
        </Pane>
      ))}
    </div>
  );
}
