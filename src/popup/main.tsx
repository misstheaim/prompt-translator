import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import type { Mode } from "../bindings/Mode";
import "../index.css";

/** Payload of `popup://source`: the captured text (empty when nothing usable) and the mode. */
interface PopupSource {
  text: string;
  mode: Mode;
}

// Placeholder for the quick-translate popup (T26): shows what the hotkey captured, Esc hides.
function Popup() {
  const [source, setSource] = useState<PopupSource | null>(null);

  useEffect(() => {
    const unlisten = listen<PopupSource>("popup://source", (event) => setSource(event.payload));
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") void getCurrentWindow().hide();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <main className="flex h-full flex-col gap-2 overflow-auto bg-bg p-4 text-text">
      {source === null ? (
        <p className="text-muted">Popup placeholder</p>
      ) : source.text.trim() === "" ? (
        <p className="text-muted">Select or copy some text first</p>
      ) : (
        <>
          <p className="text-xs text-muted">Mode: {source.mode}</p>
          <p className="whitespace-pre-wrap text-sm">{source.text}</p>
        </>
      )}
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popup />
  </React.StrictMode>,
);
