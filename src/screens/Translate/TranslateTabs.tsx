import { useState } from "react";

import { cx } from "../../lib/cx";
import { BindSessionDialog } from "./BindSessionDialog";
import { DEFAULT_TAB, MAX_BOUND_TABS, useSessionTabs } from "./sessionTabs";
import { TranslateScreen } from "./TranslateScreen";
import { useTranslateStore } from "./store";

/** The Translate screen with its tab strip: the default tab plus up to 5 session-bound tabs. */
export function TranslateTabs({ strip = true }: { strip?: boolean }) {
  const tabs = useSessionTabs((s) => s.tabs);
  const requestedId = useSessionTabs((s) => s.activeId);
  const activate = useSessionTabs((s) => s.activate);
  const close = useSessionTabs((s) => s.close);
  const [binding, setBinding] = useState(false);

  // Compact mode (no strip) always shows the default tab.
  const active = strip ? tabs.find((t) => t.id === requestedId) : undefined;
  const atLimit = tabs.length >= MAX_BOUND_TABS;

  const tabClass = (selected: boolean) =>
    cx(
      "flex h-8 items-center gap-1 rounded-t-md border-b-2 px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
      selected ? "border-accent text-accent" : "border-transparent text-muted hover:text-text",
    );

  return (
    <div className={cx(strip ? "flex flex-col gap-3" : "h-full")}>
      {strip && (
        <div
          role="tablist"
          aria-label="Translate tabs"
          className="mx-auto flex w-full max-w-3xl items-center gap-1 border-b border-border"
        >
          <button
            type="button"
            role="tab"
            aria-selected={!active}
            onClick={() => activate(DEFAULT_TAB)}
            className={tabClass(!active)}
          >
            Translate
          </button>
          {tabs.map((t) => (
            <div key={t.id} className="flex items-center">
              <button
                type="button"
                role="tab"
                aria-selected={active?.id === t.id}
                title={t.title}
                onClick={() => activate(t.id)}
                className={cx(tabClass(active?.id === t.id), "max-w-44")}
              >
                <span aria-hidden="true" className="font-mono text-xs">
                  &gt;_
                </span>
                <span className="truncate">{t.title}</span>
              </button>
              <button
                type="button"
                aria-label={`Close ${t.title}`}
                title="Unbind and close this tab"
                onClick={() => close(t.id)}
                className="-ml-2 flex h-5 w-5 items-center justify-center rounded text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            disabled={atLimit}
            title={atLimit ? "At most 5 bound tabs" : "Bind a Translate tab to a Claude session"}
            onClick={() => setBinding(true)}
            className="ml-1 h-8 rounded px-2 text-sm text-muted hover:text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            + Bind session
          </button>
        </div>
      )}
      {active ? (
        <TranslateScreen key={active.id} store={active.store} bound={active} />
      ) : (
        <TranslateScreen key={DEFAULT_TAB} store={useTranslateStore} />
      )}
      {binding && <BindSessionDialog onClose={() => setBinding(false)} />}
    </div>
  );
}
