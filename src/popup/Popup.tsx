import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Mode } from "../bindings/Mode";
import type { TranslationResult } from "../bindings/TranslationResult";
import { Button } from "../components/Button";
import { ErrorBanner } from "../components/ErrorBanner";
import { IssueList } from "../components/IssueList";
import { SegmentedControl, type SegmentedOption } from "../components/SegmentedControl";
import { Spinner } from "../components/Spinner";
import { TokenWarningBanner } from "../components/TokenWarningBanner";
import {
  cancelTranslation,
  copyResult,
  getSettings,
  setLastPopupMode,
  toAppError,
  translate,
  type AppError,
} from "../lib/api";
import { cx } from "../lib/cx";

/** Payload of `popup://source`: the captured text (empty when nothing usable) and the mode. */
interface PopupSource {
  text: string;
  mode: Mode;
}

type Status = "idle" | "loading" | "success" | "error";

const MODES: Mode[] = ["translate", "improve", "correct"];
const MODE_OPTIONS: SegmentedOption<Mode>[] = [
  { value: "translate", label: "Translate" },
  { value: "improve", label: "Improve" },
  { value: "correct", label: "Correct" },
];

/** Source texts longer than this get the "Show more" toggle. */
const EXCERPT_CHARS = 180;

function hidePopup() {
  void getCurrentWindow().hide();
}

