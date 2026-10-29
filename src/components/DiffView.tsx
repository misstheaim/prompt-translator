import { useMemo } from "react";
import { diffWords } from "diff";

interface DiffViewProps {
  /** The text the user typed. */
  source: string;
  /** The text the provider returned. */
  output: string;
}

/** Word-level diff: deletions struck through in red, insertions highlighted in green. */
export function DiffView({ source, output }: DiffViewProps) {
  const parts = useMemo(() => diffWords(source, output), [source, output]);
  return (
    <div
      aria-label="Changes"
      className="whitespace-pre-wrap break-words rounded-md border border-border bg-surface p-2 text-sm text-text"
    >
      {parts.map((part, i) => {
        if (part.added) {
          return (
            <ins key={i} className="rounded-sm bg-success/20 no-underline">
              {part.value}
            </ins>
          );
        }
        if (part.removed) {
          return (
            <del key={i} className="rounded-sm bg-danger/20 text-muted">
              {part.value}
            </del>
          );
        }
        return <span key={i}>{part.value}</span>;
      })}
    </div>
  );
}
