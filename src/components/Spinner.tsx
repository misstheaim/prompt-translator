import { cx } from "../lib/cx";

interface SpinnerProps {
  size?: "sm" | "md";
  className?: string;
}

/** 16 / 24 px accent ring; static (no rotation) under `prefers-reduced-motion`. */
export function Spinner({ size = "sm", className }: SpinnerProps) {
  return (
    <span
      role="status"
      aria-label="Loading"
      className={cx(
        "inline-block shrink-0 rounded-full border-2 border-accent border-t-transparent motion-safe:animate-spin",
        size === "sm" ? "size-4" : "size-6",
        className,
      )}
    />
  );
}
