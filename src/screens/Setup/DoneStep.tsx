import { useEffect, useState } from "react";

import { Banner } from "../../components/Banner";
import { ErrorBanner } from "../../components/ErrorBanner";
import { completeSetup, toAppError, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { workingProviders } from "./providers";
import { StepNav } from "./StepNav";
import { useSetupStore } from "./store";

export function DoneStep() {
  const status = useAppStore((s) => s.providerStatus);
  const refresh = useAppStore((s) => s.refreshProviderStatus);
  const [finishing, setFinishing] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const working = workingProviders(status);
  const anyWorking = working.ollama || working.claudeCode;

  async function finish() {
    setFinishing(true);
    setError(null);
    try {
      await completeSetup();
      useSetupStore.getState().reset();
      const app = useAppStore.getState();
      app.finishSetup();
      app.setScreen("translate");
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setFinishing(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {error && <ErrorBanner error={error} showSettingsAction={false} />}
      {anyWorking ? (
        <Banner variant="success">
          You&apos;re all set.{" "}
          {[working.ollama && "Ollama", working.claudeCode && "Claude Code"]
            .filter(Boolean)
            .join(" and ")}{" "}
          {working.ollama && working.claudeCode ? "are" : "is"} ready to translate.
        </Banner>
      ) : (
        <Banner variant="warning">
          No provider works yet, so translating will fail until you set up the local model or sign
          in to Claude Code. You can go back now, or finish and do it later in Settings.
        </Banner>
      )}
      <p className="text-sm text-muted">
        You can run this setup again any time from Settings, About.
      </p>
      <StepNav nextLabel="Finish" onNext={() => void finish()} nextLoading={finishing} />
    </div>
  );
}
