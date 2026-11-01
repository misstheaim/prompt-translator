import { useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";

import { cx } from "../lib/cx";

interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  mono?: boolean;
  /** Auto-grow stops at this height in px; the area scrolls beyond it. */
  maxHeight?: number;
  /** Compact mode: a smaller font, and the area fills its parent instead of auto-growing. */
  compact?: boolean;
  /**
   * The parent draws the border, background and focus ring (a box with a footer strip under the
   * text, #199); the area itself has none.
   */
  framed?: boolean;
}

/** Auto-growing text area, at least 4 rows. */
export function TextArea({
  mono,
  compact,
  framed,
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
        "w-full resize-none text-text placeholder:text-muted",
        framed
          ? "bg-transparent focus-visible:outline-none"
          : "rounded-md border border-border bg-surface focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent read-only:bg-surface-raised",
        compact ? "h-full px-2 py-1 text-xs" : "px-3 py-2 text-sm",
        mono ? "font-mono" : "font-sans",
        className,
      )}
      {...rest}
    />
  );
}
