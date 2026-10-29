import { useState } from "react";

import type { Issue } from "../bindings/Issue";
import type { IssueCategory } from "../bindings/IssueCategory";
import { cx } from "../lib/cx";
import { Button } from "./Button";

/** Lists longer than this collapse behind a "Show all" toggle. */
export const COLLAPSE_AFTER = 5;

// Full class names so Tailwind can see them.
const CATEGORY_STYLES: Record<IssueCategory, string> = {
  spelling: "text-cat-spelling border-cat-spelling",
  grammar: "text-cat-grammar border-cat-grammar",
  article: "text-cat-article border-cat-article",
  tense: "text-cat-tense border-cat-tense",
  preposition: "text-cat-preposition border-cat-preposition",
  word_choice: "text-cat-word-choice border-cat-word-choice",
  vocabulary: "text-cat-vocabulary border-cat-vocabulary",
  punctuation: "text-cat-punctuation border-cat-punctuation",
  style: "text-cat-style border-cat-style",
  other: "text-cat-other border-cat-other",
};

/** Coloured pill with the issue category. */
export function CategoryChip({ category }: { category: IssueCategory }) {
  return (
    <span
      className={cx(
        "rounded-full border px-2 py-0.5 text-xs font-medium",
        CATEGORY_STYLES[category],
      )}
    >
      {category.replace("_", " ")}
    </span>
  );
}

/**
 * The mistakes of one result: category, `original → corrected` and the explanation.
 * Collapses to the first few items when there are more than `COLLAPSE_AFTER`.
 */
export function IssueList({ issues }: { issues: Issue[] }) {
  const [expanded, setExpanded] = useState(false);
  const collapsible = issues.length > COLLAPSE_AFTER;
  const shown = collapsible && !expanded ? issues.slice(0, COLLAPSE_AFTER) : issues;

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-2">
        {shown.map((issue, i) => (
          <li key={i} className="rounded-md border border-border bg-surface p-2 text-sm">
            <div className="flex flex-wrap items-baseline gap-2">
              <CategoryChip category={issue.category} />
              <span className="text-muted line-through">{issue.original}</span>
              <span aria-hidden="true">→</span>
              <span className="font-medium text-text">{issue.corrected}</span>
            </div>
            {issue.explanation && <p className="mt-1 text-muted">{issue.explanation}</p>}
          </li>
        ))}
      </ul>
      {collapsible && (
        <Button
          size="sm"
          variant="ghost"
          className="self-start"
          aria-expanded={expanded}
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? "Show fewer" : `Show all ${issues.length}`}
        </Button>
      )}
    </div>
  );
}
