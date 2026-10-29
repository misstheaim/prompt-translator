import { Button } from "../../components/Button";
import { cx } from "../../lib/cx";
import { useAppStore } from "../../store/app";
import { ClaudeStep } from "./ClaudeStep";
import { STEP_TITLES, WIZARD_STEPS } from "./constants";
import { DefaultsStep } from "./DefaultsStep";
import { DoneStep } from "./DoneStep";
import { LocalModelStep } from "./LocalModelStep";
import { useSetupStore } from "./store";
import { WelcomeStep } from "./WelcomeStep";

/** The first-run wizard (plan §4.8); it replaces the main UI until setup is completed. */
export function SetupScreen() {
  const step = useSetupStore((s) => s.step);
  const leaveSetup = useAppStore((s) => s.leaveSetup);
  const index = WIZARD_STEPS.indexOf(step);

  return (
    <div className="flex h-full flex-col bg-bg text-text">
      <header className="flex shrink-0 items-center gap-3 border-b border-border bg-surface px-4 py-2">
        <h1 className="text-sm font-semibold">Setup</h1>
        <ol className="flex items-center gap-2 text-xs" aria-label="Setup steps">
          {WIZARD_STEPS.map((s, i) => (
            <li
              key={s}
              aria-current={s === step ? "step" : undefined}
              className={cx(
                "rounded-full px-2 py-0.5",
                s === step
                  ? "bg-accent text-accent-contrast"
                  : i < index
                    ? "text-text"
                    : "text-muted",
              )}
            >
              {i + 1}. {STEP_TITLES[s]}
            </li>
          ))}
        </ol>
        <Button className="ml-auto" size="sm" variant="ghost" onClick={leaveSetup}>
          Leave setup
        </Button>
      </header>
      <main className="min-h-0 flex-1 overflow-auto p-4">
        <div className="mx-auto flex max-w-2xl flex-col gap-4">
          <h2 className="text-lg font-semibold">{STEP_TITLES[step]}</h2>
          {step === "welcome" && <WelcomeStep />}
          {step === "local" && <LocalModelStep />}
          {step === "claude" && <ClaudeStep />}
          {step === "defaults" && <DefaultsStep />}
          {step === "done" && <DoneStep />}
        </div>
      </main>
    </div>
  );
}
