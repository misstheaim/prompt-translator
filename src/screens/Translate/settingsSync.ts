import type { Preset } from "../../bindings/Preset";
import type { Settings } from "../../bindings/Settings";
import { useAppStore } from "../../store/app";
import { effectiveSelection, offeredProviders, type TabChoices } from "./selection";
import { useSessionTabs } from "./sessionTabs";
import { useTranslateStore } from "./store";

interface Defaults {
  settings: Settings | null;
  presets: Preset[];
}

/**
 * Applies new settings and presets to every open Translate tab (the default tab and the bound
 * ones), changing only what the new values make necessary:
 * - a changed language pair replaces the pair each tab picked;
 * - a changed default provider or preset leaves what the open tabs show selected (the new
 *   default is for new tabs and the next start);
 * - a choice that is no longer valid (a provider no longer offered, a deleted preset) falls back
 *   to the current default, and a tab whose preset falls back keeps its mode.
 * Text, results, edited output and a running request are never touched.
 */
function applyToTabs(next: Defaults, prev: Defaults) {
  const before = prev.settings;
  const after = next.settings;
  // The first load isn't a change: the tabs follow the settings from the start.
  if (!before || !after) return;

  const pairChanged =
    before.nativeLanguage !== after.nativeLanguage ||
    before.targetLanguage !== after.targetLanguage;
  const providerDefaultChanged = before.defaultProvider !== after.defaultProvider;
  const presetDefaultChanged = before.defaultPresetId !== after.defaultPresetId;
  const offered = offeredProviders(after);
  const presetExists = (id: string) => next.presets.some((p) => p.id === id);

  const stores = [useTranslateStore, ...useSessionTabs.getState().tabs.map((t) => t.store)];
  for (const store of stores) {
    const tab = store.getState();
    const was = effectiveSelection(tab, before, prev.presets);
    const patch: Partial<TabChoices> = {};

    if (pairChanged && tab.pair !== null) patch.pair = null;

    if (tab.provider !== null && !offered.includes(tab.provider)) {
      patch.provider = null;
    } else if (tab.provider === null && providerDefaultChanged && offered.includes(was.provider)) {
      patch.provider = was.provider;
    }

    if (tab.presetId !== null && next.presets.length > 0 && !presetExists(tab.presetId)) {
      patch.presetId = null;
    } else if (
      tab.presetId === null &&
      presetDefaultChanged &&
      was.preset &&
      presetExists(was.preset.id)
    ) {
      patch.presetId = was.preset.id;
    }

    // A mode that follows the preset stays what it was when the preset falls back.
    if (tab.mode === null && was.preset) {
      const now = effectiveSelection({ ...tab, ...patch }, after, next.presets);
      if (now.mode !== was.mode) patch.mode = was.mode;
    }

    if (Object.keys(patch).length > 0) store.setState(patch);
  }
}

/** Keeps the open Translate tabs in step with the settings and presets; returns the unsubscribe. */
export function initTabSettingsSync(): () => void {
  return useAppStore.subscribe((state, prev) => {
    if (state.settings === prev.settings && state.presets === prev.presets) return;
    applyToTabs(state, prev);
  });
}
