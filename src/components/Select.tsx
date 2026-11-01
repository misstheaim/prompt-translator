import { useId, type SelectHTMLAttributes } from "react";

import { cx } from "../lib/cx";

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "onChange" | "size"> {
  label?: string;
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  /** `sm`: the smaller compact-mode size; the select may then shrink below its content width. */
  size?: "sm" | "md";
  /**
   * Classes for the wrapper (the label and the select), e.g. its width limits in a flex row.
   * At `sm` they replace the default `min-w-0`.
   */
  wrapClassName?: string;
}

/** Native `<select>` (keeps the OS dropdown for accessibility) with an optional label. */
export function Select({
  label,
  options,
  value,
  onChange,
  className,
  id,
  size = "md",
  wrapClassName,
  ...rest
}: SelectProps) {
  const autoId = useId();
  const selectId = id ?? autoId;
  const small = size === "sm";
  return (
    <div
      className={cx(
        "flex items-center",
        small ? cx("gap-1", wrapClassName ?? "min-w-0") : cx("gap-2", wrapClassName),
      )}
    >
      {label && (
        <label htmlFor={selectId} className="text-xs text-muted">
          {label}
        </label>
      )}
      <select
        id={selectId}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={cx(
          "rounded-md border border-border bg-surface text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50",
          small ? "h-6 min-w-0 max-w-full px-1 text-xs" : "h-8 px-2 text-sm",
          className,
        )}
        {...rest}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}
