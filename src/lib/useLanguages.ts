import { useEffect, useState } from "react";

import type { Language } from "../bindings/Language";
import { listLanguages } from "./api";

let cache: Language[] | null = null;
let pending: Promise<Language[]> | null = null;

function load(): Promise<Language[]> {
  pending ??= listLanguages()
    .then((list) => {
      cache = list;
      return list;
    })
    .finally(() => {
      pending = null;
    });
  return pending;
}

/** The supported languages (`list_languages`), fetched once per window. Empty until loaded. */
export function useLanguages(): Language[] {
  const [languages, setLanguages] = useState<Language[]>(cache ?? []);
  useEffect(() => {
    if (cache) return;
    let live = true;
    load()
      .then((list) => {
        if (live) setLanguages(list);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  return languages;
}

/** English name for a code, falling back to the code itself. */
export function languageName(languages: Language[], code: string): string {
  return languages.find((l) => l.code === code)?.name ?? code;
}
