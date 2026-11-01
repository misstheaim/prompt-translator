import { useId, useMemo, useRef, useState } from "react";

import { cx } from "../lib/cx";
import { languageName, useLanguages } from "../lib/useLanguages";

interface LanguageSelectProps {
  label: string;
  /** ISO 639-1 code. */
  value: string;
  onChange: (code: string) => void;
  error?: string;
  disabled?: boolean;
  /** `sm`: the smaller compact-mode size (narrower field, shorter list). */
  size?: "sm" | "md";
}

/** A searchable language picker: type to filter the English names, pick with click or Enter. */
export function LanguageSelect({
  label,
  value,
  onChange,
  error,
  disabled,
  size = "md",
}: LanguageSelectProps) {
  const small = size === "sm";
  const languages = useLanguages();
  const id = useId();
  const listId = `${id}-list`;
  const noteId = `${id}-note`;
  const [query, setQuery] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const blurTimer = useRef<number | undefined>(undefined);

  const open = query !== null;
  const matches = useMemo(() => {
    const q = (query ?? "").trim().toLowerCase();
    if (q === "") return languages;
    return languages.filter((l) => l.name.toLowerCase().includes(q) || l.code.toLowerCase() === q);
  }, [languages, query]);

  function choose(code: string) {
    onChange(code);
    setQuery(null);
  }

  return (
    <div className="relative flex flex-col gap-1">
      <label htmlFor={id} className="text-xs font-medium text-text">
        {label}
      </label>
      <input
        id={id}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? noteId : undefined}
        autoComplete="off"
        disabled={disabled}
        value={open ? query : languageName(languages, value)}
        onFocus={(e) => {
          window.clearTimeout(blurTimer.current);
          e.currentTarget.select();
          setQuery("");
          setActive(0);
        }}
        onBlur={() => {
          // Let a click on an option land before the list closes.
          blurTimer.current = window.setTimeout(() => setQuery(null), 120);
        }}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            if (!open) setQuery("");
            setActive((a) => Math.min(a + 1, matches.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((a) => Math.max(a - 1, 0));
          } else if (e.key === "Enter" && open) {
            e.preventDefault();
            e.stopPropagation();
            const pick = matches[active];
            if (pick) choose(pick.code);
          } else if (e.key === "Escape" && open) {
            e.stopPropagation();
            setQuery(null);
          }
        }}
        className={cx(
          "rounded-md border bg-surface text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50",
          small ? "h-6 w-40 px-1.5 text-xs" : "h-8 w-48 px-2 text-sm",
          error ? "border-danger" : "border-border",
        )}
      />
      {open && (
        <ul
          id={listId}
          role="listbox"
          aria-label={label}
          className={cx(
            "absolute left-0 top-full z-20 mt-1 overflow-auto rounded-md border border-border bg-surface py-1 shadow-lg",
            small ? "max-h-36 w-40" : "max-h-56 w-48",
          )}
        >
          {matches.length === 0 && (
            <li className={cx("px-2 py-1 text-muted", small ? "text-xs" : "text-sm")}>No match</li>
          )}
          {matches.map((l, i) => (
            <li
              key={l.code}
              role="option"
              aria-selected={l.code === value}
              ref={(el) => {
                if (el && i === active) el.scrollIntoView({ block: "nearest" });
              }}
              onMouseDown={(e) => {
                e.preventDefault();
                choose(l.code);
              }}
              onMouseEnter={() => setActive(i)}
              className={cx(
                "cursor-pointer px-2 py-1 text-text",
                small ? "text-xs" : "text-sm",
                i === active && "bg-accent/20",
                l.code === value && "font-medium",
              )}
            >
              {l.name}
            </li>
          ))}
        </ul>
      )}
      {error && (
        <p id={noteId} className="text-xs text-danger">
          {error}
        </p>
      )}
    </div>
  );
}
