import type { Mode } from "../../bindings/Mode";
import type { Preset } from "../../bindings/Preset";
import type { ProviderKind } from "../../bindings/ProviderKind";
import type { Settings } from "../../bindings/Settings";
import type { LanguagePair } from "./store";

/** A tab's own choices; `null` follows the settings (or the selected preset, for the mode). */
export interface TabChoices {
  mode: Mode | null;
  provider: ProviderKind | null;
  presetId: string | null;
  pair: LanguagePair | null;
}

/** What a tab runs with: its choices, else the settings' defaults. */
export interface EffectiveSelection {
  provider: ProviderKind;
  /** The preset in use; `undefined` only while no presets are loaded. */
  preset: Preset | undefined;
  /** `preset.id`, else the id the tab asks for (`""` when there is none). */
  presetId: string;
  /** The mode before the bound-tab rule (Improve is off there) is applied. */
  mode: Mode;
  pair: LanguagePair;
}

/** The providers the Translate screen offers (Mock only in dev builds or when turned on). */
export function offeredProviders(settings: Settings | null): ProviderKind[] {
  const showMock = import.meta.env.DEV || settings?.devMockProvider === true;
  return showMock ? ["ollama", "claudeCode", "mock"] : ["ollama", "claudeCode"];
}

/**
 * Resolves a tab's choices against the settings and presets. A choice that is no longer valid
 * (a deleted preset, a provider that isn't offered) falls back to the settings' default.
 */
export function effectiveSelection(
  tab: TabChoices,
  settings: Settings | null,
  presets: Preset[],
): EffectiveSelection {
  const defaultProvider = settings?.defaultProvider ?? "ollama";
  const provider =
    tab.provider !== null && offeredProviders(settings).includes(tab.provider)
      ? tab.provider
      : defaultProvider;
  const wantedPreset = tab.presetId ?? settings?.defaultPresetId ?? presets[0]?.id ?? "";
  const preset =
    presets.find((p) => p.id === wantedPreset) ??
    presets.find((p) => p.id === settings?.defaultPresetId) ??
    presets[0];
  return {
    provider,
    preset,
    presetId: preset?.id ?? wantedPreset,
    mode: tab.mode ?? preset?.defaultMode ?? "translate",
    pair: tab.pair ?? {
      native: settings?.nativeLanguage ?? "ru",
      target: settings?.targetLanguage ?? "en",
    },
  };
}
