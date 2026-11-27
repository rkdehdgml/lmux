import { type FormEvent, type ReactNode, useEffect, useId, useRef, useState } from "react";
import { CloseIcon } from "./icons";

type Props = {
  title: string;
  description?: ReactNode;
  submit: string;
  danger?: boolean;
  /** 오류를 던지면 창을 닫지 않고 보여 준다. */
  onSubmit: () => Promise<void>;
  onClose: () => void;
  children?: ReactNode;
};

/** 포커스 가두기와 Esc 닫기는 `<dialog>`가 처리한다. 처음 포커스는 `data-autofocus`로 정한다. */
export function Dialog(props: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    const dialog = ref.current!;
    if (!dialog.open) dialog.showModal();
    // showModal은 첫 단추로 포커스를 옮기고 React의 autoFocus는 그 전에 실행된다.
    dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    return () => dialog.close();
  }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      await props.onSubmit();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <dialog
      ref={ref}
      className="dialog"
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) props.onClose();
      }}
    >
      <form onSubmit={submit}>
        <div className="dialog-head">
          <div>
            <h2 id={titleId}>{props.title}</h2>
            {props.description && <p className="muted">{props.description}</p>}
          </div>
          <button
            type="button"
            className="btn ghost icon-btn"
            aria-label="닫기"
            onClick={props.onClose}
            disabled={busy}
          >
            <CloseIcon />
          </button>
        </div>
        {props.children && <div className="dialog-body">{props.children}</div>}
        {error && (
          <div className="dialog-error" role="alert">
            {error}
          </div>
        )}
        <div className="dialog-foot">
          <button type="button" className="btn" onClick={props.onClose} disabled={busy}>
            취소
          </button>
          <button type="submit" className={`btn ${props.danger ? "danger" : "primary"}`} disabled={busy}>
            {props.submit}
          </button>
        </div>
      </form>
    </dialog>
  );
}
