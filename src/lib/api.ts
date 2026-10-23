// Typed wrappers around Tauri commands. The UI never calls `invoke` directly (plan D3).
// One section per Rust module; each backend task adds its own section.
import { invoke } from "@tauri-apps/api/core";

import type { AppErrorPayload } from "../bindings/AppErrorPayload";
import type { Settings } from "../bindings/Settings";

// ---- settings ----

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/** Validates and saves; rejects with an `InvalidInput` error for bad values. */
export function saveSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("save_settings", { settings });
}

// ---- system ----

/** Round-trip check between the UI and the Rust core. */
export function ping(): Promise<string> {
  return invoke<string>("ping");
}

/** Narrows a rejected `invoke` value to the serialized `AppError` shape. */
export function isAppError(e: unknown): e is AppErrorPayload {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e &&
    "hint" in e
  );
}
