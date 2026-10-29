import type { TokenWarning } from "../bindings/TokenWarning";
import { Banner } from "./Banner";
import { Button } from "./Button";

interface TokenWarningBannerProps {
  warnings: TokenWarning[];
  /** Dismiss the warning (not "Accept": that is the main action). */
  onIgnore: () => void;
  /** Re-run the same request. */
  onRetry: () => void;
}

/** Lists tokens (ids, paths, URLs, ...) that the output lost, with Ignore and Retry. */
export function TokenWarningBanner({ warnings, onIgnore, onRetry }: TokenWarningBannerProps) {
  if (warnings.length === 0) return null;
  return (
    <Banner
      variant="warning"
      actions={
        <>
          <Button size="sm" onClick={onIgnore}>
            Ignore
          </Button>
          <Button size="sm" variant="primary" onClick={onRetry}>
            Retry
          </Button>
        </>
      }
    >
      <p>The result is missing text from your input that should stay unchanged:</p>
      <ul className="mt-1 flex flex-wrap gap-1.5">
        {warnings.map((w, i) => (
          <li key={i}>
            <code className="rounded-sm bg-surface-raised px-1 py-0.5 text-xs">{w.token}</code>
          </li>
        ))}
      </ul>
    </Banner>
  );
}
