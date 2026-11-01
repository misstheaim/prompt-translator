import { useRef, type KeyboardEvent } from "react";

import { cx } from "../lib/cx";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  /** This option can't be chosen; `title` explains why. */
  disabled?: boolean;
  title?: string;
}

interface SegmentedControlProps<T extends string> {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  "aria-label"?: string;
  disabled?: boolean;
  /** `sm`: the smaller compact-mode size. */
  size?: "sm" | "md";
}

/** Radio-group style switch; Left/Right arrows move the selection. */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  disabled,
  size = "md",
  "aria-label": ariaLabel,
}: SegmentedControlProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  function onKeyDown(e: KeyboardEvent, index: number) {
    const step =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? -1
          : 0;
    if (step === 0) return;
    e.preventDefault();
    let next = index;
    for (let i = 0; i < options.length; i++) {
      next = (next + step + options.length) % options.length;
      if (!options[next].disabled) break;
    }
    if (options[next].disabled) return;
    onChange(options[next].value);
    refs.current[next]?.focus();
  }

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className="inline-flex gap-0.5 rounded-md bg-surface-raised p-0.5"
    >
      {options.map((o, i) => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            disabled={disabled || o.disabled}
            title={o.title}
            onClick={() => onChange(o.value)}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={cx(
              "rounded font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50 motion-reduce:transition-none",
              size === "sm" ? "h-5 px-2 text-xs" : "h-7 px-3 text-sm",
              selected ? "bg-surface text-accent shadow-sm" : "text-muted hover:text-text",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
