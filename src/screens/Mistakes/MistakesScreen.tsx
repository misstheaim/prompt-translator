import { useEffect, useRef, useState } from "react";

import type { IssueCategory } from "../../bindings/IssueCategory";
import type { MistakesSummary } from "../../bindings/MistakesSummary";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Spinner } from "../../components/Spinner";
import { mistakesSummary, toAppError, type AppError } from "../../lib/api";
import { cx } from "../../lib/cx";
import { useAppStore } from "../../store/app";

const PERIODS = [
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
  { label: "All time", days: null },
] as const;

// Full class names so Tailwind can see them.
const CATEGORY_BG: Record<IssueCategory, string> = {
  spelling: "bg-cat-spelling",
  grammar: "bg-cat-grammar",
  article: "bg-cat-article",
  tense: "bg-cat-tense",
  preposition: "bg-cat-preposition",
  word_choice: "bg-cat-word-choice",
  vocabulary: "bg-cat-vocabulary",
  punctuation: "bg-cat-punctuation",
  style: "bg-cat-style",
  other: "bg-cat-other",
};

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
}

function SectionTitle({ children }: { children: string }) {
  return <h2 className="mb-2 text-sm font-semibold text-text">{children}</h2>;
}

export function MistakesScreen() {
  const setScreen = useAppStore((s) => s.setScreen);
  const setHistoryIssue = useAppStore((s) => s.setHistoryIssue);

  const [periodIndex, setPeriodIndex] = useState(1);
  const [summary, setSummary] = useState<MistakesSummary | null>(null);
  const [loadedIndex, setLoadedIndex] = useState<number | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const request = useRef(0);

  useEffect(() => {
    const id = ++request.current;
    mistakesSummary(PERIODS[periodIndex].days)
      .then((s) => {
        if (id !== request.current) return;
        setSummary(s);
        setError(null);
        setLoadedIndex(periodIndex);
      })
      .catch((e: unknown) => {
        if (id !== request.current) return;
        setError(toAppError(e));
        setLoadedIndex(periodIndex);
      });
  }, [periodIndex, reloadKey]);

  const loading = loadedIndex !== periodIndex;
  const empty =
    summary !== null &&
    summary.byCategory.length === 0 &&
    summary.recurring.length === 0 &&
    summary.vocabulary.length === 0;
  const maxCount = Math.max(1, ...(summary?.byCategory.map((c) => c.count) ?? []));

  function openHistory(original: string, corrected: string) {
    setHistoryIssue({ original, corrected });
    setScreen("history");
  }

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-5">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="text-lg font-semibold text-text">Mistakes</h1>
        <div role="group" aria-label="Period" className="ml-auto flex gap-1">
          {PERIODS.map((p, i) => (
            <button
              key={p.label}
              type="button"
              aria-pressed={i === periodIndex}
              onClick={() => setPeriodIndex(i)}
              className={cx(
                "h-8 rounded-md border px-3 text-sm transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent motion-reduce:transition-none",
                i === periodIndex
                  ? "border-accent bg-surface-raised text-accent"
                  : "border-border bg-surface text-muted hover:text-text",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {error && <ErrorBanner error={error} onRetry={() => setReloadKey((k) => k + 1)} />}

      {loading && (
        <div className="flex justify-center py-2">
          <Spinner />
        </div>
      )}

      {empty && !loading && (
        <p className="py-8 text-center text-muted">
          No mistakes recorded for this period. They appear here after Correct and Improve runs,
          which explain what you wrote wrong.
        </p>
      )}

      {summary && !empty && (
        <>
          <section aria-label="By category">
            <SectionTitle>By category</SectionTitle>
            <ul className="flex flex-col gap-1.5">
              {summary.byCategory.map((c) => (
                <li key={c.category} className="flex items-center gap-3 text-sm">
                  <span className="w-24 shrink-0 text-muted">{c.category.replace("_", " ")}</span>
                  <span className="h-3 min-w-0 flex-1 rounded bg-surface-raised">
                    <span
                      className={cx("block h-full rounded", CATEGORY_BG[c.category])}
                      style={{ width: `${(c.count / maxCount) * 100}%` }}
                    />
                  </span>
                  <span className="w-8 shrink-0 text-right tabular-nums text-text">{c.count}</span>
                </li>
              ))}
            </ul>
          </section>

          <section aria-label="Recurring mistakes">
            <SectionTitle>Recurring mistakes</SectionTitle>
            {summary.recurring.length === 0 ? (
              <p className="text-sm text-muted">Nothing has repeated yet.</p>
            ) : (
              <ul className="flex flex-col gap-1.5">
                {summary.recurring.map((m) => (
                  <li key={`${m.original}\u0000${m.corrected}`}>
                    <button
                      type="button"
                      onClick={() => openHistory(m.original, m.corrected)}
                      className="flex w-full items-baseline gap-2 rounded-md border border-border bg-surface p-2 text-left text-sm transition-colors duration-150 hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent motion-reduce:transition-none"
                    >
                      <span className="text-muted line-through">{m.original}</span>
                      <span aria-hidden="true">→</span>
                      <span className="font-medium text-text">{m.corrected}</span>
                      <span className="ml-auto text-xs text-muted">
                        {m.category.replace("_", " ")}
                      </span>
                      <span className="tabular-nums text-text">×{m.count}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section aria-label="Vocabulary">
            <SectionTitle>Vocabulary</SectionTitle>
            {summary.vocabulary.length === 0 ? (
              <p className="text-sm text-muted">No new words yet.</p>
            ) : (
              <ul className="flex flex-col gap-1.5">
                {summary.vocabulary.map((v) => (
                  <li
                    key={`${v.original}\u0000${v.corrected}`}
                    className="flex items-baseline gap-2 rounded-md border border-border bg-surface p-2 text-sm"
                  >
                    <span className="text-muted">{v.original}</span>
                    <span aria-hidden="true">→</span>
                    <span className="font-medium text-text">{v.corrected}</span>
                    <span className="ml-auto text-xs text-muted">
                      last seen {formatDate(v.lastSeen)}
                    </span>
                    <span className="tabular-nums text-text">×{v.count}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}
    </div>
  );
}
