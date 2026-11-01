import { useEffect, useRef, useState } from "react";

import { LanguageSelect } from "../../components/LanguageSelect";
import { cx } from "../../lib/cx";
import { languageName, useLanguages } from "../../lib/useLanguages";
import type { LanguagePair } from "./store";

interface LanguagePairControlProps {
  pair: LanguagePair;
  onChange: (pair: LanguagePair) => void;
  disabled?: boolean;
  /**
   * `sm`: the smaller compact-mode size. The label may then shrink (with an ellipsis) and the
   * popover opens to the left with smaller selects side by side, so it fits the small window.
   */
  size?: "sm" | "md";
  className?: string;
}

/** "Russian → English" with a swap button; the two selects open in a popover. */
export function LanguagePairControl({
  pair,
  onChange,
  disabled,
  size = "md",
  className,
}: LanguagePairControlProps) {
  const small = size === "sm";
  const languages = useLanguages();
  const [open, setOpen] = useState(false);
  const wrapRef = useRef<HTMLDivElement>(null);
  const same = pair.native === pair.target;

  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const btn = cx(
    "rounded-md border border-border bg-surface text-text hover:bg-border/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50",
    small ? "h-6 px-1.5 text-xs" : "h-8 px-2 text-sm",
  );
  const label = `${languageName(languages, pair.native)} → ${languageName(languages, pair.target)}`;

  return (
    <div
      ref={wrapRef}
      className={cx("relative flex items-center gap-1", small && "min-w-0", className)}
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) setOpen(false);
      }}
    >
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={small ? `${label}: languages for this session` : "Languages for this session"}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className={cx(btn, small && "min-w-0 truncate")}
      >
        {label}
      </button>
      <button
        type="button"
        aria-label="Swap languages"
        title="Swap languages"
        disabled={disabled}
        onClick={() => onChange({ native: pair.target, target: pair.native })}
        className={cx(btn, "shrink-0")}
      >
        ⇄
      </button>
      {open && (
        <div
          role="dialog"
          aria-label="Languages for this session"
          className={cx(
            "absolute top-full z-20 mt-1 flex flex-col rounded-lg border border-border bg-surface shadow-lg",
            small ? "right-0 w-max gap-2 p-2" : "left-0 gap-3 p-3",
          )}
        >
          <div className={cx("flex gap-3", small ? "flex-row items-start" : "flex-col")}>
            <LanguageSelect
              size={size}
              label="From (my language)"
              value={pair.native}
              onChange={(native) => onChange({ ...pair, native })}
            />
            <LanguageSelect
              size={size}
              label="To (language I write prompts in)"
              value={pair.target}
              onChange={(target) => onChange({ ...pair, target })}
              error={same ? "Choose two different languages" : undefined}
            />
          </div>
          <p className="text-xs text-muted">Only for this session; Settings stay unchanged.</p>
        </div>
      )}
    </div>
  );
}
