import { create } from "zustand";

import type { BenchmarkResult } from "../../bindings/BenchmarkResult";
import type { ModelTier } from "../../bindings/ModelTier";
import { WIZARD_STEPS, type WizardStep } from "./constants";

/** The wizard's progress. Kept here so leaving the wizard and coming back resumes it. */
interface SetupState {
  step: WizardStep;
  /** Tier picked in the Local model step; `null` until the user or the recommendation sets it. */
  selectedTier: ModelTier | null;
  /** The user chose "Set up anyway" on a machine without a usable GPU. */
  setUpAnyway: boolean;
  benchmark: BenchmarkResult | null;
  /** The benchmark (or the tier) says Claude Code should be the default provider. */
  preferClaude: boolean;

  setStep: (step: WizardStep) => void;
  next: () => void;
  back: () => void;
  patch: (patch: Partial<Omit<SetupState, "setStep" | "next" | "back" | "reset">>) => void;
  reset: () => void;
}

const INITIAL = {
  step: "welcome" as WizardStep,
  selectedTier: null,
  setUpAnyway: false,
  benchmark: null,
  preferClaude: false,
};

export const useSetupStore = create<SetupState>((set, get) => ({
  ...INITIAL,
  setStep: (step) => set({ step }),
  next: () => {
    const i = WIZARD_STEPS.indexOf(get().step);
    set({ step: WIZARD_STEPS[Math.min(i + 1, WIZARD_STEPS.length - 1)] });
  },
  back: () => {
    const i = WIZARD_STEPS.indexOf(get().step);
    set({ step: WIZARD_STEPS[Math.max(i - 1, 0)] });
  },
  patch: (patch) => set(patch),
  reset: () => set(INITIAL),
}));
