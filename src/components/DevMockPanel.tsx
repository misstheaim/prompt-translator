import { useState } from "react";

import type { Mode } from "../bindings/Mode";
import type { TranslationResult } from "../bindings/TranslationResult";
import { cancelTranslation, copyResult, isAppError, translate } from "../lib/api";

const MODES: Mode[] = ["translate", "improve", "correct"];
const SAMPLE = "я хочу add кнопку для PROJ-123 в src/settings.rs";

/**
 * Dev-only panel for checking the translation service by hand with the Mock provider
 * (markers: [mock:malformed], [mock:empty], [mock:slow], [mock:timeout]). The Translate
 * screen (T20) replaces it.
 */
export function DevMockPanel() {
  const [text, setText] = useState(SAMPLE);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [result, setResult] = useState<TranslationResult | null>(null);
  const [status, setStatus] = useState("");

  async function run(mode: Mode) {
    const id = crypto.randomUUID();
    setRequestId(id);
    setResult(null);
    setStatus(`Running ${mode}...`);
    try {
      const res = await translate({
        requestId: id,
        sourceText: text,
        mode,
        provider: "mock",
        presetId: "general",
      });
      setResult(res);
      setStatus(`Done in ${res.durationMs} ms`);
    } catch (e: unknown) {
      setStatus(isAppError(e) ? `${e.kind}: ${e.message}` : String(e));
    } finally {
      setRequestId(null);
    }
  }

  async function copy() {
    if (!result) return;
    try {
      await copyResult(result.output, result.historyId);
      setStatus("Copied to the clipboard");
    } catch (e: unknown) {
      setStatus(isAppError(e) ? `${e.kind}: ${e.message}` : String(e));
    }
  }

  return (
    <section className="mt-4 w-full max-w-xl rounded-lg border border-border bg-surface p-4 text-left text-sm text-text">
      <h2 className="font-semibold">Mock provider (dev)</h2>
      <textarea
        className="mt-2 h-20 w-full rounded border border-border bg-bg p-2"
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <div className="mt-2 flex flex-wrap gap-2">
        {MODES.map((mode) => (
          <button
            key={mode}
            type="button"
            className="rounded border border-border px-3 py-1 disabled:opacity-50"
            disabled={requestId !== null}
            onClick={() => void run(mode)}
          >
            {mode}
          </button>
        ))}
        <button
          type="button"
          className="rounded border border-border px-3 py-1 disabled:opacity-50"
          disabled={requestId === null}
          onClick={() => requestId && void cancelTranslation(requestId)}
        >
          Cancel
        </button>
        <button
          type="button"
          className="rounded border border-border px-3 py-1 disabled:opacity-50"
          disabled={result === null}
          onClick={() => void copy()}
        >
          Copy
        </button>
      </div>
      <p className="mt-2 text-muted">{status}</p>
      {result && (
        <pre className="mt-2 max-h-60 overflow-auto whitespace-pre-wrap rounded bg-bg p-2 text-xs">
          {JSON.stringify(result, null, 2)}
        </pre>
      )}
    </section>
  );
}
