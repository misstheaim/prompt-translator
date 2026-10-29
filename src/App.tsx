import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

import type { Mode } from "./bindings/Mode";
import type { TranslationResult } from "./bindings/TranslationResult";
import { Banner } from "./components/Banner";
import { Button } from "./components/Button";
import { DevMockPanel } from "./components/DevMockPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { ToastHost } from "./components/Toast";
import { cx } from "./lib/cx";
import { HistoryScreen } from "./screens/History/HistoryScreen";
import { SettingsScreen } from "./screens/Settings/SettingsScreen";
import { TranslateScreen } from "./screens/Translate/TranslateScreen";
import { useTranslateStore } from "./screens/Translate/store";
import { initAppStore, SCREENS, useAppStore, type Screen } from "./store/app";

const TAB_LABELS: Record<Screen, string> = {
  translate: "Translate",
  history: "History",
  mistakes: "Mistakes",
  settings: "Settings",
};

/** Shown for screens that later tickets build (T20 Translate, T29 History, ...). */
function ScreenPlaceholder({ screen }: { screen: Screen }) {
  return (
    <div className="flex flex-col items-center gap-2 py-12 text-center">
      <h1 className="text-lg font-semibold text-text">{TAB_LABELS[screen]}</h1>
      <p className="text-muted">This screen isn't built yet.</p>
    </div>
  );
}

/** Payload of `main://open-entry` (sent by the popup, T26): the input and its result. */
interface OpenEntryPayload {
  input: string;
  mode: Mode;
  result: TranslationResult | null;
}

function App() {
  const screen = useAppStore((s) => s.screen);
  const setScreen = useAppStore((s) => s.setScreen);
  const loadError = useAppStore((s) => s.loadError);
  const hotkeyError = useAppStore((s) => s.hotkeyError);

  useEffect(() => {
    const unlisten = initAppStore();
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // The popup's "Open in main window" sends `main://open-entry` (Rust shows this window).
  useEffect(() => {
    const unlisten = listen<OpenEntryPayload>("main://open-entry", (event) => {
      const { input, mode, result } = event.payload;
      useTranslateStore.setState({
        input,
        mode,
        provider: result?.provider ?? null,
        status: result ? "success" : "idle",
        requestId: null,
        result,
        output: result?.output ?? "",
        error: null,
      });
      setScreen("translate");
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [setScreen]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (!e.ctrlKey) return;
      if (e.key === "Tab") {
        e.preventDefault();
        const i = SCREENS.indexOf(useAppStore.getState().screen);
        const step = e.shiftKey ? -1 : 1;
        setScreen(SCREENS[(i + step + SCREENS.length) % SCREENS.length]);
      } else if (e.key === ",") {
        e.preventDefault();
        setScreen("settings");
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setScreen]);

  return (
    <div className="flex h-full flex-col bg-bg text-text">
      <nav
        aria-label="Main"
        className="flex shrink-0 items-center gap-1 border-b border-border bg-surface px-4"
      >
        {SCREENS.map((s) => (
          <button
            key={s}
            type="button"
            aria-current={s === screen ? "page" : undefined}
            onClick={() => setScreen(s)}
            className={cx(
              "-mb-px h-10 border-b-2 px-3 text-sm font-medium transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent motion-reduce:transition-none",
              s === screen
                ? "border-accent text-accent"
                : "border-transparent text-muted hover:text-text",
            )}
          >
            {TAB_LABELS[s]}
          </button>
        ))}
        {/* Right side of the top bar: the session picker (E9) goes here. */}
        <div className="ml-auto" />
      </nav>

      {loadError && (
        <div className="shrink-0 px-4 pt-4">
          <ErrorBanner
            error={loadError}
            onRetry={() => void useAppStore.getState().loadSettings()}
          />
        </div>
      )}

      {hotkeyError && (
        <div className="shrink-0 px-4 pt-4">
          <Banner
            variant="warning"
            actions={
              <Button variant="secondary" onClick={() => setScreen("settings")}>
                Open settings
              </Button>
            }
          >
            {hotkeyError}
          </Banner>
        </div>
      )}

      <main className="min-h-0 flex-1 overflow-auto p-4">
        {screen === "translate" && import.meta.env.DEV && <DevMockPanel />}
        {screen === "translate" ? (
          <TranslateScreen />
        ) : screen === "history" ? (
          <HistoryScreen />
        ) : screen === "settings" ? (
          <SettingsScreen />
        ) : (
          <ScreenPlaceholder screen={screen} />
        )}
      </main>
      <ToastHost />
    </div>
  );
}

export default App;
