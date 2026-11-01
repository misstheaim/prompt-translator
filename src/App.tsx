import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import type { Mode } from "./bindings/Mode";
import type { TranslationResult } from "./bindings/TranslationResult";
import { Banner } from "./components/Banner";
import { Button } from "./components/Button";
import { DevMockPanel } from "./components/DevMockPanel";
import { CompactToggle } from "./components/CompactToggle";
import { ErrorBanner } from "./components/ErrorBanner";
import { ToastHost } from "./components/Toast";
import { UpdateBanner } from "./components/UpdateBanner";
import { toAppError, unloadModel } from "./lib/api";
import { applyAppearance } from "./lib/appearance";
import { cx } from "./lib/cx";
import { HistoryScreen } from "./screens/History/HistoryScreen";
import { MistakesScreen } from "./screens/Mistakes/MistakesScreen";
import { SetupScreen } from "./screens/Setup/SetupScreen";
import { SettingsScreen } from "./screens/Settings/SettingsScreen";
import { initSessionTabs, useSessionTabs, DEFAULT_TAB } from "./screens/Translate/sessionTabs";
import { initTabSettingsSync } from "./screens/Translate/settingsSync";
import { TranslateTabs } from "./screens/Translate/TranslateTabs";
import { useTranslateStore } from "./screens/Translate/store";
import {
  COMPACT_EXIT_MARGIN,
  initAppStore,
  NORMAL_MIN_HEIGHT,
  NORMAL_MIN_WIDTH,
  resizeIsSettled,
  SCREENS,
  useAppStore,
  type Screen,
} from "./store/app";
import { initUpdateStore } from "./store/update";

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
  const showToast = useAppStore((s) => s.showToast);
  const model = useAppStore((s) => s.settings?.ollama.model ?? "");
  const compact = useAppStore((s) => s.compact);
  const translating = useTranslateStore((s) => s.status === "loading");
  const [unloading, setUnloading] = useState(false);

  const theme = useAppStore((s) => s.settings?.theme);
  const accent = useAppStore((s) => s.settings?.accent);
  useEffect(() => {
    if (theme && accent) applyAppearance(theme, accent);
  }, [theme, accent]);

  useEffect(() => {
    const unlisten = initAppStore();
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = initUpdateStore();
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // A bound terminal window closing closes its Translate tab.
  useEffect(() => {
    const unlisten = initSessionTabs();
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // Saved settings and preset changes reach the open Translate tabs.
  useEffect(() => initTabSettingsSync(), []);

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
        resultSource: input,
        resultMode: mode,
        error: null,
      });
      // Opening an entry always loads the default tab.
      useSessionTabs.getState().activate(DEFAULT_TAB);
      setScreen("translate");
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [setScreen]);

  // The tray's "Unload model" reports through `main://model-unloaded`.
  useEffect(() => {
    const unlisten = listen<boolean | string>("main://model-unloaded", (event) => {
      const { showToast } = useAppStore.getState();
      if (typeof event.payload === "string") showToast("error", event.payload);
      else showToast("info", event.payload ? "Model unloaded" : "No model loaded");
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  async function unload() {
    setUnloading(true);
    try {
      const unloaded = await unloadModel();
      showToast("info", unloaded ? "Model unloaded" : "No model loaded");
    } catch (e) {
      showToast("error", toAppError(e).message);
    } finally {
      setUnloading(false);
    }
  }

  // Compact mode follows the window size (TRANS-016): dragging the width or the height down to
  // the normal minimum turns it on; a resize-entered one turns off again only once the window is
  // past the minimum plus a margin on both sides (so it doesn't leave right after entering).
  useEffect(() => {
    function onResize() {
      if (!resizeIsSettled()) return;
      const { compact, enterCompact, exitCompact } = useAppStore.getState();
      const width = window.innerWidth;
      const height = window.innerHeight;
      if (compact === null) {
        if (width <= NORMAL_MIN_WIDTH + 2 || height <= NORMAL_MIN_HEIGHT + 2) {
          void enterCompact("resize");
        }
      } else if (
        compact === "resize" &&
        width > NORMAL_MIN_WIDTH + COMPACT_EXIT_MARGIN &&
        height > NORMAL_MIN_HEIGHT + COMPACT_EXIT_MARGIN
      ) {
        void exitCompact();
      }
    }
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

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

  const setupCompleted = useAppStore((s) => s.settings?.setupCompleted);
  const setupRequested = useAppStore((s) => s.setupRequested);
  const setupDismissed = useAppStore((s) => s.setupDismissed);
  const openSetup = useAppStore((s) => s.openSetup);

  // First run (or "Run setup again"): the wizard replaces the main UI.
  if (setupCompleted !== undefined && (setupRequested || (!setupCompleted && !setupDismissed))) {
    return (
      <>
        <SetupScreen />
        <ToastHost />
      </>
    );
  }

  // Compact mode: the current Translate tab in its compact layout (the shell's bars and banners
  // are hidden). Never a window scrollbar: the layout fits down to the compact minimum size.
  if (compact !== null) {
    return (
      <div className="h-full overflow-hidden bg-bg text-text">
        <TranslateTabs strip={false} />
        <ToastHost />
      </div>
    );
  }

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
        {/* Right side of the top bar: the session picker (E9) goes here, before this button. */}
        <div className="ml-auto" />
        <CompactToggle />
        <Button
          size="sm"
          variant="ghost"
          loading={unloading}
          disabled={translating}
          title={`Unload model (${model}) from memory`}
          onClick={() => void unload()}
        >
          Unload model
        </Button>
      </nav>

      {loadError && (
        <div className="shrink-0 px-4 pt-4">
          <ErrorBanner
            error={loadError}
            onRetry={() => void useAppStore.getState().loadSettings()}
          />
        </div>
      )}

      <UpdateBanner />

      {setupCompleted === false && (
        <div className="shrink-0 px-4 pt-4">
          <Banner
            variant="info"
            actions={
              <Button size="sm" variant="primary" onClick={openSetup}>
                Resume setup
              </Button>
            }
          >
            Setup isn&apos;t finished. Translating needs a working provider.
          </Banner>
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
          <TranslateTabs />
        ) : screen === "history" ? (
          <HistoryScreen />
        ) : screen === "mistakes" ? (
          <MistakesScreen />
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
