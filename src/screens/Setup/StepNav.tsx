import type { ReactNode } from "react";

import { Button } from "../../components/Button";
import { useSetupStore } from "./store";

interface StepNavProps {
  /** Label of the forward button; defaults to "Continue". */
  nextLabel?: string;
  nextVariant?: "primary" | "secondary";
  nextDisabled?: boolean;
  nextLoading?: boolean;
  /** Replaces the default (go to the next step). */
  onNext?: () => void;
  hideBack?: boolean;
  /** Extra buttons on the left of the forward button (e.g. Skip). */
  extra?: ReactNode;
}

/** Back / Skip / Continue row at the bottom of every step. */
export function StepNav({
  nextLabel = "Continue",
  nextVariant = "primary",
  nextDisabled,
  nextLoading,
  onNext,
  hideBack,
  extra,
}: StepNavProps) {
  const back = useSetupStore((s) => s.back);
  const next = useSetupStore((s) => s.next);
  return (
    <div className="mt-2 flex items-center gap-2">
      {!hideBack && (
        <Button variant="ghost" onClick={back}>
          Back
        </Button>
      )}
      <div className="ml-auto flex items-center gap-2">
        {extra}
        <Button
          variant={nextVariant}
          disabled={nextDisabled}
          loading={nextLoading}
          onClick={onNext ?? next}
        >
          {nextLabel}
        </Button>
      </div>
    </div>
  );
}
