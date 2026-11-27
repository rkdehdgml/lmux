import { type PointerEvent as ReactPointerEvent, useRef, useState } from "react";
import { dropZone, leaves, type Node, type Zone, zoneAt } from "./layout";

export type DropTarget = { pane: number; zone: Zone };
export type DragState = { session: string; target?: DropTarget };

/** 이만큼 움직여야 클릭이 아니라 끌기로 본다. */
const THRESHOLD = 6;

/** 끌기가 끝난 뒤 같은 요소에서 클릭이 이어서 실행되지 않게 한다. */
function swallowClick() {
  const stop = (e: Event) => {
    e.preventDefault();
    e.stopPropagation();
  };
  window.addEventListener("click", stop, true);
  setTimeout(() => window.removeEventListener("click", stop, true));
}

/**
 * 세션을 패널로 끌어다 놓는다. HTML 끌어다 놓기 대신 포인터 이벤트를 써서 Tauri의 파일 끌기 설정과
 * 터미널의 끌기 처리에 기대지 않는다. Esc나 창 전환으로 취소한다.
 */
export function useSessionDrag(
  tree: Node,
  onDrop: (session: string, target: DropTarget) => void,
  /** 끌었지만 아무것도 바꾸지 않고 끝났을 때. */
  onCancel: () => void,
) {
  const [drag, setDrag] = useState<DragState>();
  // 끄는 동안 등록한 함수가 최신 값을 보게 한다.
  const latest = useRef({ tree, onDrop, onCancel });
  latest.current = { tree, onDrop, onCancel };

  const begin = (session: string) => (down: ReactPointerEvent) => {
    // 머리글 안의 단추는 끌기 손잡이가 아니다.
    const button = (down.target as Element).closest("button");
    if (down.button !== 0 || (button && button !== down.currentTarget)) return;
    let moving = false;
    let cancelled = false;

    const find = (x: number, y: number): DropTarget | undefined => {
      const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-pane]");
      if (!el) return;
      const pane = Number(el.dataset.pane);
      const now = latest.current.tree;
      // 자기 자리에 놓으면 아무것도 바뀌지 않는다.
      if (leaves(now)[pane] === session) return;
      const box = el.getBoundingClientRect();
      const zone = zoneAt((x - box.left) / box.width, (y - box.top) / box.height);
      return { pane, zone: dropZone(now, session, pane, zone) };
    };
    const move = (e: PointerEvent) => {
      if (e.pointerId !== down.pointerId) return;
      // 놓기 알림을 못 받은 경우.
      if (e.buttons === 0) {
        cancel();
        return end();
      }
      if (cancelled) return;
      if (!moving && Math.hypot(e.clientX - down.clientX, e.clientY - down.clientY) < THRESHOLD) {
        return;
      }
      moving = true;
      document.body.classList.add("dragging");
      setDrag({ session, target: find(e.clientX, e.clientY) });
    };
    const up = (e: PointerEvent) => {
      if (e.pointerId !== down.pointerId) return;
      end();
      if (!moving) return;
      // 끌었다면 놓으면서 생기는 클릭은 버린다.
      swallowClick();
      if (cancelled) return;
      const at = find(e.clientX, e.clientY);
      if (at) latest.current.onDrop(session, at);
      else latest.current.onCancel();
    };
    // 단추를 놓을 때까지 지켜본다. 그래야 뒤따르는 클릭을 버릴 수 있다.
    const cancel = () => {
      if (!moving) return end();
      if (cancelled) return;
      cancelled = true;
      document.body.classList.remove("dragging");
      setDrag(undefined);
      latest.current.onCancel();
    };
    const lost = (e: PointerEvent) => {
      if (e.pointerId !== down.pointerId) return;
      cancel();
      end();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !moving || cancelled) return;
      e.preventDefault();
      e.stopPropagation();
      cancel();
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", lost);
      window.removeEventListener("blur", cancel);
      window.removeEventListener("keydown", key, true);
      document.body.classList.remove("dragging");
      setDrag(undefined);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", lost);
    window.addEventListener("blur", cancel);
    window.addEventListener("keydown", key, true);
  };

  return { drag, begin };
}
