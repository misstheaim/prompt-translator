import type { ReactNode } from "react";

import { cx } from "../lib/cx";

export type BannerVariant = "info" | "success" | "warning" | "danger";

interface BannerProps {
  variant?: BannerVariant;
  children: ReactNode;
  /** Buttons shown on the right (Retry, Open settings, ...). */
  actions?: ReactNode;
  className?: string;
}

const STYLES: Record<BannerVariant, { box: string; icon: string; glyph: string }> = {
  info: { box: "border-accent bg-accent/10", icon: "text-accent", glyph: "i" },
  success: { box: "border-success bg-success/10", icon: "text-success", glyph: "✓" },
  warning: { box: "border-warning bg-warning/10", icon: "text-warning", glyph: "!" },
  danger: { box: "border-danger bg-danger/10", icon: "text-danger", glyph: "!" },
};

/** Left-bordered, tinted message box. `role="alert"` for warning/danger, `"status"` otherwise. */
export function Banner({ variant = "info", children, actions, className }: BannerProps) {
  const s = STYLES[variant];
  return (
    <div
      role={variant === "warning" || variant === "danger" ? "alert" : "status"}
      className={cx("flex items-start gap-3 rounded-md border-l-4 px-3 py-2", s.box, className)}
    >
      <span
        aria-hidden="true"
        className={cx(
          "mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border border-current text-[10px] font-bold leading-none",
          s.icon,
        )}
      >
        {s.glyph}
      </span>
      <div className="min-w-0 flex-1 text-sm text-text">{children}</div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}
