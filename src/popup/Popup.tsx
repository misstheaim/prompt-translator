import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Mode } from "../bindings/Mode";
import type { PopupLanguagePair } from "../bindings/PopupLanguagePair";
import type { PopupSource } from "../bindings/PopupSource";
import type { Settings } from "../bindings/Settings";
import type { SourceKind } from "../bindings/SourceKind";
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
  replaceSelection,
  setLastPopupMode,
  toAppError,
  translate,
  type AppError,
} from "../lib/api";
import { applyAppearance } from "../lib/appearance";
import { cx } from "../lib/cx";
import { languageName, useLanguages } from "../lib/useLanguages";

/** Where the current capture came from, read by `run` and the mode-memory calls. */
interface Capture {
  sourceKind: SourceKind;
  /** The reversed pair of a reverse translation; Translate only, `null` for the settings' pair. */
  languagePair: PopupLanguagePair | null;
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
  const [languagePair, setLanguagePair] = useState<PopupLanguagePair | null>(null);
  const [sourceKind, setSourceKind] = useState<SourceKind>("unknown");
  const languages = useLanguages();

  /** Finished results of the current capture, by mode; cleared by a new capture. */
  const resultsRef = useRef<Partial<Record<Mode, TranslationResult>>>({});

  /** The current capture's source; set synchronously on `popup://source`. */
  const captureRef = useRef<Capture>({ sourceKind: "unknown", languagePair: null });

  /** Failed runs of the current capture, by mode; shown when that mode is selected. */
  const errorsRef = useRef<Partial<Record<Mode, AppError>>>({});

  /** The one request in flight; other answers are stale. */
  const runningRef = useRef<{ mode: Mode; requestId: string } | null>(null);

  /** Modes waiting for the running request to finish. */
  const queueRef = useRef<Mode[]>([]);

  /** The mode shown right now; set synchronously (the state lags a render behind). */
  const shownModeRef = useRef<Mode>("translate");

  /** Cancels the request in flight and forgets the queue. */
  const cancelRunning = useCallback(() => {
    const running = runningRef.current;
    runningRef.current = null;
    queueRef.current = [];
    if (running) void cancelTranslation(running.requestId).catch(() => undefined);
  }, []);

  /** Shows `shown` from what the capture has so far: result, error, waiting or nothing. */
  const showMode = useCallback((shown: Mode) => {
    shownModeRef.current = shown;
    setMode(shown);
    setCopyError(null);
    setIssuesOpen(false);
    setWarningsIgnored(false);
    const kept = resultsRef.current[shown];
    const failed = errorsRef.current[shown];
    if (kept) {
      setResult(kept);
      setError(null);
      setStatus("success");
    } else if (failed) {
      setResult(null);
      setError(failed);
      setStatus("error");
    } else {
      setResult(null);
      setError(null);
      const waiting = runningRef.current?.mode === shown || queueRef.current.includes(shown);
      setStatus(waiting ? "loading" : "idle");
    }
  }, []);

  /**
   * Saves the mode the user ended on to `lastPopupMode`. Not for read-only sources: their
   * Translate was not the user's choice, and "Last used" is about text the user writes.
   */
  const rememberMode = useCallback((used: Mode) => {
    if (captureRef.current.sourceKind === "readOnly") return;
    void setLastPopupMode(used).catch(() => undefined);
  }, []);

  /** `enqueue` for its own use when a run settles. */
  const enqueueRef = useRef<((source: string, runMode: Mode) => void) | null>(null);

  /**
   * Runs `runMode` now if no request is in flight, otherwise queues it. A mode that is
   * already running, queued or finished is left alone. When a run settles, the next queued
   * mode starts, the shown one first.
   */
  const enqueue = useCallback(
    (source: string, runMode: Mode) => {
      if (resultsRef.current[runMode] || errorsRef.current[runMode]) return;
      if (runningRef.current?.mode === runMode || queueRef.current.includes(runMode)) return;
      if (runningRef.current) {
        queueRef.current.push(runMode);
        return;
      }
      const requestId = crypto.randomUUID();
      runningRef.current = { mode: runMode, requestId };
      // A reverse translation swaps the pair for Translate only; Improve and Correct always
      // use the settings' pair.
      const pair = runMode === "translate" ? captureRef.current.languagePair : null;

      /** Stores the outcome for its mode, updates the view if it's shown, starts the next. */
      const settle = (res: TranslationResult | null, err: AppError | null) => {
        if (runningRef.current?.requestId !== requestId) return; // cancelled: stale
        runningRef.current = null;
        if (res) resultsRef.current[runMode] = res;
        else if (err && err.kind !== "Cancelled") errorsRef.current[runMode] = err;
        if (shownModeRef.current === runMode) showMode(runMode);
        const queue = queueRef.current;
        const next = queue.includes(shownModeRef.current) ? shownModeRef.current : queue[0];
        if (next === undefined) return;
        queueRef.current = queue.filter((m) => m !== next);
        enqueueRef.current?.(source, next);
      };

      void (async () => {
        try {
          const settings = await getSettings();
          if (runningRef.current?.requestId !== requestId) return;
          const res = await translate({
            requestId,
            sourceText: source,
            mode: runMode,
            provider: settings.defaultProvider,
            presetId: settings.defaultPresetId,
            ...(pair ?? {}),
          });
          settle(res, null);
        } catch (e) {
          settle(null, toAppError(e));
        }
      })();
    },
    [showMode],
  );
  useEffect(() => {
    enqueueRef.current = enqueue;
  }, [enqueue]);

