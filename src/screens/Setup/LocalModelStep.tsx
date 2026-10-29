import { useCallback, useEffect, useState } from "react";

import type { EnvironmentReport } from "../../bindings/EnvironmentReport";
import type { ModelTier } from "../../bindings/ModelTier";
import type { SetupProgress } from "../../bindings/SetupProgress";
import { Banner } from "../../components/Banner";
import { Button } from "../../components/Button";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import {
  cancelSetupTask,
  environmentCheck,
  LINKS,
  ollamaBenchmark,
  ollamaInstall,
  ollamaPull,
  ollamaStart,
  onSetupProgress,
  openUrl,
  toAppError,
  type AppError,
} from "../../lib/api";
import { useAppStore } from "../../store/app";
import { benchmarkProblems, formatSeconds, formatSize, TIER_ROWS, tierRank } from "./constants";
import { StepNav } from "./StepNav";
import { useSetupStore } from "./store";

type Task = "install" | "start" | "download" | "benchmark";

const TASK_LABELS: Record<Task, string> = {
  install: "Installing Ollama...",
  start: "Starting Ollama...",
  download: "Downloading the model...",
  benchmark: "Measuring speed (this loads the model and runs two short texts)...",
};

/** "No supported graphics card, ..." or the card found, in plain words. */
function describeMachine(report: EnvironmentReport): string {
  const ram = `${Math.round(report.ramMb / 1024)} GB of memory`;
  if (!report.usableGpu) {
    return `No supported graphics card (${ram}), so the local model isn't recommended: use Claude Code. You can still set up Ollama anyway.`;
  }
  const gpu = report.gpus.find((g) => g.usable) ?? report.gpus[0];
  return `Found ${gpu.name} with ${Math.round(gpu.vramMb / 1024)} GB of video memory (${ram}).`;
}

function ProgressBar({ progress }: { progress: SetupProgress | null }) {
  const total = progress?.totalBytes ?? 0;
  const done = progress?.completedBytes ?? 0;
  const percent = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : null;
  return (
    <div className="flex flex-col gap-1">
      {percent !== null ? (
        <progress
          className="h-2 w-full accent-accent"
          value={percent}
          max={100}
          aria-label="Download progress"
        />
      ) : (
        <progress className="h-2 w-full accent-accent" aria-label="Working" />
      )}
      <p className="text-xs text-muted">
        {percent !== null
          ? `${percent}% (${formatSize(Math.round(done / 1_000_000))} of ${formatSize(Math.round(total / 1_000_000))})`
          : (progress?.message ?? progress?.status ?? "Working...")}
      </p>
    </div>
  );
}

