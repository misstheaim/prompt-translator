import type { Issue } from "../../bindings/Issue";
import type { IssueCategory } from "../../bindings/IssueCategory";

// Full class names so Tailwind can see them.
const CATEGORY_TEXT: Record<IssueCategory, string> = {
  spelling: "text-cat-spelling",
  grammar: "text-cat-grammar",
  article: "text-cat-article",
  tense: "text-cat-tense",
  preposition: "text-cat-preposition",
  word_choice: "text-cat-word-choice",
  vocabulary: "text-cat-vocabulary",
  punctuation: "text-cat-punctuation",
  style: "text-cat-style",
  other: "text-cat-other",
};

/** The mistakes of one entry: `original → corrected`, category and explanation. */
export function IssueList({ issues }: { issues: Issue[] }) {
  return (
    <ul className="flex flex-col gap-2">
      {issues.map((issue, i) => (
        <li key={i} className="rounded-md border border-border bg-surface p-2 text-sm">
          <div className="flex flex-wrap items-baseline gap-2">
            <span className="text-muted line-through">{issue.original}</span>
            <span aria-hidden="true">→</span>
            <span className="font-medium text-text">{issue.corrected}</span>
            <span className={`ml-auto text-xs font-medium ${CATEGORY_TEXT[issue.category]}`}>
              {issue.category.replace("_", " ")}
            </span>
          </div>
          {issue.explanation && <p className="mt-1 text-muted">{issue.explanation}</p>}
        </li>
      ))}
    </ul>
  );
}
