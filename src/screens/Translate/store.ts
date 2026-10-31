import { create } from "zustand";

import type { Mode } from "../../bindings/Mode";
import type { ProviderKind } from "../../bindings/ProviderKind";
import type { TranslationResult } from "../../bindings/TranslationResult";
import { cancelTranslation, toAppError, translate, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";

export const MAX_INPUT_CHARS = 8000;

/** ISO 639-1 codes: the language the user writes in, and the one the result is in. */
export interface LanguagePair {
  native: string;
  target: string;
}

export type RunStatus = "idle" | "loading" | "success" | "error";

interface TranslateState {
  input: string;
  /** User overrides; `null` means "follow the settings / the selected preset". */
  mode: Mode | null;
  provider: ProviderKind | null;
  presetId: string | null;
  /** The session's language pair; `null` follows the settings' pair. Never saved. */
  pair: LanguagePair | null;

  status: RunStatus;
  requestId: string | null;
  result: TranslationResult | null;
  /** The editable output; starts as `result.output`. */
  output: string;
  /** The source text and mode `result` was produced from (the input may change afterwards). */
  resultSource: string;
  resultMode: Mode | null;
  error: AppError | null;

  setInput: (input: string) => void;
  setMode: (mode: Mode) => void;
  setProvider: (provider: ProviderKind) => void;
  /** Selecting a preset also sets its default mode. */
  setPreset: (presetId: string, defaultMode: Mode) => void;
  setOutput: (output: string) => void;
  setPair: (pair: LanguagePair) => void;
  /** Runs the current input with the given effective selections. */
  run: (sel: {
    mode: Mode;
    provider: ProviderKind;
    presetId: string;
    pair: LanguagePair;
  }) => Promise<void>;
  cancel: () => Promise<void>;
}

export const useTranslateStore = create<TranslateState>((set, get) => ({
  input: "",
  mode: null,
  provider: null,
  presetId: null,
  pair: null,

  status: "idle",
  requestId: null,
  result: null,
  output: "",
  resultSource: "",
  resultMode: null,
  error: null,

  setInput: (input) => set({ input }),
  setMode: (mode) => set({ mode }),
  setProvider: (provider) => set({ provider }),
  setPreset: (presetId, defaultMode) => set({ presetId, mode: defaultMode }),
  setOutput: (output) => set({ output }),

  setPair: (pair) => set({ pair }),

  run: async ({ mode, provider, presetId, pair }) => {
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
        nativeLanguage: pair.native,
        targetLanguage: pair.target,
      });
      if (get().requestId !== requestId) return;
      set({
        status: "success",
        result,
        output: result.output,
        resultSource: input,
        resultMode: mode,
        requestId: null,
      });
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
