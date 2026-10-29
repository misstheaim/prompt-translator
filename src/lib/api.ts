// Typed wrappers around Tauri commands. The UI never calls `invoke` directly (plan D3).
// One section per Rust module; each backend task adds its own section.
import { invoke } from "@tauri-apps/api/core";

import type { AppErrorPayload } from "../bindings/AppErrorPayload";
import type { ErrorKind } from "../bindings/ErrorKind";
import type { HistoryEntry } from "../bindings/HistoryEntry";
import type { HistoryEntrySummary } from "../bindings/HistoryEntrySummary";
import type { IssueFilter } from "../bindings/IssueFilter";
import type { MistakesSummary } from "../bindings/MistakesSummary";
import type { Mode } from "../bindings/Mode";
import type { Preset } from "../bindings/Preset";
import type { ProviderStatus } from "../bindings/ProviderStatus";
import type { Settings } from "../bindings/Settings";
import type { TranslationRequest } from "../bindings/TranslationRequest";
import type { TranslationResult } from "../bindings/TranslationResult";

// ---- errors and the shared call helper ----

/** The Rust `AppError` as a thrown value: `{ kind, message, hint, detail? }`. */
export class AppError extends Error implements AppErrorPayload {
  readonly kind: ErrorKind;
  readonly hint: string;
  readonly detail?: string;

  constructor(payload: AppErrorPayload) {
    super(payload.message);
    this.name = "AppError";
    this.kind = payload.kind;
    this.hint = payload.hint;
    this.detail = payload.detail;
  }
}

/** Narrows a rejected `invoke` value to the serialized `AppError` shape. */
export function isAppError(e: unknown): e is AppErrorPayload {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e && "hint" in e;
}

/** Converts anything a rejected `invoke` can throw into an `AppError`. */
export function toAppError(e: unknown): AppError {
  if (e instanceof AppError) return e;
  if (isAppError(e)) return new AppError(e);
  // Not from `AppError` (e.g. an unknown command or a bridge failure).
  const message = typeof e === "string" ? e : e instanceof Error ? e.message : "Unknown error";
  return new AppError({ kind: "Io", message, hint: "" });
}

/** Invokes a command; a rejection is always an `AppError`. */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw toAppError(e);
  }
}

// ---- settings ----

export function getSettings(): Promise<Settings> {
  return call<Settings>("get_settings");
}

/** Validates and saves; rejects with an `InvalidInput` error for bad values. */
export function saveSettings(settings: Settings): Promise<Settings> {
  return call<Settings>("save_settings", { settings });
}

/** Saves the mode the popup ended on to `lastPopupMode`. */
export function setLastPopupMode(mode: Mode): Promise<Settings> {
  return call<Settings>("set_last_popup_mode", { mode });
}

// ---- system ----

/** Why the popup hotkey isn't registered (taken, invalid), or `null` when it works. */
export function hotkeyError(): Promise<string | null> {
  return call<string | null>("hotkey_error");
}

// ---- presets ----

export function listPresets(): Promise<Preset[]> {
  return call<Preset[]>("list_presets");
}

/** Creates or updates a preset; resolves with all presets. */
export function savePreset(preset: Preset): Promise<Preset[]> {
  return call<Preset[]>("save_preset", { preset });
}

/** Deletes a preset (never the last one); resolves with all presets. */
export function deletePreset(id: string): Promise<Preset[]> {
  return call<Preset[]>("delete_preset", { id });
}

/** Restores the built-in presets, keeping user-created ones; resolves with all presets. */
export function resetPresets(): Promise<Preset[]> {
  return call<Preset[]>("reset_presets");
}

// ---- translation ----

/** Picks Translate or Correct from the text; `last` is used when the text is too short to tell. */
export function detectMode(text: string, last: Mode): Promise<Mode> {
  return call<Mode>("detect_mode", { text, last });
}

/** Runs one request; rejects with `Cancelled`, `Timeout`, `MalformedOutput`, ... errors. */
export function translate(req: TranslationRequest): Promise<TranslationResult> {
  return call<TranslationResult>("translate", { req });
}

/** Cancels the running request with this id; its `translate` call rejects with `Cancelled`. */
export function cancelTranslation(requestId: string): Promise<void> {
  return call<void>("cancel_translation", { requestId });
}

/** Ollama's and Claude Code's state. */
export function providerStatus(): Promise<ProviderStatus> {
  return call<ProviderStatus>("provider_status");
}

/** Accept with the Clipboard target: copies the text (Rust writes the clipboard). */
export function copyResult(text: string, historyId: number | null = null): Promise<void> {
  return call<void>("copy_result", { text, historyId });
}

// ---- history ----

/** Newest first. `query` searches source and output; `issue` keeps entries with that mistake. */
export function historyList(args: {
  query?: string | null;
  issue?: IssueFilter | null;
  limit: number;
  offset: number;
}): Promise<HistoryEntrySummary[]> {
  return call<HistoryEntrySummary[]>("history_list", {
    query: args.query ?? null,
    issue: args.issue ?? null,
    limit: args.limit,
    offset: args.offset,
  });
}

/** One entry with its issues; rejects with `InvalidInput` when it doesn't exist. */
export function historyGet(id: number): Promise<HistoryEntry> {
  return call<HistoryEntry>("history_get", { id });
}

export function historyDelete(id: number): Promise<void> {
  return call<void>("history_delete", { id });
}

/** Deletes the whole history. */
export function historyClear(): Promise<void> {
  return call<void>("history_clear");
}

/** The "my mistakes" aggregates for the last `sinceDays` days (all when null). */
export function mistakesSummary(sinceDays: number | null = null): Promise<MistakesSummary> {
  return call<MistakesSummary>("mistakes_summary", { sinceDays });
}

// ---- system ----

/** Round-trip check between the UI and the Rust core. */
export function ping(): Promise<string> {
  return call<string>("ping");
}

/** Native file dialog; `filter` is a list of extensions without dots. Null when cancelled. */
export function pickFile(filter: string[] | null = null): Promise<string | null> {
  return call<string | null>("pick_file", { filter });
}

/** Opens an allow-listed link (Ollama download, Claude Code install docs, releases). */
export const LINKS = {
  ollamaDownload: "https://ollama.com/download",
  claudeCodeSetup: "https://docs.claude.com/en/docs/claude-code/setup",
  releases: "https://github.com/misstheaim/prompt-translator/releases",
} as const;

export function openUrl(url: string): Promise<void> {
  return call<void>("open_url", { url });
}
