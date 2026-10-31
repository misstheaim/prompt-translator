import { useEffect, useRef, useState } from "react";

import { LanguageSelect } from "../../components/LanguageSelect";
import { languageName, useLanguages } from "../../lib/useLanguages";
import type { LanguagePair } from "./store";

interface LanguagePairControlProps {
  pair: LanguagePair;
  onChange: (pair: LanguagePair) => void;
  disabled?: boolean;
}

/** "Russian → English" with a swap button; the two selects open in a popover. */
export function LanguagePairControl({ pair, onChange, disabled }: LanguagePairControlProps) {
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

  const btn =
    "h-8 rounded-md border border-border bg-surface px-2 text-sm text-text hover:bg-border/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50";

  return (
    <div
      ref={wrapRef}
      className="relative flex items-center gap-1"
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) setOpen(false);
      }}
    >
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Languages for this session"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className={btn}
      >
        {languageName(languages, pair.native)} → {languageName(languages, pair.target)}
      </button>
      <button
        type="button"
        aria-label="Swap languages"
        title="Swap languages"
        disabled={disabled}
        onClick={() => onChange({ native: pair.target, target: pair.native })}
        className={btn}
      >
        ⇄
      </button>
      {open && (
        <div
          role="dialog"
          aria-label="Languages for this session"
          className="absolute left-0 top-full z-20 mt-1 flex flex-col gap-3 rounded-lg border border-border bg-surface p-3 shadow-lg"
        >
          <LanguageSelect
            label="From (my language)"
            value={pair.native}
            onChange={(native) => onChange({ ...pair, native })}
          />
          <LanguageSelect
            label="To (language I write prompts in)"
            value={pair.target}
            onChange={(target) => onChange({ ...pair, target })}
            error={same ? "Choose two different languages" : undefined}
          />
          <p className="text-xs text-muted">Only for this session; Settings stay unchanged.</p>
        </div>
      )}
    </div>
  );
}