  /** Starts `runMode` (or queues it) and shows the waiting state if it's the shown mode. */
  const run = useCallback(
    (source: string, runMode: Mode) => {
      enqueue(source, runMode);
      if (shownModeRef.current === runMode) showMode(runMode);
    },
    [enqueue, showMode],
  );

  // Latest values for the keyboard handler, which is registered once.
  const stateRef = useRef({ text, mode, status, result });
  useEffect(() => {
    stateRef.current = { text, mode, status, result };
  });

  const changeMode = useCallback(
    (next: Mode) => {
      const { text: source } = stateRef.current;
      if (source === null || source.trim() === "" || next === shownModeRef.current) return;
      rememberMode(next);
      // A run in progress is never cancelled by a switch; a mode without a result is queued.
      enqueue(source, next);
      showMode(next);
    },
    [enqueue, showMode, rememberMode],
  );

  /** Enter replaces the selection when the source was editable; Ctrl+Enter only copies. */
  const accept = useCallback(
    async (replace: boolean) => {
      const { mode: used, status: s, result: res } = stateRef.current;
      if (s !== "success" || !res) return;
      const doReplace = replace && captureRef.current.sourceKind === "editable";
      if (doReplace) rememberMode(used);
      try {
        if (doReplace) {
          await replaceSelection(res.output, res.historyId);
        } else {
          await copyResult(res.output, res.historyId);
        }
      } catch (e) {
        // Stay open so the user sees why nothing was copied.
        setCopyError(toAppError(e));
        return;
      }
      if (!doReplace) rememberMode(used);
      hidePopup();
    },
    [rememberMode],
  );

  const openInMain = useCallback(() => {
    const { text: source, mode: used, result: res } = stateRef.current;
    if (source === null) return;
    void emit("main://open-entry", { input: source, mode: used, result: res });
  }, []);

  const retry = useCallback(() => {
    const { text: source } = stateRef.current;
    if (source === null || source.trim() === "") return;
    const shown = shownModeRef.current;
    const running = runningRef.current;
    // Only the shown mode is rerun. If it is waiting in the queue, it will run anyway.
    if (queueRef.current.includes(shown)) return;
    if (running?.mode === shown) {
      runningRef.current = null;
      void cancelTranslation(running.requestId).catch(() => undefined);
    }
    delete resultsRef.current[shown];
    delete errorsRef.current[shown];
    run(source, shown);
  }, [run]);

  // Theme and accent: read once, then follow `settings://changed` (T67).
  useEffect(() => {
    let cancelled = false;
    void getSettings().then(
      (s) => {
        if (!cancelled) applyAppearance(s.theme, s.accent);
      },
      () => undefined,
    );
    const unlisten = listen<Settings>("settings://changed", (event) => {
      applyAppearance(event.payload.theme, event.payload.accent);
    });
    return () => {
      cancelled = true;
      void unlisten.then((fn) => fn());
    };
  }, []);

  // A new capture from the hotkey: reset and auto-run.
  useEffect(() => {
    const unlisten = listen<PopupSource>("popup://source", (event) => {
      const { text: captured, mode: detected, sourceKind, languagePair: pair } = event.payload;
      cancelRunning();
      resultsRef.current = {};
      errorsRef.current = {};
      captureRef.current = { sourceKind, languagePair: pair ?? null };
      setLanguagePair(pair ?? null);
      setSourceKind(sourceKind);
      setText(captured);
      setExcerptOpen(false);
      showMode(detected);
      if (captured.trim() !== "") run(captured, detected);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [run, cancelRunning, showMode]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelRunning();
        hidePopup();
      } else if (e.key === "Enter" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        void accept(!e.ctrlKey);
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
  // Shown only when the direction isn't the settings' pair (a reverse translation).
  const direction =
    mode === "translate" && languagePair
      ? `${languageName(languages, languagePair.nativeLanguage)} → ${languageName(languages, languagePair.targetLanguage)}`
      : null;

  const keyHint = `${
    sourceKind === "editable" ? "Enter: replace · Ctrl+Enter: copy" : "Enter: copy"
  } · Esc: close · Ctrl+R: retry · Tab: mode`;

  return (
    <main className="flex h-full flex-col gap-2 overflow-hidden bg-bg p-3 text-text">
      {!hasText ? (
        <p className="m-auto text-muted">
          {text === null ? "Waiting for text..." : "Select or copy some text first"}
        </p>
      ) : (
        <>
          <section aria-label="Source" className="max-h-[40%] shrink-0 overflow-y-auto text-sm">
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

          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <SegmentedControl
              aria-label="Mode"
              options={MODE_OPTIONS}
              value={mode}
              onChange={changeMode}
            />
            {direction && (
              <span className="text-xs text-muted" title="Translation direction">
                {direction}
              </span>
            )}
          </div>

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

      {/* One line whatever the width: the hint shrinks with an ellipsis (full text on hover). */}
      <footer className="flex h-9 shrink-0 items-center justify-between gap-2 border-t border-border pt-2 text-xs text-muted">
        <span className="min-w-0 truncate whitespace-nowrap" title={keyHint}>
          {keyHint}
        </span>
        {hasText && (
          <Button
            size="sm"
            variant="ghost"
            className="shrink-0 whitespace-nowrap"
            onClick={openInMain}
          >
            Open in main window
          </Button>
        )}
      </footer>
    </main>
  );
}
