import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useStore } from "zustand";

import type { Mode } from "../../bindings/Mode";
import type { ProviderKind } from "../../bindings/ProviderKind";
import type { TranslationResult } from "../../bindings/TranslationResult";
import { Button } from "../../components/Button";
import { DiffView } from "../../components/DiffView";
import { ErrorBanner } from "../../components/ErrorBanner";
import { IssueList } from "../../components/IssueList";
import { SegmentedControl, type SegmentedOption } from "../../components/SegmentedControl";
import { Select, type SelectOption } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { TextArea } from "../../components/TextArea";
import { TokenWarningBanner } from "../../components/TokenWarningBanner";
import { copyResult, deliverToTarget, toAppError } from "../../lib/api";
import { cx } from "../../lib/cx";
import { useAppStore } from "../../store/app";
import { CompactToggle } from "../../components/CompactToggle";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { LanguagePairControl } from "./LanguagePairControl";
import { effectiveSelection, offeredProviders } from "./selection";
import type { BoundTab } from "./sessionTabs";
import { MAX_INPUT_CHARS, type TabStore } from "./store";

const MODE_OPTIONS: SegmentedOption<Mode>[] = [
  { value: "translate", label: "Translate" },
  { value: "improve", label: "Improve" },
  { value: "correct", label: "Correct" },
];

const PROVIDER_LABELS: Record<ProviderKind, string> = {
  ollama: "Ollama",
  claudeCode: "Claude Code",
  mock: "Mock (dev)",
};

