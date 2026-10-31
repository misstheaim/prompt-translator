import { useEffect, useRef, useState, type KeyboardEvent } from "react";

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
import { copyResult, toAppError } from "../../lib/api";
import { cx } from "../../lib/cx";
import { useAppStore } from "../../store/app";
import { CompactToggle } from "../../components/CompactToggle";
import { LanguagePairControl } from "./LanguagePairControl";
import { MAX_INPUT_CHARS, useTranslateStore, type LanguagePair } from "./store";

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

export function TranslateScreen() {
  const settings = useAppStore((s) => s.settings);
  const presets = useAppStore((s) => s.presets);
  const showToast = useAppStore((s) => s.showToast);

  const input = useTranslateStore((s) => s.input);
  const modeOverride = useTranslateStore((s) => s.mode);
  const providerOverride = useTranslateStore((s) => s.provider);
  const presetOverride = useTranslateStore((s) => s.presetId);
  const status = useTranslateStore((s) => s.status);
  const result = useTranslateStore((s) => s.result);
  const output = useTranslateStore((s) => s.output);
  const resultSource = useTranslateStore((s) => s.resultSource);
  const resultMode = useTranslateStore((s) => s.resultMode);
  const error = useTranslateStore((s) => s.error);
  // The result whose token warnings the user dismissed (a new result shows its own).
  const [ignoredWarningsFor, setIgnoredWarningsFor] = useState<TranslationResult | null>(null);
  const inputWrapRef = useRef<HTMLDivElement>(null);
  const pairOverride = useTranslateStore((s) => s.pair);
  const compact = useAppStore((s) => s.compact !== null);
  const { setInput, setMode, setProvider, setPreset, setOutput, setPair, run, cancel } =
    useTranslateStore.getState();

  // Effective selections: the user's choice, else the settings defaults.
  const showMock = import.meta.env.DEV || settings?.devMockProvider === true;
  const providerOptions: SelectOption[] = (["ollama", "claudeCode"] as ProviderKind[])
    .concat(showMock ? ["mock"] : [])
    .map((p) => ({ value: p, label: PROVIDER_LABELS[p] }));
  const provider: ProviderKind = providerOverride ?? settings?.defaultProvider ?? "ollama";
  const presetId = presetOverride ?? settings?.defaultPresetId ?? presets[0]?.id ?? "";
  const preset = presets.find((p) => p.id === presetId) ?? presets[0];
  const mode: Mode = modeOverride ?? preset?.defaultMode ?? "translate";
  const pair: LanguagePair = pairOverride ?? {
    native: settings?.nativeLanguage ?? "ru",
    target: settings?.targetLanguage ?? "en",
  };

  const loading = status === "loading";
  const tooLong = input.length > MAX_INPUT_CHARS;
  const canRun =
    !loading && input.trim() !== "" && !tooLong && presetId !== "" && pair.native !== pair.target;

  function runNow() {
    if (!canRun) return;
    void run({ mode, provider, presetId: preset?.id ?? presetId, pair });
  }

  async function handleAccept() {
    const { output: text, result: res } = useTranslateStore.getState();
    if (text.trim() === "") return;
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

  if (compact) {
    return (
      <div
        className="relative flex h-full min-h-0 flex-col gap-2 p-2"
        data-compact="true"
        onKeyDown={onKeyDown}
      >
        <CompactToggle className="absolute right-3 top-3 z-10" />
        <div ref={inputWrapRef} className="relative min-h-0 flex-1">
          <TextArea
            compact
            aria-label="Text to process"
            placeholder="Type or paste your text (any language)"
            autoFocus
            className="pr-8"
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
              className="absolute bottom-1 right-1.5 flex h-5 w-5 items-center justify-center rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-40"
            >
              ×
            </button>
          )}
        </div>
        <div className="relative min-h-0 flex-1">
          <TextArea
            compact
            aria-label="Result text"
            placeholder="Result"
            value={output}
            onChange={(e) => setOutput(e.target.value)}
          />
          {loading && (
            <div className="absolute bottom-1.5 right-1.5">
              <Spinner />
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4" onKeyDown={onKeyDown}>
      <div className="flex flex-wrap items-center gap-3">
        <SegmentedControl
          aria-label="Mode"
          options={MODE_OPTIONS}
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
          options={presets.map((p) => ({ value: p.id, label: p.name }))}
          value={preset?.id ?? ""}
          onChange={(id) => {
            const p = presets.find((x) => x.id === id);
            if (p) setPreset(p.id, p.defaultMode);
          }}
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
              className="absolute right-1.5 top-1.5 flex h-6 w-6 items-center justify-center rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-40"
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
            <AcceptButton disabled={output.trim() === ""} onAccept={() => void handleAccept()} />
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
    </div>
  );
}

/** The main action. T40 changes the label and routing (clipboard vs. session). */
function AcceptButton({ disabled, onAccept }: { disabled: boolean; onAccept: () => void }) {
  return (
    <Button variant="primary" disabled={disabled} onClick={onAccept} shortcut="Ctrl+Shift+Enter">
      Copy
    </Button>
  );
}
