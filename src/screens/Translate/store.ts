import { create } from "zustand";

import type { Mode } from "../../bindings/Mode";
import type { ProviderKind } from "../../bindings/ProviderKind";
import type { TranslationResult } from "../../bindings/TranslationResult";
import { cancelTranslation, toAppError, translate, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";

export const MAX_INPUT_CHARS = 8000;

export type RunStatus = "idle" | "loading" | "success" | "error";

interface TranslateState {
  input: string;
  /** User overrides; `null` means "follow the settings / the selected preset". */
  mode: Mode | null;
  provider: ProviderKind | null;
  presetId: string | null;

  status: RunStatus;
  requestId: string | null;
  result: TranslationResult | null;
  /** The editable output; starts as `result.output`. */
  output: string;
  error: AppError | null;

  setInput: (input: string) => void;
  setMode: (mode: Mode) => void;
  setProvider: (provider: ProviderKind) => void;
  /** Selecting a preset also sets its default mode. */
  setPreset: (presetId: string, defaultMode: Mode) => void;
  setOutput: (output: string) => void;
  /** Runs the current input with the given effective selections. */
  run: (sel: { mode: Mode; provider: ProviderKind; presetId: string }) => Promise<void>;
  cancel: () => Promise<void>;
}

export const useTranslateStore = create<TranslateState>((set, get) => ({
  input: "",
  mode: null,
  provider: null,
  presetId: null,

  status: "idle",
  requestId: null,
  result: null,
  output: "",
  error: null,

  setInput: (input) => set({ input }),
  setMode: (mode) => set({ mode }),
  setProvider: (provider) => set({ provider }),
  setPreset: (presetId, defaultMode) => set({ presetId, mode: defaultMode }),
  setOutput: (output) => set({ output }),

  run: async ({ mode, provider, presetId }) => {
    const { input, status } = get();
    if (status === "loading" || input.trim() === "" || input.length > MAX_INPUT_CHARS) return;
    const requestId = crypto.randomUUID();
    set({ status: "loading", requestId, error: null });
    try {
      const result = await translate({
        requestId,
        sourceText: input,
        mode,
        provider,
        presetId,
      });
      if (get().requestId !== requestId) return;
      set({ status: "success", result, output: result.output, requestId: null });
    } catch (e) {
      if (get().requestId !== requestId) return;
      const error = toAppError(e);
      // A cancel the user asked for isn't an error to show.
      if (error.kind === "Cancelled") {
        set({ status: get().result ? "success" : "idle", requestId: null });
      } else {
        set({ status: "error", error, requestId: null });
      }
    }
  },

  cancel: async () => {
    const { requestId } = get();
    if (!requestId) return;
    try {
      await cancelTranslation(requestId);
    } catch (e) {
      useAppStore.getState().showToast("error", toAppError(e).message);
    }
  },
}));
