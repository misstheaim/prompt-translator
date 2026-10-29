import { useState } from "react";

import { cx } from "../../lib/cx";
import { GeneralSettings } from "./GeneralSettings";
import { PresetsSettings } from "./PresetsSettings";

const TABS = [
  { id: "general", label: "General" },
  { id: "presets", label: "Presets" },
  { id: "about", label: "About" },
] as const;
type TabId = (typeof TABS)[number]["id"];

function TabPlaceholder({ title, ticket }: { title: string; ticket: string }) {
  return (
    <div className="flex flex-col items-center gap-2 py-12 text-center">
      <h2 className="text-base font-semibold text-text">{title}</h2>
      <p className="text-muted">Coming with {ticket}.</p>
    </div>
  );
}

export function SettingsScreen() {
  const [tab, setTab] = useState<TabId>("general");

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4">
      <div
        role="tablist"
        aria-label="Settings sections"
        className="flex gap-1 border-b border-border"
      >
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`settings-tab-${t.id}`}
            aria-selected={t.id === tab}
            aria-controls={`settings-panel-${t.id}`}
            onClick={() => setTab(t.id)}
            className={cx(
              "-mb-px h-9 border-b-2 px-3 text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent",
              t.id === tab
                ? "border-accent text-accent"
                : "border-transparent text-muted hover:text-text",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* General stays mounted so unsaved edits survive switching sub-tabs. */}
      <div
        role="tabpanel"
        id="settings-panel-general"
        aria-labelledby="settings-tab-general"
        hidden={tab !== "general"}
      >
        <GeneralSettings />
      </div>
      {tab === "presets" && (
        <div role="tabpanel" id="settings-panel-presets" aria-labelledby="settings-tab-presets">
          <PresetsSettings />
        </div>
      )}
      {tab === "about" && (
        <div role="tabpanel" id="settings-panel-about" aria-labelledby="settings-tab-about">
          <TabPlaceholder title="About" ticket="the About tab (T33)" />
        </div>
      )}
    </div>
  );
}
