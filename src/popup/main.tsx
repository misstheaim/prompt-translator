import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";

// Placeholder for the quick-translate popup (T25a/T26).
function Popup() {
  return (
    <main className="flex h-full items-center justify-center bg-bg text-text">
      Popup placeholder
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Popup />
  </React.StrictMode>,
);
