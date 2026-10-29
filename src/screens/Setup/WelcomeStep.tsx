import { StepNav } from "./StepNav";

export function WelcomeStep() {
  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-text">
        Prompt Translator helps you practise English while you work with Claude Code. Write a prompt
        in Russian (or any language, or a mix with English) and it translates, improves or corrects
        it, explains your mistakes and keeps a history and a &quot;my mistakes&quot; log.
      </p>
      <p className="text-sm text-text">
        Setup takes a few minutes: pick where translations run (a local model with Ollama, Claude
        Code, or both), then choose a few defaults.
      </p>
      <div className="rounded-md border border-border bg-surface-raised p-3 text-sm text-muted">
        <p className="font-medium text-text">Privacy</p>
        <p className="mt-1">
          Your text goes only to the provider you pick: Ollama runs on this computer, and Claude
          Code uses your own Claude login. Only the English text you accept is copied to the
          clipboard; nothing is sent to your Claude sessions on its own.
        </p>
      </div>
      <StepNav hideBack nextLabel="Get started" />
    </div>
  );
}
