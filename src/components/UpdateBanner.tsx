import type { UpdateProgress } from "../bindings/UpdateProgress";
import { useUpdateStore } from "../store/update";
import { Banner } from "./Banner";
import { Button } from "./Button";

/** Progress bar for the download; indeterminate while the total size is unknown. */
export function UpdateProgressBar({ progress }: { progress: UpdateProgress | null }) {
  const total = progress?.totalBytes;
  const percent =
    progress && total ? Math.min(100, Math.round((progress.downloadedBytes / total) * 100)) : null;
  const label = progress?.finished
    ? "Starting the installer..."
    : percent !== null
      ? `Downloading... ${percent}%`
      : "Downloading...";
  return (
    <div className="mt-2">
      <div
        role="progressbar"
        aria-label="Update download"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={progress?.finished ? 100 : (percent ?? undefined)}
        className="h-1.5 w-full overflow-hidden rounded-full bg-border"
      >
        <div
          className={
            percent === null && !progress?.finished
              ? "h-full w-1/3 rounded-full bg-accent motion-safe:animate-pulse"
              : "h-full rounded-full bg-accent transition-[width] duration-150 motion-reduce:transition-none"
          }
          style={
            percent !== null || progress?.finished
              ? { width: `${progress?.finished ? 100 : percent}%` }
              : undefined
          }
        />
      </div>
      <p className="mt-1 text-xs text-muted">{label}</p>
    </div>
  );
}

/** Release notes are Markdown; shown as plain text. */
export function UpdateNotes({ notes }: { notes?: string }) {
  if (!notes?.trim()) return null;
  return (
    <p className="mt-1 max-h-24 overflow-auto whitespace-pre-wrap text-xs text-muted">
      {notes.trim()}
    </p>
  );
}

/** Global, dismissible banner for `update://available`. */
export function UpdateBanner() {
  const available = useUpdateStore((s) => s.available);
  const dismissed = useUpdateStore((s) => s.bannerDismissed);
  const installing = useUpdateStore((s) => s.installing);
  const progress = useUpdateStore((s) => s.progress);
  const installError = useUpdateStore((s) => s.installError);
  const install = useUpdateStore((s) => s.install);
  const dismiss = useUpdateStore((s) => s.dismissBanner);

  if (!available || (dismissed && !installing)) return null;

  return (
    <div className="shrink-0 px-4 pt-4">
      <Banner
        variant={installError ? "danger" : "info"}
        actions={
          !installing && (
            <>
              <Button size="sm" variant="primary" onClick={() => void install()}>
                Install and restart
              </Button>
              <Button size="sm" variant="ghost" onClick={dismiss}>
                Dismiss
              </Button>
            </>
          )
        }
      >
        <p className="font-medium">Version {available.version ?? ""} is available.</p>
        <UpdateNotes notes={available.notes} />
        {installError && <p className="mt-1 text-xs text-danger">Update failed: {installError}</p>}
        {installing && <UpdateProgressBar progress={progress} />}
      </Banner>
    </div>
  );
}
