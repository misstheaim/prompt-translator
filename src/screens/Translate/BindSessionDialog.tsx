import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import type { SessionTarget } from "../../bindings/SessionTarget";
import { Button } from "../../components/Button";
import { Spinner } from "../../components/Spinner";
import { bindTarget, bindTerminalByFocus, listSessionTargets, toAppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { useSessionTabs } from "./sessionTabs";

const FOCUS_WAIT_MS = 10_000;

const HOST_LABELS = { windowsTerminal: "Windows Terminal", conhost: "Console window" } as const;

/** Lists the terminal windows that can be bound, or binds the one the user clicks into. */
export function BindSessionDialog({ onClose }: { onClose: () => void }) {
  const showToast = useAppStore((s) => s.showToast);
  const [targets, setTargets] = useState<SessionTarget[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Set while a bind is running: the label of what is being waited for.
  const [busy, setBusy] = useState<string | null>(null);
  const [secondsLeft, setSecondsLeft] = useState(0);
  const closeRef = useRef<HTMLButtonElement>(null);
  const previousFocus = useRef<Element | null>(null);

  useEffect(() => {
    previousFocus.current = document.activeElement;
    closeRef.current?.focus();
    return () => {
      if (previousFocus.current instanceof HTMLElement) previousFocus.current.focus();
    };
  }, []);

  useEffect(() => {
    let alive = true;
    listSessionTargets().then(
      (list) => alive && setTargets(list),
      (e) => alive && setLoadError(toAppError(e).message),
    );
    return () => {
      alive = false;
    };
  }, []);

  // The countdown while waiting for a click into a terminal.
  useEffect(() => {
    if (secondsLeft <= 0) return;
    const t = window.setTimeout(() => setSecondsLeft((n) => n - 1), 1000);
    return () => window.clearTimeout(t);
  }, [secondsLeft]);

  async function bind(label: string, run: () => Promise<SessionTarget>, countdown: boolean) {
    setBusy(label);
    if (countdown) setSecondsLeft(Math.ceil(FOCUS_WAIT_MS / 1000));
    try {
      const target = await run();
      useSessionTabs.getState().addBound(target);
      onClose();
    } catch (e) {
      showToast("error", toAppError(e).message);
      setBusy(null);
      setSecondsLeft(0);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && !busy) {
      e.stopPropagation();
      onClose();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onKeyDown={onKeyDown}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="bind-title"
        className="flex max-h-full w-full max-w-md flex-col rounded-lg border border-border bg-surface p-4 shadow-lg"
      >
        <h2 id="bind-title" className="text-sm font-semibold text-text">
          Bind a Claude session
        </h2>
        <p className="mt-1 text-xs text-muted">
          Pick a terminal window running Claude Code. Accepted text is pasted into its input; you
          press Enter there.
        </p>

        <div className="mt-3 min-h-0 flex-1 overflow-auto">
          {loadError ? (
            <p className="text-sm text-danger">{loadError}</p>
          ) : targets === null ? (
            <div className="flex items-center gap-2 text-sm text-muted">
              <Spinner /> Looking for terminal windows
            </div>
          ) : targets.length === 0 ? (
            <p className="text-sm text-muted">No terminal windows found.</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {targets.map((t) => (
                <li key={`${t.hwnd}:${t.tabRuntimeId ?? ""}`}>
                  <button
                    type="button"
                    disabled={busy !== null}
                    title={t.title}
                    onClick={() => void bind(t.title, () => bindTarget(t), false)}
                    className="flex w-full flex-col rounded-md border border-border px-3 py-1.5 text-left hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <span className="truncate text-sm text-text">{t.title}</span>
                    <span className="text-xs text-muted">{HOST_LABELS[t.host]}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="mt-4 flex items-center justify-between gap-2">
          <Button
            disabled={busy !== null}
            loading={busy === "focus"}
            onClick={() => void bind("focus", () => bindTerminalByFocus(FOCUS_WAIT_MS), true)}
          >
            {busy === "focus"
              ? `Click into a terminal (${secondsLeft} s)`
              : "Click into a terminal…"}
          </Button>
          <Button ref={closeRef} disabled={busy !== null} onClick={onClose}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}
