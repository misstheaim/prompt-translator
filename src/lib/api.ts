// Typed wrappers around Tauri commands. The UI never calls `invoke` directly (plan D3).
// One section per Rust module; each backend task adds its own section.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AppErrorPayload } from "../bindings/AppErrorPayload";
import type { AppInfo } from "../bindings/AppInfo";
import type { UpdateInfo } from "../bindings/UpdateInfo";
import type { UpdateProgress } from "../bindings/UpdateProgress";
import type { BenchmarkResult } from "../bindings/BenchmarkResult";
import type { ErrorKind } from "../bindings/ErrorKind";
import type { EnvironmentReport } from "../bindings/EnvironmentReport";
import type { HistoryEntry } from "../bindings/HistoryEntry";
import type { HistoryEntrySummary } from "../bindings/HistoryEntrySummary";
import type { IssueCategory } from "../bindings/IssueCategory";
import type { IssueFilter } from "../bindings/IssueFilter";
import type { Language } from "../bindings/Language";
import type { MistakesSummary } from "../bindings/MistakesSummary";
import type { Mode } from "../bindings/Mode";
import type { Preset } from "../bindings/Preset";
import type { ProviderStatus } from "../bindings/ProviderStatus";
import type { Settings } from "../bindings/Settings";
import type { SetupProgress } from "../bindings/SetupProgress";
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

/**
 * Picks Translate or Correct from the text (Correct when it is in the target language); `last`
 * is used when the text is too short to tell. The pair defaults to the settings' one.
 */
export function detectMode(
  text: string,
  last: Mode,
  pair: { nativeLanguage?: string | null; targetLanguage?: string | null } = {},
): Promise<Mode> {
  return call<Mode>("detect_mode", {
    text,
    last,
    nativeLanguage: pair.nativeLanguage ?? null,
    targetLanguage: pair.targetLanguage ?? null,
  });
}

/** The languages that can be the native or the target language, sorted by English name. */
export function listLanguages(): Promise<Language[]> {
  return call<Language[]>("list_languages");
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

/**
 * Newest first. `query` searches source and output; `issue` keeps entries with that mistake;
 * `mode`, `category` (entries with an issue of it) and `sinceDays` narrow further.
 */
export function historyList(args: {
  query?: string | null;
  issue?: IssueFilter | null;
  mode?: Mode | null;
  category?: IssueCategory | null;
  sinceDays?: number | null;
  limit: number;
  offset: number;
}): Promise<HistoryEntrySummary[]> {
  return call<HistoryEntrySummary[]>("history_list", {
    query: args.query ?? null,
    issue: args.issue ?? null,
    mode: args.mode ?? null,
    category: args.category ?? null,
    sinceDays: args.sinceDays ?? null,
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

// ---- setup ----

/** GPU, RAM, Ollama, winget and Claude Code status, plus the recommended model tier. */
export function environmentCheck(): Promise<EnvironmentReport> {
  return call<EnvironmentReport>("environment_check");
}

/**
 * Installs Ollama with winget; output arrives as `setup://progress` (see `onSetupProgress`).
 * Rejects with `InvalidInput` when winget is missing: open `LINKS.ollamaDownload` then.
 */
export function ollamaInstall(): Promise<void> {
  return call<void>("ollama_install");
}

/** Starts Ollama if needed and resolves once it answers (up to 30 s). */
export function ollamaStart(): Promise<void> {
  return call<void>("ollama_start");
}

/** Downloads a model and selects it as the Ollama model; progress via `onSetupProgress`. */
export function ollamaPull(model: string): Promise<void> {
  return call<void>("ollama_pull", { model });
}

/** Times a Translate and a Correct run with `model` and reads the GPU offload. */
export function ollamaBenchmark(model: string): Promise<BenchmarkResult> {
  return call<BenchmarkResult>("ollama_benchmark", { model });
}

/** Cancels the running install wait, Ollama start or model download. */
export function cancelSetupTask(): Promise<void> {
  return call<void>("cancel_setup_task");
}

/** Marks the first-run setup as done. */
export function completeSetup(): Promise<void> {
  return call<void>("complete_setup");
}

/** Listens to `setup://progress`; resolves with the function that stops listening. */
export function onSetupProgress(handler: (progress: SetupProgress) => void): Promise<UnlistenFn> {
  return listen<SetupProgress>("setup://progress", (event) => handler(event.payload));
}

// ---- updater and app info ----

/** Version and the config and data folder paths. */
export function appInfo(): Promise<AppInfo> {
  return call<AppInfo>("app_info");
}

/** Opens the config or the data folder in Explorer. */
export function openFolder(folder: "config" | "data"): Promise<void> {
  return call<void>("open_folder", { folder });
}

/** Asks Ollama to unload the model: `true` unloaded, `false` nothing was loaded (or Ollama is off). */
export function unloadModel(): Promise<boolean> {
  return call<boolean>("unload_model");
}

/** Whether this build has an uninstaller (false in dev runs). */
export function uninstallerAvailable(): Promise<boolean> {
  return call<boolean>("uninstaller_available");
}

/** Starts the uninstaller and exits the app. */
export function runUninstaller(): Promise<void> {
  return call<void>("run_uninstaller");
}

/** Asks GitHub for a newer release. Rejects when offline; show the error to the user. */
export function checkUpdate(): Promise<UpdateInfo> {
  return call<UpdateInfo>("check_update");
}

/** Downloads and installs the newest release; the app exits and the installer restarts it. */
export function installUpdate(): Promise<void> {
  return call<void>("install_update");
}

/** Listens to `update://available` (sent once, 10 s after startup, if a release is newer). */
export function onUpdateAvailable(handler: (info: UpdateInfo) => void): Promise<UnlistenFn> {
  return listen<UpdateInfo>("update://available", (event) => handler(event.payload));
}

/** Listens to `update://progress` while `installUpdate` downloads. */
export function onUpdateProgress(handler: (progress: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>("update://progress", (event) => handler(event.payload));
}
