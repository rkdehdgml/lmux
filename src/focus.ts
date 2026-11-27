/**
 * 패널 안의 단추를 쓰는 중이 아니면 터미널(없으면 패널)로 초점을 옮긴다.
 * 대화 상자가 열려 있으면 옮기지 않는다.
 */
export function landFocus(pane: HTMLElement, onlyIfIdle = false) {
  if (document.querySelector("dialog[open]")) return;
  const active = document.activeElement;
  // 붙는 동안 사용자가 밖(사이드바·레이아웃 바)으로 옮긴 초점은 가져가지 않는다.
  if (onlyIfIdle && active && active !== document.body && !pane.contains(active)) return;
  if (active !== pane && pane.contains(active)) return;
  (pane.querySelector<HTMLElement>(".xterm-helper-textarea") ?? pane).focus({ preventScroll: true });
}
