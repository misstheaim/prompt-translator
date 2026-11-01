import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import type { IssueCategory } from "../bindings/IssueCategory";
import type { IssueFilter } from "../bindings/IssueFilter";
import type { Preset } from "../bindings/Preset";
import type { ProviderStatus } from "../bindings/ProviderStatus";
import type { Settings } from "../bindings/Settings";
import {
  getSettings,
  hotkeyError,
  listPresets,
  providerStatus,
  setCompactWindow,
  toAppError,
  type AppError,
} from "../lib/api";

export const SCREENS = ["translate", "history", "mistakes", "settings"] as const;
export type Screen = (typeof SCREENS)[number];

/** What turned compact mode on: the button, or dragging the window to its minimum width or height. */
export type CompactSource = "button" | "resize";

/**
 * Normal-mode minimum window size (`NORMAL_MIN_SIZE` in Rust), and the margin above it, on both
 * sides, that leaves resize-entered compact mode.
 */
export const NORMAL_MIN_WIDTH = 640;
export const NORMAL_MIN_HEIGHT = 480;
export const COMPACT_EXIT_MARGIN = 40;
/** Window resizes within this long of a programmatic change are ignored by the auto rules. */
const RESIZE_SETTLE_MS = 400;
let ignoreResizeUntil = 0;
function markWindowResized() {
  ignoreResizeUntil = Date.now() + RESIZE_SETTLE_MS;
}
export function resizeIsSettled(): boolean {
  return Date.now() >= ignoreResizeUntil;
}

export type ToastKind = "info" | "success" | "error";
export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const TOAST_MS: Record<ToastKind, number> = { info: 3000, success: 3000, error: 6000 };
const MAX_TOASTS = 3;
let nextToastId = 1;

/**
 * A History filter coming from the Mistakes screen: a category or a pair, for a period, in the
 * Mistakes screen's target language.
 */
export interface HistoryFilter {
  issue: IssueFilter | null;
  category: IssueCategory | null;
  /** Last N days; null for all time. */
  sinceDays: number | null;
  /** ISO code of the results' language the History language filter starts with; null for all. */
  targetLanguage: string | null;
}

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

  /** What the History screen is filtered by (set from the Mistakes screen). */
  historyFilter: HistoryFilter | null;

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

  /** Compact mode of the Translate screen (TRANS-016): `null` off, else what turned it on. */
  compact: CompactSource | null;
  /**
   * Turns compact mode on, on the current Translate tab (the button also switches to the
   * Translate screen and resizes the window).
   */
  enterCompact: (by: CompactSource) => Promise<void>;
  /** Turns compact mode off and restores the window limits (and, for the button, the size). */
  exitCompact: () => Promise<void>;

  setScreen: (screen: Screen) => void;
  setHistoryFilter: (filter: HistoryFilter | null) => void;
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
  compact: null,
  enterCompact: async (by) => {
    if (get().compact) return;
    // On before the window shrinks, so the full screen never flashes at the small size.
    set({ compact: by, ...(by === "button" ? { screen: "translate" as const } : {}) });
    markWindowResized();
    try {
      await setCompactWindow(true, by === "button");
    } catch (e) {
      set({ compact: null });
      get().showToast("error", toAppError(e).message);
    }
  },
  exitCompact: async () => {
    const by = get().compact;
    if (!by) return;
    markWindowResized();
    try {
      await setCompactWindow(false, by === "button");
    } catch (e) {
      get().showToast("error", toAppError(e).message);
    }
    // Off only after the window is back, so the transient small size can't re-enter compact mode.
    markWindowResized();
    set({ compact: null });
  },
  settings: null,
  presets: [],
  providerStatus: null,
  loadError: null,
  toasts: [],
  hotkeyError: null,

  historyFilter: null,

  setupRequested: false,
  setupDismissed: false,
  openSetup: () => set({ setupRequested: true, setupDismissed: false }),
  leaveSetup: () => set({ setupRequested: false, setupDismissed: true }),
  finishSetup: () => set({ setupRequested: false, setupDismissed: false }),

  setScreen: (screen) => set({ screen }),
  setHistoryFilter: (historyFilter) => set({ historyFilter }),
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
