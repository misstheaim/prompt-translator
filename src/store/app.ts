import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import type { IssueFilter } from "../bindings/IssueFilter";
import type { Preset } from "../bindings/Preset";
import type { ProviderStatus } from "../bindings/ProviderStatus";
import type { Settings } from "../bindings/Settings";
import {
  getSettings,
  hotkeyError,
  listPresets,
  providerStatus,
  toAppError,
  type AppError,
} from "../lib/api";

export const SCREENS = ["translate", "history", "mistakes", "settings"] as const;
export type Screen = (typeof SCREENS)[number];

export type ToastKind = "info" | "success" | "error";
export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const TOAST_MS: Record<ToastKind, number> = { info: 3000, success: 3000, error: 6000 };
const MAX_TOASTS = 3;
let nextToastId = 1;

interface AppState {
  screen: Screen;
  settings: Settings | null;
  presets: Preset[];
  providerStatus: ProviderStatus | null;
  /** Set when loading settings failed; the shell shows it as a banner. */
  loadError: AppError | null;
  toasts: ToastItem[];
  /** Why the popup hotkey isn't registered; the shell shows it as a banner. */
  hotkeyError: string | null;

  /** Mistake the History screen is filtered by (set from the Mistakes screen). */
  historyIssue: IssueFilter | null;

  /** The wizard was opened on purpose ("Run setup again") although setup is completed. */
  setupRequested: boolean;
  /** The user left the first-run wizard; the shell shows a "Resume setup" banner instead. */
  setupDismissed: boolean;
  /** Opens the setup wizard (Settings → About "Run setup again", or the resume banner). */
  openSetup: () => void;
  /** Leaves the wizard without finishing; its step is kept for resuming. */
  leaveSetup: () => void;
  /** Called when the wizard finishes. */
  finishSetup: () => void;

  setScreen: (screen: Screen) => void;
  setHistoryIssue: (issue: IssueFilter | null) => void;
  /** Replaces the settings (after `save_settings`, or from `settings://changed`). */
  setSettings: (settings: Settings) => void;
  setPresets: (presets: Preset[]) => void;
  loadSettings: () => Promise<void>;
  loadPresets: () => Promise<void>;
  /** Re-checks Ollama and Claude Code; never throws (a failed check leaves the old status). */
  refreshProviderStatus: () => Promise<void>;
  showToast: (kind: ToastKind, message: string) => void;
  dismissToast: (id: number) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  screen: "translate",
  settings: null,
  presets: [],
  providerStatus: null,
  loadError: null,
  toasts: [],
  hotkeyError: null,

  historyIssue: null,

  setupRequested: false,
  setupDismissed: false,
  openSetup: () => set({ setupRequested: true, setupDismissed: false }),
  leaveSetup: () => set({ setupRequested: false, setupDismissed: true }),
  finishSetup: () => set({ setupRequested: false, setupDismissed: false }),

  setScreen: (screen) => set({ screen }),
  setHistoryIssue: (historyIssue) => set({ historyIssue }),
  setSettings: (settings) => set({ settings }),
  setPresets: (presets) => set({ presets }),

  loadSettings: async () => {
    try {
      set({ settings: await getSettings(), loadError: null });
    } catch (e) {
      set({ loadError: toAppError(e) });
    }
  },

  loadPresets: async () => {
    try {
      set({ presets: await listPresets() });
    } catch (e) {
      get().showToast("error", toAppError(e).message);
    }
  },

  refreshProviderStatus: async () => {
    try {
      set({ providerStatus: await providerStatus() });
    } catch {
      // Keep the last known status; the next refresh retries.
    }
  },

  showToast: (kind, message) => {
    const id = nextToastId++;
    set((s) => ({ toasts: [...s.toasts, { id, kind, message }].slice(-MAX_TOASTS) }));
    setTimeout(() => get().dismissToast(id), TOAST_MS[kind]);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/**
 * Loads settings, presets and provider status, then refreshes them whenever
 * `settings://changed` fires. Returns the unsubscribe function.
 */
export async function initAppStore(): Promise<() => void> {
  const store = useAppStore.getState();
  const unlisten = await listen<Settings>("settings://changed", (event) => {
    const s = useAppStore.getState();
    s.setSettings(event.payload);
    void s.refreshProviderStatus();
  });
  // A registration failure at startup happens before this window listens, so ask once too.
  const unlistenHotkey = await listen<string | null>("hotkey://error", (event) => {
    useAppStore.setState({ hotkeyError: event.payload });
  });
  await Promise.all([
    store.loadSettings(),
    store.loadPresets(),
    store.refreshProviderStatus(),
    hotkeyError().then(
      (message) => useAppStore.setState({ hotkeyError: message }),
      () => undefined,
    ),
  ]);
  return () => {
    unlisten();
    unlistenHotkey();
  };
}