function formatDuration(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

interface TranslateScreenProps {
  /** The tab's state: the default tab's store, or a bound tab's own. */
  store: TabStore;
  /** Set for a tab bound to a session: Accept sends there and Improve writes a Claude prompt. */
  bound?: BoundTab;
  /** Compact mode: the dropdown that switches tabs (shown in the top row). */
  tabPicker?: ReactNode;
}

export function TranslateScreen({ store, bound, tabPicker }: TranslateScreenProps) {
  const settings = useAppStore((s) => s.settings);
  const presets = useAppStore((s) => s.presets);
  const showToast = useAppStore((s) => s.showToast);

  const input = useStore(store, (s) => s.input);
  const modeOverride = useStore(store, (s) => s.mode);
  const providerOverride = useStore(store, (s) => s.provider);
  const presetOverride = useStore(store, (s) => s.presetId);
  const status = useStore(store, (s) => s.status);
  const result = useStore(store, (s) => s.result);
  const output = useStore(store, (s) => s.output);
  const resultSource = useStore(store, (s) => s.resultSource);
  const resultMode = useStore(store, (s) => s.resultMode);
  const error = useStore(store, (s) => s.error);
  // The result whose token warnings the user dismissed (a new result shows its own).
  const [ignoredWarningsFor, setIgnoredWarningsFor] = useState<TranslationResult | null>(null);
  // The D35 confirmation: the text to send once the user agrees (prefix `!`, `/` or `#`).
  const [confirmSend, setConfirmSend] = useState<string | null>(null);
  const inputWrapRef = useRef<HTMLDivElement>(null);
  const pairOverride = useStore(store, (s) => s.pair);
  const compact = useAppStore((s) => s.compact !== null);
  const { setInput, setMode, setProvider, setPreset, setOutput, setPair, run, cancel } =
    store.getState();

  // Effective selections: the user's choice, else the settings defaults.
  const providerOptions: SelectOption[] = offeredProviders(settings).map((p) => ({
    value: p,
    label: PROVIDER_LABELS[p],
  }));
  const { provider, preset, presetId, mode, pair } = effectiveSelection(
    {
      mode: modeOverride,
      provider: providerOverride,
      presetId: presetOverride,
      pair: pairOverride,
    },
    settings,
    presets,
  );
  // In a bound tab Improve rewrites the text as a clearer prompt for Claude (#199).
  const modeOptions: SegmentedOption<Mode>[] = bound
    ? MODE_OPTIONS.map((o) =>
        o.value === "improve"
          ? { ...o, title: "Improve: rewrite as a clearer prompt for Claude" }
          : o,
      )
    : MODE_OPTIONS;

  const loading = status === "loading";
  const tooLong = input.length > MAX_INPUT_CHARS;
  const canRun =
    !loading && input.trim() !== "" && !tooLong && presetId !== "" && pair.native !== pair.target;

  function runNow() {
    if (!canRun) return;
    void run({
      mode,
      provider,
      presetId,
      pair,
      purpose: bound ? "claudePrompt" : undefined,
    });
  }

  async function sendToSession(text: string, historyId: number | null, confirmed: boolean) {
    if (!bound) return;
    try {
      const outcome = await deliverToTarget(bound.target, text, historyId, confirmed);
      if (outcome.kind === "delivered") {
        showToast("success", `Sent to ${bound.title}: press Enter in Claude`);
      } else {
        showToast("info", outcome.reason);
      }
    } catch (e) {
      const err = toAppError(e);
      if (err.kind === "PrefixNeedsConfirmation") setConfirmSend(text);
      else showToast("error", err.message);
    }
  }

  async function handleAccept() {
    const { output: text, result: res } = store.getState();
    if (text.trim() === "") return;
    if (bound) {
      await sendToSession(text, res?.historyId ?? null, false);
      return;
    }
    try {
      await copyResult(text, res?.historyId ?? null);
      showToast("success", "Copied");
    } catch (e) {
      showToast("error", toAppError(e).message);
    }
  }

  /** Clears only the input; the result, mode, provider and preset stay. */
  function clearInput() {
    setInput("");
    inputWrapRef.current?.querySelector("textarea")?.focus();
  }

  // Compact mode has no banner: an error shows as a toast instead (once per failed run).
  useEffect(() => {
    if (compact && status === "error" && error) {
      useAppStore.getState().showToast("error", error.message);
    }
  }, [compact, status, error]);

  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "Enter" || !e.ctrlKey) return;
    e.preventDefault();
    if (e.shiftKey) void handleAccept();
    else runNow();
  }

  const confirmDialog = (
    <ConfirmDialog
      open={confirmSend !== null}
      title="Send this text?"
      message="The text starts with !, / or #. In Claude Code that runs a shell command, runs a slash command or writes a memory note when you press Enter."
      confirmLabel="Send"
      onCancel={() => setConfirmSend(null)}
      onConfirm={() => {
        const text = confirmSend;
        setConfirmSend(null);
        if (text !== null) {
          void sendToSession(text, store.getState().result?.historyId ?? null, true);
        }
      }}
    />
  );

  const presetOptions: SelectOption[] = presets.map((p) => ({ value: p.id, label: p.name }));
  function choosePreset(id: string) {
    const p = presets.find((x) => x.id === id);
    if (p) setPreset(p.id, p.defaultMode);
  }
  const acceptLabel = bound ? `Send to ${bound.title}` : "Copy";

  // Compact mode (TRANS-016): a small complete window. Its height is split by the two boxes;
  // everything else has a fixed height, and wide items shrink (with an ellipsis) instead of
  // wrapping, so the window never needs a scrollbar down to the compact minimum size.
  if (compact) {
    const acceptHint = bound ? `send to ${bound.title}` : "copy";
    return (
      <div
        className="flex h-full min-h-0 flex-col gap-1.5 overflow-hidden p-2"
        data-compact="true"
        onKeyDown={onKeyDown}
      >
        <div className="flex shrink-0 items-center gap-1.5">
          {tabPicker}
          <SegmentedControl
            size="sm"
            aria-label="Mode"
            options={modeOptions}
            value={mode}
            onChange={setMode}
            disabled={loading}
          />
          <Button
            size="xs"
            variant="primary"
            loading={loading}
            disabled={!canRun}
            onClick={runNow}
            title="Run (Ctrl+Enter)"
            className="ml-auto shrink-0"
          >
            {loading ? "Running" : "Run"}
          </Button>
          <CompactToggle className="shrink-0" />
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Select
            size="sm"
            aria-label="Provider"
            title="Provider"
            options={providerOptions}
            value={provider}
            onChange={(v) => setProvider(v as ProviderKind)}
            disabled={loading}
          />
          <Select
            size="sm"
            aria-label="Preset"
            title={preset ? `Preset: ${preset.name}` : "Preset"}
            options={presetOptions}
            value={preset?.id ?? ""}
            onChange={choosePreset}
            disabled={loading || presets.length === 0}
          />
          <LanguagePairControl
            size="sm"
            className="ml-auto"
            pair={pair}
            onChange={setPair}
            disabled={loading}
          />
        </div>
        {/* The counter and the clear button sit in a strip under the text, never over it. */}
        <div
          ref={inputWrapRef}
          className="flex min-h-14 flex-1 flex-col overflow-hidden rounded-md border border-border bg-surface focus-within:ring-2 focus-within:ring-accent"
        >
          <TextArea
            compact
            framed
            aria-label="Text to process"
            placeholder="Type or paste your text (any language)"
            autoFocus
            className="min-h-0 flex-1"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
          <div className="flex h-5 shrink-0 items-center justify-between gap-2 pl-2 pr-1">
            <span
              className={cx("text-[10px] leading-3", tooLong ? "text-danger" : "text-muted")}
              aria-live="polite"
            >
              {input.length} / {MAX_INPUT_CHARS}
              {tooLong && " (too long)"}
            </span>
            {input !== "" && (
              <button
                type="button"
                aria-label="Clear text"
                title="Clear"
                disabled={loading}
                onClick={clearInput}
                className="flex h-4 w-4 items-center justify-center rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-40"
              >
                ×
              </button>
            )}
          </div>
        </div>
        <div className="relative min-h-14 flex-1">
          <TextArea
            compact
            aria-label="Result text"
            placeholder="Result"
            value={output}
            onChange={(e) => setOutput(e.target.value)}
          />
          {loading && (
            <div className="absolute bottom-1.5 right-3">
              <Spinner />
            </div>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <p
            className="min-w-0 flex-1 truncate text-[11px] leading-4 text-muted"
            title={`Ctrl+Enter run · Ctrl+Shift+Enter ${acceptHint}`}
          >
            Ctrl+Enter run · Ctrl+Shift+Enter {acceptHint}
          </p>
          <Button
            size="xs"
            variant="primary"
            disabled={output.trim() === ""}
            onClick={() => void handleAccept()}
            aria-label={acceptLabel}
            title={`${acceptLabel} (Ctrl+Shift+Enter)`}
            className="w-8 shrink-0 px-0"
          >
            {bound ? <SendIcon /> : <CopyIcon />}
          </Button>
        </div>
        {confirmDialog}
      </div>
    );
  }

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4" onKeyDown={onKeyDown}>
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl
          aria-label="Mode"
          options={modeOptions}
          value={mode}
          onChange={setMode}
          disabled={loading}
        />
        <Select
          label="Provider"
          options={providerOptions}
          value={provider}
          onChange={(v) => setProvider(v as ProviderKind)}
          disabled={loading}
        />
        <Select
          label="Preset"
          options={presetOptions}
          value={preset?.id ?? ""}
          onChange={choosePreset}
          disabled={loading || presets.length === 0}
        />
        <LanguagePairControl pair={pair} onChange={setPair} disabled={loading} />
      </div>

      <div className="flex flex-col gap-1">
        <div ref={inputWrapRef} className="relative">
          <TextArea
            aria-label="Text to process"
            placeholder="Type or paste your text (any language)"
            rows={6}
            autoFocus
            className="pr-9"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
          {input !== "" && (
            <button
              type="button"
              aria-label="Clear text"
              title="Clear"
              disabled={loading}
              onClick={clearInput}
              className="absolute right-3 top-1.5 flex h-6 w-6 items-center justify-center rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-40"
            >
              ×
            </button>
          )}
        </div>
        <div className="flex items-center justify-between gap-3">
          <span
            className={cx("text-xs", tooLong ? "text-danger" : "text-muted")}
            aria-live="polite"
          >
            {input.length} / {MAX_INPUT_CHARS}
            {tooLong && " (too long)"}
          </span>
          <div className="flex items-center gap-2">
            {loading && <Button onClick={() => void cancel()}>Cancel</Button>}
            <Button variant="primary" loading={loading} disabled={!canRun} onClick={runNow}>
              {loading ? "Running" : "Run"}
              {!loading && <span className="text-xs font-normal opacity-70">Ctrl+Enter</span>}
            </Button>
          </div>
        </div>
      </div>

      {status === "error" && error && <ErrorBanner error={error} onRetry={runNow} />}

      {result && (
        <section aria-label="Result" className="flex flex-col gap-2">
          <TextArea
            aria-label="Result text"
            rows={6}
            value={output}
            onChange={(e) => setOutput(e.target.value)}
          />
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-muted">
              {PROVIDER_LABELS[result.provider]} · {result.model} ·{" "}
              {formatDuration(result.durationMs)}
            </span>
            <AcceptButton
              label={acceptLabel}
              disabled={output.trim() === ""}
              onAccept={() => void handleAccept()}
            />
          </div>
          {result.tokenWarnings.length > 0 && ignoredWarningsFor !== result && (
            <TokenWarningBanner
              warnings={result.tokenWarnings}
              onIgnore={() => setIgnoredWarningsFor(result)}
              onRetry={runNow}
            />
          )}
          {resultMode !== "translate" && (
            <>
              <h3 className="mt-2 text-sm font-medium text-text">Changes</h3>
              <DiffView source={resultSource} output={result.output} />
              {result.issues.length > 0 && (
                <>
                  <h3 className="mt-2 text-sm font-medium text-text">
                    Issues ({result.issues.length})
                  </h3>
                  <IssueList issues={result.issues} />
                </>
              )}
            </>
          )}
        </section>
      )}
      {confirmDialog}
    </div>
  );
}

/** The main action: Copy in the default tab, "Send to <session>" in a bound tab. */
function AcceptButton({
  label,
  disabled,
  onAccept,
}: {
  label: string;
  disabled: boolean;
  onAccept: () => void;
}) {
  return (
    <Button
      variant="primary"
      disabled={disabled}
      onClick={onAccept}
      shortcut="Ctrl+Shift+Enter"
      title={label}
      className="max-w-72"
    >
      <span className="truncate">{label}</span>
    </Button>
  );
}

/** Paper plane: send to the bound Claude session (compact mode). */
function SendIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      width="14"
      height="14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M14.5 1.5 7 9M14.5 1.5 10 14.5 7 9 1.5 6z" />
    </svg>
  );
}

/** Two sheets: copy the result (compact mode, default tab). */
function CopyIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      width="14"
      height="14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
      <path d="M10.5 3.5v-1A1 1 0 0 0 9.5 1.5h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h1" />
    </svg>
  );
}
