import { cx } from "../lib/cx";
import { useAppStore } from "../store/app";

/** Icon button that turns the Translate screen's compact mode on and off (TRANS-015). */
export function CompactToggle({ className }: { className?: string }) {
  const compact = useAppStore((s) => s.compact !== null);
  const label = compact ? "Leave compact mode" : "Compact mode";

  function toggle() {
    const { enterCompact, exitCompact } = useAppStore.getState();
    void (compact ? exitCompact() : enterCompact("button"));
  }

  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={compact}
      title={label}
      onClick={toggle}
      className={cx(
        "flex size-6 items-center justify-center rounded text-muted transition-colors hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent motion-reduce:transition-none",
        className,
      )}
    >
      <svg
        viewBox="0 0 16 16"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {compact ? (
          // Expand: arrows pointing out.
          <path d="M10 2h4v4M6 14H2v-4M14 2l-5 5M2 14l5-5" />
        ) : (
          // Compact: arrows pointing in.
          <path d="M14 6h-4V2M2 10h4v4M10 6l4-4M6 10l-4 4" />
        )}
      </svg>
    </button>
  );
}
