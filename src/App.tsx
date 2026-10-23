import { useEffect, useState } from "react";

import { isAppError, ping } from "./lib/api";

function App() {
  const [pingResult, setPingResult] = useState("...");

  useEffect(() => {
    ping()
      .then(setPingResult)
      .catch((e: unknown) =>
        setPingResult(isAppError(e) ? e.message : "ping failed"),
      );
  }, []);

  return (
    <main className="flex h-full items-center justify-center bg-bg">
      <div className="rounded-lg border border-border bg-surface px-8 py-6 text-center shadow-sm">
        <h1 className="text-xl font-semibold text-text">Prompt Translator</h1>
        <p className="mt-1 text-muted">Placeholder window</p>
        <p className="mt-3 text-sm text-muted">Rust core: {pingResult}</p>
      </div>
    </main>
  );
}

export default App;
