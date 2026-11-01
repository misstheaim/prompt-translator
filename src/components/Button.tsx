import type { ComponentProps } from "react";

import { cx } from "../lib/cx";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  /** `xs` is the compact-mode size. */
  size?: "xs" | "sm" | "md";
  loading?: boolean;
  /** Muted suffix showing the keyboard shortcut, e.g. "Ctrl+Enter". */
  shortcut?: string;
}

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-contrast hover:opacity-90",
  secondary: "border border-border bg-surface text-text hover:bg-surface-raised",
  danger: "bg-danger text-accent-contrast hover:opacity-90",
  ghost: "text-text hover:bg-surface-raised",
};

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  shortcut,
  disabled,
  className,
  children,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      className={cx(
        "inline-flex items-center justify-center gap-2 rounded-md font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50 motion-reduce:transition-none",
        size === "xs"
          ? "h-6 px-2 text-xs"
          : size === "sm"
            ? "h-7 px-2 text-xs"
            : "h-8 px-3 text-sm",
        VARIANTS[variant],
        className,
      )}
      {...rest}
    >
      {loading && <Spinner />}
      {children}
      {shortcut && <span className="text-xs font-normal opacity-70">{shortcut}</span>}
    </button>
  );
}
