import type { AppErrorPayload } from "../bindings/AppErrorPayload";
import type { ErrorKind } from "../bindings/ErrorKind";
import { LINKS, openUrl, toAppError } from "../lib/api";
import { useAppStore } from "../store/app";
import { Banner, type BannerVariant } from "./Banner";
import { Button } from "./Button";

interface ErrorBannerProps {
  error: AppErrorPayload;
  /** Shows the Retry action for kinds where retrying makes sense. */
  onRetry?: () => void;
  className?: string;
  /** Set to `false` where there is no settings screen to open (the popup window). */
  showSettingsAction?: boolean;
}

interface KindInfo {
  variant: BannerVariant;
  retry: boolean;
  openSettings: boolean;
  /** Link to the Claude Code install docs. */
  installLink?: boolean;
}

// Plan §4.6. The message and hint text come from Rust (`AppError`), so only the variant and the
// actions are decided here.
const KINDS: Record<ErrorKind, KindInfo> = {
  OllamaUnreachable: { variant: "danger", retry: true, openSettings: true },
  ModelMissing: { variant: "danger", retry: true, openSettings: true },
  ClaudeNotFound: { variant: "danger", retry: true, openSettings: true, installLink: true },
  ClaudeAuth: { variant: "danger", retry: true, openSettings: false },
  Timeout: { variant: "danger", retry: true, openSettings: false },
  Cancelled: { variant: "info", retry: true, openSettings: false },
  EmptyOutput: { variant: "danger", retry: true, openSettings: false },
  MalformedOutput: { variant: "danger", retry: true, openSettings: false },
  ProcessCrashed: { variant: "danger", retry: true, openSettings: false },
  SessionClosed: { variant: "warning", retry: false, openSettings: false },
  SessionUnreachable: { variant: "warning", retry: false, openSettings: false },
  InjectionFailed: { variant: "warning", retry: false, openSettings: false },
  PrefixNeedsConfirmation: { variant: "warning", retry: false, openSettings: false },
  Io: { variant: "danger", retry: false, openSettings: false },
  Db: { variant: "danger", retry: false, openSettings: false },
  InvalidInput: { variant: "danger", retry: false, openSettings: false },
};

/** Maps an `AppError` to a banner with its message, hint, details and actions. */
export function ErrorBanner({
  error,
  onRetry,
  className,
  showSettingsAction = true,
}: ErrorBannerProps) {
  const setScreen = useAppStore((s) => s.setScreen);
  const showToast = useAppStore((s) => s.showToast);
  const info = KINDS[error.kind] ?? KINDS.Io;

  function openLink(url: string) {
    openUrl(url).catch((e: unknown) => showToast("error", toAppError(e).message));
  }

  const actions = (
    <>
      {info.installLink && (
        <Button size="sm" variant="ghost" onClick={() => openLink(LINKS.claudeCodeSetup)}>
          Install help
        </Button>
      )}
      {info.openSettings && showSettingsAction && (
        <Button size="sm" onClick={() => setScreen("settings")}>
          Open settings
        </Button>
      )}
      {info.retry && onRetry && (
        <Button size="sm" variant="primary" onClick={onRetry}>
          Retry
        </Button>
      )}
    </>
  );

  return (
    <Banner variant={info.variant} actions={actions} className={className}>
      <p className="font-medium">{error.message}</p>
      {error.hint && <p className="text-muted">{error.hint}</p>}
      {error.detail && (
        <details className="mt-1">
          <summary className="cursor-pointer text-xs text-muted">Details</summary>
          <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap font-mono text-xs text-muted">
            {error.detail}
          </pre>
        </details>
      )}
    </Banner>
  );
}
