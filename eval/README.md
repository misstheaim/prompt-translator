# Eval harness

Re-measures translation quality with the real pipeline (`TranslationService`, the real prompts, the real providers). Run it by hand when prompts, presets or models change. It is not part of CI.

## Run

From the repo root (run `npm run build` once first; Tauri needs `dist/` to compile):

```
cargo run --manifest-path src-tauri/Cargo.toml --bin eval -- --provider ollama
```

or from `src-tauri`: `cargo run --bin eval -- --provider ollama`.

| Argument | Meaning | Default |
|---|---|---|
| `--provider ollama\|claude-code` | Provider to measure (required) | none |
| `--model <name>` | Ollama model tag, or the Claude model alias (`haiku`, ...) | the app's default (`qwen3.5:9b` / `haiku`) |
| `--cases <file>` | JSONL cases file | `eval/cases.jsonl` |
| `--out <file>` | Report path | `eval/reports/<UTC timestamp>.md` |
| `--preset <id>` | Preset used for every case without its own `preset` | the default preset (`technical-development`) |

Needs the provider to be ready: Ollama running with the model pulled, or Claude Code installed and signed in. Progress goes to stderr; the report is written at the end. `eval/reports/` is git-ignored.

The harness never touches your real app data: settings and presets live in `%TEMP%\prompt-translator-eval` (presets are re-seeded from the bundled ones on every run), and history is off. Timeouts are raised to 180 s (Ollama) and 120 s (Claude) because the first request can include loading the model.

## Cases

`eval/cases.jsonl`, one JSON object per line: `id`, `mode` (`translate`, `improve` or `correct`), `source`, `mustKeep` (tokens that must appear verbatim in the output), `notes`, and optionally `preset` (the preset id for this case; it overrides `--preset`). The `voice-*` cases run the five joke presets (T68); whether the output is in the preset's voice is judged by hand from the report, which shows each case's preset.

## Report

Per case: source, output, issues, token warnings, `mustKeep` misses, duration (or the error). Summary:

- **JSON-valid rate:** share of cases whose answer the output parser accepted. Provider failures (timeout, Ollama unreachable) are not counted as invalid.
- **mustKeep pass rate:** share of cases with a non-empty `mustKeep` whose output contains every token; a failed case counts as a miss.
- **Latency avg / p95:** over the successful cases (nearest-rank p95). The first case may include model load time.
- **Improve rows** (over the Improve cases that returned a result; issues counted after validation): **rewritten**, the share of cases whose output has at least 25% words that aren't in the source (a check that Improve rewrites rather than only fixing typos); **every issue explained**, the share with at least one issue and an explanation on each; **issues per case**; and **explanations over 35 words**. Each Improve case also shows its share of new words.

The earlier Python spike script (`spike_eval.py`) and the findings in `docs/spikes/local-model-eval.md` came from the spike; this binary is the maintained replacement.
