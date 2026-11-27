import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

/** WebView2는 최소화해도 visibilityState가 바뀌지 않아 창 상태를 직접 확인한다. */
export function useMinimized() {
  const [minimized, setMinimized] = useState(false);
  useEffect(() => {
    const win = getCurrentWindow();
    const check = () => void win.isMinimized().then(setMinimized);
    // 최소화와 복원은 모두 크기 변경 이벤트로 온다.
    const unlisten = win.onResized(check);
    check();
    return () => void unlisten.then((off) => off());
  }, []);
  return minimized;
}
