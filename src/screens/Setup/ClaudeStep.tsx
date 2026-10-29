import { useCallback, useEffect, useState } from "react";

import { Banner } from "../../components/Banner";
import { Button } from "../../components/Button";
import { Spinner } from "../../components/Spinner";
import { LINKS, openUrl, toAppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { StepNav } from "./StepNav";

export function ClaudeStep() {
  const claude = useAppStore((s) => s.providerStatus?.claude ?? null);
  const refresh = useAppStore((s) => s.refreshProviderStatus);
  const showToast = useAppStore((s) => s.showToast);
  const [checking, setChecking] = useState(true);

  const recheck = useCallback(async () => {
    setChecking(true);
    await refresh();
    setChecking(false);
  }, [refresh]);

  useEffect(() => {
    void refresh().then(() => setChecking(false));
  }, [refresh]);

  const working = claude?.found === true && claude.signedIn !== false;

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-text">
        Claude Code can translate too, using your own Claude login. It needs no download and works
        on any computer.
      </p>

      <div className="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4">
        {checking && !claude ? (
          <div className="flex items-center gap-2 text-sm text-muted">
            <Spinner /> Looking for Claude Code...
          </div>
        ) : !claude?.found ? (
          <Banner
            variant="warning"
            actions={
              <Button
                size="sm"
                variant="primary"
                onClick={() =>
                  openUrl(LINKS.claudeCodeSetup).catch((e: unknown) =>
                    showToast("error", toAppError(e).message),
                  )
                }
              >
                Install instructions
              </Button>
            }
          >
            Claude Code wasn&apos;t found. Install it, then choose &quot;Check again&quot;.
          </Banner>
        ) : (
          <>
            <Banner variant="success">
              Claude Code found{claude.version ? ` (version ${claude.version})` : ""}.
            </Banner>
            {claude.signedIn === false ? (
              <Banner variant="warning">
                Not signed in. Run <code className="font-mono">claude</code> once in a terminal and
                sign in, then choose &quot;Check again&quot;.
              </Banner>
            ) : claude.signedIn === true ? (
              <Banner variant="success">Signed in.</Banner>
            ) : (
              <Banner variant="info">
                Couldn&apos;t tell if you&apos;re signed in. If translating fails, run{" "}
                <code className="font-mono">claude</code> once in a terminal and sign in.
              </Banner>
            )}
          </>
        )}
        <div>
          <Button loading={checking} onClick={() => void recheck()}>
            Check again
          </Button>
        </div>
      </div>

      <StepNav
        nextLabel={working ? "Continue" : "Skip"}
        nextVariant={working ? "primary" : "secondary"}
      />
    </div>
  );
}
