import { useEffect, useMemo, useState } from "react";

import { historyTargetLanguages } from "../lib/api";
import { languageName, useLanguages } from "../lib/useLanguages";

interface TargetLanguageFilterProps {
  /** ISO 639-1 code of the results' language; null for "All languages". */
  value: string | null;
  onChange: (code: string | null) => void;
}

/**
 * The language filter of History and Mistakes: "All languages" plus every target language that
 * occurs in the history, by English name.
 */
export function TargetLanguageFilter({ value, onChange }: TargetLanguageFilterProps) {
  const languages = useLanguages();
  const [codes, setCodes] = useState<string[]>([]);

  useEffect(() => {
    let live = true;
    historyTargetLanguages()
      .then((list) => {
        if (live) setCodes(list);
      })
      // Without the list the filter only offers "All languages" (and the current choice).
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  const options = useMemo(() => {
    // Keep the current choice even when its entries are gone, so the select never goes blank.
    const all = value !== null && !codes.includes(value) ? [...codes, value] : codes;
    return all
      .map((code) => ({ code, name: languageName(languages, code) }))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [codes, languages, value]);

  return (
    <select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
      aria-label="Filter by language"
      className="h-8 rounded-md border border-border bg-surface px-2 text-sm text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      <option value="">All languages</option>
      {options.map((o) => (
        <option key={o.code} value={o.code}>
          {o.name}
        </option>
      ))}
    </select>
  );
}
