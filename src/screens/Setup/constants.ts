import type { BenchmarkResult } from "../../bindings/BenchmarkResult";
import type { ModelTier } from "../../bindings/ModelTier";

/** Benchmark thresholds (plan T36): a slower local model is flagged and Claude Code preselected. */
export const SLOW_TRANSLATE_MS = 8_000;
export const SLOW_CORRECT_MS = 20_000;
/** Below this share of the model in GPU memory, a machine guessed as `gpu-*` is probably CPU-bound. */
export const LOW_GPU_OFFLOAD_PERCENT = 50;

export const WIZARD_STEPS = ["welcome", "local", "claude", "defaults", "done"] as const;
export type WizardStep = (typeof WIZARD_STEPS)[number];

export const STEP_TITLES: Record<WizardStep, string> = {
  welcome: "Welcome",
  local: "Local model",
  claude: "Claude Code",
  defaults: "Defaults",
  done: "Done",
};

export interface TierRow {
  tier: Exclude<ModelTier, "none">;
  label: string;
  /** Mirrors the Rust tier table (`setup/model_tiers.rs`), used for the selectable alternatives. */
  model: string;
  downloadMb: number;
}

/** Lowest to highest. */
export const TIER_ROWS: TierRow[] = [
  { tier: "gpu-small", label: "Small", model: "qwen3.5:4b", downloadMb: 3_300 },
  { tier: "gpu-medium", label: "Medium", model: "qwen3.5:9b", downloadMb: 6_600 },
  { tier: "gpu-large", label: "Large", model: "qwen3.5:9b", downloadMb: 6_600 },
];

/** Position in the tier order; the `none` tier is below every row. */
export function tierRank(tier: ModelTier): number {
  return TIER_ROWS.findIndex((r) => r.tier === tier);
}

export function formatSize(mb: number): string {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

export function formatSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} s`;
}

/** Why the local model is too slow (empty when it is fine). */
export function benchmarkProblems(result: BenchmarkResult, guessedGpu: boolean): string[] {
  const problems: string[] = [];
  if (result.translateMs > SLOW_TRANSLATE_MS) {
    problems.push(
      `Translate took ${formatSeconds(result.translateMs)} (more than ${SLOW_TRANSLATE_MS / 1000} s).`,
    );
  }
  if (result.correctMs > SLOW_CORRECT_MS) {
    problems.push(
      `Correct took ${formatSeconds(result.correctMs)} (more than ${SLOW_CORRECT_MS / 1000} s).`,
    );
  }
  if (guessedGpu && result.gpuOffloadPercent < LOW_GPU_OFFLOAD_PERCENT) {
    problems.push(
      `Only ${Math.round(result.gpuOffloadPercent)}% of the model runs on the graphics card.`,
    );
  }
  return problems;
}
