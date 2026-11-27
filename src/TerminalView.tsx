import { Channel, invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef, useState } from "react";
import { landFocus } from "./focus";
import { clipboardAction, isTerminalReply } from "./keys";
import { latestOnly } from "./serial";

const FONT = '"JetBrains Mono Variable", "D2Coding", "Malgun Gothic", monospace';
const ACK_EVERY = 256 * 1024;
// 화면(viewer) 번호는 웹뷰를 새로 고쳐도 겹치지 않아야 한다.
let viewerSeq = Date.now();

type Size = { cols: number; rows: number };
const resizers = new Map<string, (size: Size) => Promise<void>>();

/** 늦게 도착한 이전 크기가 적용되지 않도록 세션마다 한 번에 하나씩 보낸다. */
function resizer(id: string) {
  let resize = resizers.get(id);
  if (!resize) {
    resize = latestOnly((size: Size) => invoke("session_resize", { id, ...size }));
    resizers.set(id, resize);
  }
  return resize;
}

type Props = {
  sessionId: string;
  /** 보고 있는 실행 번호. 다시 시작하면 컴포넌트를 새로 만든다. */
  run: number;
  osBuild: number;
  /** 초점 패널이면 붙자마자 키 입력을 받는다. 그 뒤의 초점 이동은 패널이 맡는다. */
  focused: boolean;
  /** 붙었을 때 이미 끝난 세션이면 알려 준다(웹뷰를 새로 고친 경우). */
  onExited?: (run: number, code: number) => void;
};

/** 보이는 세션에만 마운트한다. 해제하면 출력은 Rust 링버퍼에만 쌓이고, 다시 마운트하면 그린다. */
export function TerminalView({ sessionId, run, osBuild, onExited, ...props }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const focused = useRef(props.focused);
  focused.current = props.focused;
  const [attempt, setAttempt] = useState(0);
  const [notice, setNotice] = useState<string>();
  // 크기 오류는 크기 맞추기가 성공해야 지운다. 입력 성공으로는 지우지 않는다.
  const [resizeNotice, setResizeNotice] = useState<string>();

  useEffect(() => {
    const el = host.current!;
    const viewer = ++viewerSeq;
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let frame = 0;
    const report = (error: unknown) => {
      if (!disposed) setNotice(String(error));
    };

    const term = new Terminal({
      fontFamily: FONT,
      fontSize: 13,
      lineHeight: 1.25,
      cursorBlink: true,
      scrollback: 10_000,
      windowsPty: { backend: "conpty", buildNumber: osBuild },
      theme: {
        background: "#0B0D10",
        foreground: "#D5D9E0",
        cursor: "#D5D9E0",
        selectionBackground: "#2F6FE466",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.attachCustomKeyEventHandler((e) => {
      const action = clipboardAction(e, term.hasSelection());
      if (action === "copy") {
        e.preventDefault();
        // 복사에 실패하면 선택을 남겨 다시 시도할 수 있게 한다.
        navigator.clipboard
          .writeText(term.getSelection())
          .then(() => term.clearSelection())
          .catch(() => report("클립보드에 복사하지 못했어요"));
      }
      // false면 xterm이 키를 처리하지 않는다. 붙여넣기는 브라우저 paste 이벤트가 이어서 처리한다.
      return action === null;
    });

    const start = async () => {
      await document.fonts.load(`13px "JetBrains Mono Variable"`).catch(() => {});
      if (disposed) return;
      term.open(el);
      try {
        const webgl = new WebglAddon();
        webgl.onContextLoss(() => webgl.dispose());
        term.loadAddon(webgl);
      } catch (error) {
        console.warn("WebGL을 쓸 수 없어 기본 렌더러를 씁니다", error);
      }
      fit.fit();
      const sendSize = resizer(sessionId);
      const resize = (size: Size) =>
        sendSize(size).then(
          () => setResizeNotice(undefined),
          (error) => {
            if (!disposed) setResizeNotice(String(error));
          },
        );
      // 크기 맞추기에 실패해도 출력은 이어서 받는다.
      await resize({ cols: term.cols, rows: term.rows });

      // 과거 출력을 다시 그리는 동안 xterm이 만드는 질의 응답은 셸에 보내지 않는다.
      let replaying = true;
      let replayReceived = false;
      let unacked = 0;
      const ack = () => {
        if (disposed || unacked === 0) return;
        void invoke("session_ack", { id: sessionId, viewer, bytes: unacked });
        unacked = 0;
      };
      const channel = new Channel<ArrayBuffer>();
      channel.onmessage = (message) => {
        if (disposed) return;
        const data = new Uint8Array(message);
        if (data.length === 0) {
          if (replayReceived) {
            // 응답이 늦어 백엔드가 연결을 끊었다. 처음부터 다시 붙는다.
            setAttempt((n) => n + 1);
            return;
          }
          replayReceived = true;
          term.write("", () => {
            replaying = false;
            ack();
          });
          return;
        }
        term.write(data, () => {
          unacked += data.length;
          if (unacked >= ACK_EVERY) ack();
        });
      };
      // 입력 순서가 바뀌지 않도록 앞의 전송이 끝난 뒤 다음을 보낸다.
      let sending: Promise<void> = Promise.resolve();
      const send = (data: string, binary: boolean) => {
        if (replaying && isTerminalReply(data)) return;
        sending = sending
          .then(() => invoke("session_write", { id: sessionId, run, viewer, data, binary }))
          .then(() => setNotice(undefined), report);
      };
      term.onData((data) => send(data, false));
      term.onBinary((data) => send(data, true));
      term.onResize(({ cols, rows }) => void resize({ cols, rows }));
      const info = await invoke<{ run: number; exit_code: number | null }>("session_attach", {
        id: sessionId,
        viewer,
        onOutput: channel,
      });
      if (disposed) {
        void invoke("session_detach", { id: sessionId, viewer });
        return;
      }
      if (info.exit_code !== null) onExited?.(info.run, info.exit_code);
      observer = new ResizeObserver(() => {
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(() => fit.fit());
      });
      observer.observe(el);
      fit.fit();
      const pane = el.closest<HTMLElement>("[data-pane]");
      if (focused.current && pane) landFocus(pane, true);
    };
    start().catch((error) => report(`터미널 연결 실패: ${error}`));

    return () => {
      disposed = true;
      observer?.disconnect();
      cancelAnimationFrame(frame);
      void invoke("session_detach", { id: sessionId, viewer });
      term.dispose();
    };
    // onExited가 바뀌어도 터미널을 다시 만들지 않는다.
  }, [sessionId, run, osBuild, attempt]);

  return (
    <div className="term-box">
      <div className="term-host" ref={host} />
      {(notice ?? resizeNotice) && (
        <div className="term-notice" role="alert">
          {notice ?? resizeNotice}
        </div>
      )}
    </div>
  );
}
