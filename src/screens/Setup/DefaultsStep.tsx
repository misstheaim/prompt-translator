import { useEffect, useState } from "react";

import type { ProviderKind } from "../../bindings/ProviderKind";
import { Banner } from "../../components/Banner";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Select } from "../../components/Select";
import { saveSettings, toAppError, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { HotkeyField, Section, Toggle } from "../Settings/fields";
import { workingProviders } from "./providers";
import { StepNav } from "./StepNav";
import { useSetupStore } from "./store";

const PROVIDER_LABELS: Record<ProviderKind, string> = {
  ollama: "Ollama (local model)",
  claudeCode: "Claude Code",
  mock: "Mock",
};

export function DefaultsStep() {
  const settings = useAppStore((s) => s.settings);
  const status = useAppStore((s) => s.providerStatus);
  const refresh = useAppStore((s) => s.refreshProviderStatus);
  const preferClaude = useSetupStore((s) => s.preferClaude);
  const next = useSetupStore((s) => s.next);

  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [hotkey, setHotkey] = useState<string | null>(null);
  const [history, setHistory] = useState<boolean | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!settings) return null;

  const working = workingProviders(status);
  const options: { value: ProviderKind; label: string }[] = [];
  if (working.ollama) options.push({ value: "ollama", label: PROVIDER_LABELS.ollama });
  if (working.claudeCode) options.push({ value: "claudeCode", label: PROVIDER_LABELS.claudeCode });

  // The preselection: Claude Code when the local model is slow (or not set up) and Claude works.
  let suggested: ProviderKind = settings.defaultProvider;
  if (working.claudeCode && (preferClaude || !working.ollama)) suggested = "claudeCode";
  else if (working.ollama) suggested = "ollama";
  const effective = provider ?? suggested;

  async function save() {
    if (!settings) return;
    setSaving(true);
    setError(null);
    try {
      // Read the settings again: the model download changed `ollama.model` in the meantime.
      const current = useAppStore.getState().settings ?? settings;
      const saved = await saveSettings({
        ...current,
        defaultProvider: options.length > 0 ? effective : current.defaultProvider,
        hotkey: hotkey ?? current.hotkey,
        historyEnabled: history ?? current.historyEnabled,
        autostart: autostart ?? current.autostart,
      });
      useAppStore.getState().setSettings(saved);
      next();
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {error && <ErrorBanner error={error} showSettingsAction={false} />}
      <Section title="Default provider" description="Used when you open the app or the popup.">
        {options.length > 0 ? (
          <Select
            label="Provider"
            value={effective}
            options={options}
            onChange={(v) => setProvider(v as ProviderKind)}
          />
        ) : (
          <Banner variant="warning">
            No provider works yet. Go back to set up the local model or Claude Code; you can also do
            it later in Settings.
          </Banner>
        )}
      </Section>
      <Section title="Hotkey and startup">
        <HotkeyField
          label="Global hotkey"
          value={hotkey ?? settings.hotkey}
          onChange={setHotkey}
          hint="Opens the translation popup from any app. Click the field and press the combination."
        />
        <Toggle
          label="Start with Windows"
          description="Runs in the tray after you sign in."
          checked={autostart ?? settings.autostart}
          onChange={setAutostart}
        />
      </Section>
      <Section title="History">
        <Toggle
          label="Save translations to history"
          description="Needed for the history and the mistakes log."
          checked={history ?? settings.historyEnabled}
          onChange={setHistory}
        />
      </Section>
      <StepNav onNext={() => void save()} nextLoading={saving} />
    </div>
  );
}
