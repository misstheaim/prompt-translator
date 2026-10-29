import { useEffect, useRef, type KeyboardEvent } from "react";

import { Button } from "./Button";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Destructive confirm uses the danger button, and focus starts on Cancel. */
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Modal over a scrim: Esc cancels, Tab stays inside, Enter activates the focused button. */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const previousFocus = useRef<Element | null>(null);

  useEffect(() => {
    if (!open) return;
    previousFocus.current = document.activeElement;
    (destructive ? cancelRef : confirmRef).current?.focus();
    return () => {
      if (previousFocus.current instanceof HTMLElement) previousFocus.current.focus();
    };
  }, [open, destructive]);

  if (!open) return null;

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onCancel();
    } else if (e.key === "Tab") {
      // Focus trap: two buttons, so Tab just toggles between them.
      e.preventDefault();
      const next = document.activeElement === cancelRef.current ? confirmRef : cancelRef;
      next.current?.focus();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onKeyDown={onKeyDown}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-message"
        className="w-full max-w-sm rounded-lg border border-border bg-surface p-4 shadow-lg"
      >
        <h2 id="confirm-title" className="text-sm font-semibold text-text">
          {title}
        </h2>
        <p id="confirm-message" className="mt-2 text-sm text-muted">
          {message}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button ref={cancelRef} onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button ref={confirmRef} variant={destructive ? "danger" : "primary"} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