export function LocalModelStep() {
  const showToast = useAppStore((s) => s.showToast);
  const refreshProviderStatus = useAppStore((s) => s.refreshProviderStatus);
  const selectedTier = useSetupStore((s) => s.selectedTier);
  const setUpAnyway = useSetupStore((s) => s.setUpAnyway);
  const benchmark = useSetupStore((s) => s.benchmark);
  const patch = useSetupStore((s) => s.patch);

  const [report, setReport] = useState<EnvironmentReport | null>(null);
  const [checkError, setCheckError] = useState<AppError | null>(null);
  const [task, setTask] = useState<Task | null>(null);
  const [progress, setProgress] = useState<SetupProgress | null>(null);
  const [taskError, setTaskError] = useState<AppError | null>(null);

  const check = useCallback(async () => {
    setCheckError(null);
    try {
      setReport(await environmentCheck());
    } catch (e) {
      setCheckError(toAppError(e));
    }
  }, []);

  useEffect(() => {
    environmentCheck().then(
      (r) => setReport(r),
      (e: unknown) => setCheckError(toAppError(e)),
    );
  }, []);

  useEffect(() => {
    const unlisten = onSetupProgress(setProgress);
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  if (checkError) {
    return (
      <div className="flex flex-col gap-4">
        <ErrorBanner error={checkError} onRetry={() => void check()} showSettingsAction={false} />
        <StepNav nextLabel="Skip" nextVariant="secondary" />
      </div>
    );
  }
  if (!report) {
    return (
      <div className="flex items-center gap-2 py-6 text-sm text-muted">
        <Spinner /> Checking this computer...
      </div>
    );
  }

  const recommended = report.recommendation;
  const showLocal = report.usableGpu || setUpAnyway;
  const tier: ModelTier =
    selectedTier ?? (recommended.tier !== "none" ? recommended.tier : "gpu-small");
  const row = TIER_ROWS.find((r) => r.tier === tier) ?? TIER_ROWS[0];
  const aboveMachine = tierRank(tier) > tierRank(recommended.tier);
  const modelInstalled = report.ollama.models.includes(row.model);
  const ready = report.ollama.running && modelInstalled && benchmark !== null;
  const problems = benchmark ? benchmarkProblems(benchmark, recommended.tier !== "none") : [];

  async function runTask(name: Task, fn: () => Promise<void>) {
    setTask(name);
    setTaskError(null);
    setProgress(null);
    try {
      await fn();
    } catch (e) {
      const err = toAppError(e);
      if (err.kind !== "Cancelled") setTaskError(err);
    } finally {
      setTask(null);
      setProgress(null);
      await check();
      void refreshProviderStatus();
    }
  }

  async function measure(model: string) {
    const result = await ollamaBenchmark(model);
    const slow = benchmarkProblems(result, recommended.tier !== "none").length > 0;
    patch({ benchmark: result, preferClaude: slow });
  }

  function install() {
    if (!report?.winget) {
      openUrl(LINKS.ollamaDownload).catch((e: unknown) =>
        showToast("error", toAppError(e).message),
      );
      return;
    }
    void runTask("install", ollamaInstall);
  }

  function download() {
    void runTask("download", async () => {
      patch({ benchmark: null, preferClaude: false });
      await ollamaPull(row.model);
      setTask("benchmark");
      await measure(row.model);
    });
  }

  function runBenchmark() {
    void runTask("benchmark", () => measure(row.model));
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-text">{describeMachine(report)}</p>

      {!report.usableGpu && !setUpAnyway && (
        <Banner
          variant="info"
          actions={
            <Button size="sm" onClick={() => patch({ setUpAnyway: true })}>
              Set up anyway
            </Button>
          }
        >
          Claude Code works well on this computer. Continue to set it up, or install the local model
          anyway (it can be slow without a graphics card).
        </Banner>
      )}

      {showLocal && (
        <div className="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4">
          <div className="flex flex-wrap items-center gap-3">
            <Select
              label="Model size"
              value={tier}
              disabled={task !== null}
              options={TIER_ROWS.map((r) => ({
                value: r.tier,
                label: `${r.label}: ${r.model}, ${formatSize(r.downloadMb)}${r.tier === recommended.tier ? " (recommended)" : ""}`,
              }))}
              onChange={(v) => patch({ selectedTier: v as ModelTier, benchmark: null })}
            />
          </div>
          {recommended.model && (
            <p className="text-xs text-muted">
              Recommended for this computer: {recommended.model}
              {recommended.downloadMb ? ` (${formatSize(recommended.downloadMb)} download)` : ""}.
            </p>
          )}
          {aboveMachine && (
            <Banner variant="warning">
              This model is larger than this computer is expected to handle. It may be slow or run
              out of memory.
            </Banner>
          )}

          {task ? (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2 text-sm text-text">
                <Spinner /> {TASK_LABELS[task]}
              </div>
              {(task === "download" || task === "install") && <ProgressBar progress={progress} />}
              {task !== "benchmark" && (
                <div>
                  <Button size="sm" onClick={() => void cancelSetupTask()}>
                    Cancel
                  </Button>
                </div>
              )}
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-2">
              {!report.ollama.installed ? (
                <>
                  <Button variant="primary" onClick={install}>
                    {report.winget ? "Install Ollama" : "Open the Ollama download page"}
                  </Button>
                  <Button onClick={() => void check()}>Check again</Button>
                </>
              ) : !report.ollama.running ? (
                <Button variant="primary" onClick={() => void runTask("start", ollamaStart)}>
                  Start Ollama
                </Button>
              ) : !modelInstalled ? (
                <Button variant="primary" onClick={download}>
                  Download {row.model} ({formatSize(row.downloadMb)})
                </Button>
              ) : benchmark === null ? (
                <Button variant="primary" onClick={runBenchmark}>
                  Measure speed
                </Button>
              ) : (
                <Button onClick={runBenchmark}>Measure again</Button>
              )}
            </div>
          )}

          {!report.ollama.installed && !report.winget && !task && (
            <p className="text-xs text-muted">
              Windows Package Manager (winget) isn&apos;t available, so install Ollama from its
              website, then choose &quot;Check again&quot;.
            </p>
          )}
          {taskError && (
            <ErrorBanner
              error={taskError}
              showSettingsAction={false}
              onRetry={
                taskError.kind === "OllamaUnreachable" ? undefined : () => setTaskError(null)
              }
            />
          )}

          {benchmark && !task && (
            <div className="flex flex-col gap-2">
              <p className="text-sm text-text">
                Translate: {formatSeconds(benchmark.translateMs)} · Correct:{" "}
                {formatSeconds(benchmark.correctMs)} · On the graphics card:{" "}
                {Math.round(benchmark.gpuOffloadPercent)}%
              </p>
              {problems.length > 0 ? (
                <Banner variant="warning">
                  <p>{problems.join(" ")}</p>
                  <p className="text-muted">
                    The local model is slow here, so Claude Code will be preselected as your default
                    provider if it&apos;s available.
                  </p>
                </Banner>
              ) : (
                <Banner variant="success">The local model is fast enough on this computer.</Banner>
              )}
            </div>
          )}
        </div>
      )}

      <StepNav
        nextLabel={ready ? "Continue" : "Skip"}
        nextVariant={ready ? "primary" : "secondary"}
        nextDisabled={task !== null}
      />
    </div>
  );
}