/** The quick-translate popup: review the result of the captured text and copy it with Enter. */
export function Popup() {
  const [text, setText] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>("translate");
  const [status, setStatus] = useState<Status>("idle");
  const [result, setResult] = useState<TranslationResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [copyError, setCopyError] = useState<AppError | null>(null);
  const [excerptOpen, setExcerptOpen] = useState(false);
  const [issuesOpen, setIssuesOpen] = useState(false);
  const [warningsIgnored, setWarningsIgnored] = useState(false);

  /** Id of the request whose answer the popup is waiting for; other answers are stale. */
  const requestIdRef = useRef<string | null>(null);

  /** Finished results of the current capture, by mode; cleared by a new capture. */
  const resultsRef = useRef<Partial<Record<Mode, TranslationResult>>>({});

  const cancelRunning = useCallback(() => {
    const id = requestIdRef.current;
    requestIdRef.current = null;
    if (id) void cancelTranslation(id).catch(() => undefined);
  }, []);

  const run = useCallback(
    async (source: string, runMode: Mode) => {
      cancelRunning();
      const requestId = crypto.randomUUID();
      requestIdRef.current = requestId;
      setStatus("loading");
      setError(null);
      setCopyError(null);
      setResult(null);
      setIssuesOpen(false);
      setWarningsIgnored(false);
      try {
        const settings = await getSettings();
        if (requestIdRef.current !== requestId) return;
        const res = await translate({
          requestId,
          sourceText: source,
          mode: runMode,
          provider: settings.defaultProvider,
          presetId: settings.defaultPresetId,
        });
        if (requestIdRef.current !== requestId) return;
        requestIdRef.current = null;
        resultsRef.current[runMode] = res;
        setResult(res);
        setStatus("success");
      } catch (e) {
        if (requestIdRef.current !== requestId) return;
        requestIdRef.current = null;
        const err = toAppError(e);
        if (err.kind === "Cancelled") {
          setStatus("idle");
        } else {
          setError(err);
          setStatus("error");
        }
      }
    },
    [cancelRunning],
  );

  // Latest values for the keyboard handler, which is registered once.
  const stateRef = useRef({ text, mode, status, result });
  useEffect(() => {
    stateRef.current = { text, mode, status, result };
  });

  const changeMode = useCallback(
    (next: Mode) => {
      const { text: source, mode: current } = stateRef.current;
      if (source === null || source.trim() === "" || next === current) return;
      setMode(next);
      void setLastPopupMode(next).catch(() => undefined);
      const kept = resultsRef.current[next];
      if (kept) {
        // Already ran for this capture: show it, no new run.
        cancelRunning();
        setResult(kept);
        setStatus("success");
        setError(null);
        setCopyError(null);
        setIssuesOpen(false);
        setWarningsIgnored(false);
        return;
      }
      void run(source, next);
    },
    [run, cancelRunning],
  );

  const accept = useCallback(async () => {
    const { mode: used, status: s, result: res } = stateRef.current;
    if (s !== "success" || !res) return;
    try {
      await copyResult(res.output, res.historyId);
    } catch (e) {
      // Stay open so the user sees why nothing was copied.
      setCopyError(toAppError(e));
      return;
    }
    void setLastPopupMode(used).catch(() => undefined);
    hidePopup();
  }, []);

  const openInMain = useCallback(() => {
    const { text: source, mode: used, result: res } = stateRef.current;
    if (source === null) return;
    void emit("main://open-entry", { input: source, mode: used, result: res });
  }, []);

  const retry = useCallback(() => {
    const { text: source, mode: used } = stateRef.current;
    if (source === null || source.trim() === "") return;
    void run(source, used);
  }, [run]);

  // A new capture from the hotkey: reset and auto-run.
  useEffect(() => {
    const unlisten = listen<PopupSource>("popup://source", (event) => {
      const { text: captured, mode: detected } = event.payload;
      cancelRunning();
      resultsRef.current = {};
      setText(captured);
      setMode(detected);
      setExcerptOpen(false);
      setResult(null);
      setError(null);
      setCopyError(null);
      if (captured.trim() === "") {
        setStatus("idle");
      } else {
        void run(captured, detected);
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [run, cancelRunning]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelRunning();
        hidePopup();
      } else if (e.key === "Enter" && !e.ctrlKey && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        void accept();
      } else if ((e.key === "r" || e.key === "R") && e.ctrlKey) {
        // Also stops the webview from reloading.
        e.preventDefault();
        retry();
      } else if (e.key === "Tab") {
        e.preventDefault();
        const i = MODES.indexOf(stateRef.current.mode);
        const step = e.shiftKey ? -1 : 1;
        changeMode(MODES[(i + step + MODES.length) % MODES.length]);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [accept, retry, changeMode, cancelRunning]);

  const hasText = text !== null && text.trim() !== "";
  const long = hasText && (text.length > EXCERPT_CHARS || text.split("\n").length > 3);
  const warnings = result && !warningsIgnored ? result.tokenWarnings : [];
  const issues = result?.issues ?? [];

  return (
    <main className="flex h-full flex-col gap-2 bg-bg px-3 pb-3 text-text">
      <div
        data-tauri-drag-region
        className="-mx-3 flex h-6 shrink-0 select-none items-center justify-center border-b border-border text-[10px] uppercase tracking-wide text-muted"
      >
        Prompt Translator
      </div>
      {!hasText ? (
        <p className="m-auto text-muted">
          {text === null ? "Waiting for text..." : "Select or copy some text first"}
        </p>
      ) : (
        <>
          <section aria-label="Source" className="text-sm">
            <p
              className={cx(
                "whitespace-pre-wrap break-words text-muted",
                !excerptOpen && "line-clamp-3",
              )}
            >
              {text}
            </p>
            {long && (
              <button
                type="button"
                className="mt-0.5 text-xs text-accent hover:underline"
                aria-expanded={excerptOpen}
                onClick={() => setExcerptOpen((v) => !v)}
              >
                {excerptOpen ? "Show less" : "Show more"}
              </button>
            )}
          </section>

          <SegmentedControl
            aria-label="Mode"
            options={MODE_OPTIONS}
            value={mode}
            onChange={changeMode}
          />

          <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
            {status === "loading" && (
              <div className="flex items-center gap-2 text-sm text-muted">
                <Spinner />
                <span>Working... Esc to cancel</span>
              </div>
            )}
            {status === "error" && error && (
              <ErrorBanner error={error} onRetry={retry} showSettingsAction={false} />
            )}
            {copyError && <ErrorBanner error={copyError} showSettingsAction={false} />}
            {status === "success" && result && (
              <>
                <p className="whitespace-pre-wrap break-words text-sm font-medium">
                  {result.output}
                </p>
                <TokenWarningBanner
                  warnings={warnings}
                  onIgnore={() => setWarningsIgnored(true)}
                  onRetry={retry}
                />
                {issues.length > 0 && (
                  <div className="flex flex-col gap-2">
                    <button
                      type="button"
                      className="self-start text-xs text-accent hover:underline"
                      aria-expanded={issuesOpen}
                      onClick={() => setIssuesOpen((v) => !v)}
                    >
                      {issues.length} {issues.length === 1 ? "fix" : "fixes"}
                    </button>
                    {issuesOpen && <IssueList issues={issues} />}
                  </div>
                )}
              </>
            )}
          </div>
        </>
      )}

      <footer className="flex items-center justify-between gap-2 border-t border-border pt-2 text-xs text-muted">
        <span>Enter: copy · Esc: close · Ctrl+R: retry · Tab: mode</span>
        {hasText && (
          <Button size="sm" variant="ghost" onClick={openInMain}>
            Open in main window
          </Button>
        )}
      </footer>
    </main>
  );
}
