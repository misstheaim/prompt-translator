import { useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";

import { cx } from "../lib/cx";

interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  mono?: boolean;
  /** Auto-grow stops at this height in px; the area scrolls beyond it. */
  maxHeight?: number;
  /** Compact mode: a smaller font, and the area fills its parent instead of auto-growing. */
  compact?: boolean;
}

/** Auto-growing text area, at least 4 rows. */
export function TextArea({
  mono,
  compact,
  maxHeight = 320,
  className,
  value,
  rows = 4,
  ...rest
}: TextAreaProps) {
  const ref = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (compact) {
      el.style.height = "";
      return;
    }
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`;
  }, [value, maxHeight, compact]);

  return (
    <textarea
      ref={ref}
      rows={rows}
      value={value}
      className={cx(
        "w-full resize-none rounded-md border border-border bg-surface text-text",
        compact ? "h-full px-2 py-1 text-xs" : "px-3 py-2 text-sm",
        " placeholder:text-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent read-only:bg-surface-raised",
        mono ? "font-mono" : "font-sans",
        className,
      )}
      {...rest}
    />
  );
}
