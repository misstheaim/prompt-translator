import { cx } from "../lib/cx";
import { useAppStore, type ToastKind } from "../store/app";

const STYLES: Record<ToastKind, string> = {
  info: "border-accent",
  success: "border-success",
  error: "border-danger",
};

/** Bottom-center stack of toasts (max 3); the store auto-dismisses them. */
export function ToastHost() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex flex-col items-center gap-2 px-4">
      {toasts.map((t) => (
        <div
          key={t.id}
          role="status"
          className={cx(
            "pointer-events-auto flex max-w-md items-center gap-3 rounded-lg border-l-4 border-y border-r border-y-border border-r-border bg-surface px-3 py-2 text-sm text-text shadow-lg",
            STYLES[t.kind],
          )}
        >
          <span className="min-w-0 flex-1">{t.message}</span>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => dismiss(t.id)}
            className="rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
