import { useState } from "react";

import type { Mode } from "../../bindings/Mode";
import type { Preset } from "../../bindings/Preset";
import { Button } from "../../components/Button";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Select, type SelectOption } from "../../components/Select";
import { TextArea } from "../../components/TextArea";
import { cx } from "../../lib/cx";
import { deletePreset, resetPresets, savePreset, toAppError, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { TextField, Toggle } from "./fields";

const MODE_OPTIONS: SelectOption[] = [
  { value: "translate", label: "Translate" },
  { value: "improve", label: "Improve" },
  { value: "correct", label: "Correct" },
];

interface Draft {
  /** Null while creating a new preset. */
  id: string | null;
  name: string;
  description: string;
  defaultMode: Mode;
  /** One rule per line. */
  rules: string;
  builtIn: boolean;
  saveToHistory: boolean;
}

const NEW_DRAFT: Draft = {
  id: null,
  name: "",
  description: "",
  defaultMode: "translate",
  rules: "",
  builtIn: false,
  saveToHistory: true,
};

function toDraft(p: Preset): Draft {
  return {
    id: p.id,
    name: p.name,
    description: p.description,
    defaultMode: p.defaultMode,
    rules: p.rules.join("\n"),
    builtIn: p.builtIn,
    saveToHistory: p.saveToHistory,
  };
}

function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/** A slug from the name that no existing preset uses. */
function uniqueId(name: string, taken: Set<string>): string {
  const base = slugify(name) || "preset";
  let id = base;
  for (let n = 2; taken.has(id); n++) id = `${base}-${n}`;
  return id;
}

function parseRules(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((r) => r.trim())
    .filter((r) => r !== "");
}

export function PresetsSettings() {
  const presets = useAppStore((s) => s.presets);
  const setPresets = useAppStore((s) => s.setPresets);
  const showToast = useAppStore((s) => s.showToast);

  const [draft, setDraft] = useState<Draft | null>(null);
  const [base, setBase] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [confirm, setConfirm] = useState<"delete" | "reset" | null>(null);

  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(base);

  function open(next: Draft) {
    setDraft(next);
    setBase(next);
    setError(null);
  }

  function patch(update: Partial<Draft>) {
    setDraft((d) => (d ? { ...d, ...update } : d));
  }

  async function run(
    action: () => Promise<Preset[]>,
    done: string,
    after?: (all: Preset[]) => void,
  ) {
    setBusy(true);
    setError(null);
    try {
      const all = await action();
      setPresets(all);
      showToast("success", done);
      after?.(all);
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy(false);
    }
  }

  function save() {
    if (!draft) return;
    const id = draft.id ?? uniqueId(draft.name, new Set(presets.map((p) => p.id)));
    const preset: Preset = {
      id,
      name: draft.name.trim(),
      description: draft.description.trim(),
      defaultMode: draft.defaultMode,
      rules: parseRules(draft.rules),
      builtIn: draft.builtIn,
      saveToHistory: draft.saveToHistory,
    };
    void run(
      () => savePreset(preset),
      "Preset saved",
      (all) => {
        const saved = all.find((p) => p.id === id);
        if (saved) open(toDraft(saved));
      },
    );
  }

  function remove() {
    const id = draft?.id;
    setConfirm(null);
    if (!id) return;
    void run(
      () => deletePreset(id),
      "Preset deleted",
      () => {
        setDraft(null);
        setBase(null);
      },
    );
  }

  function reset() {
    setConfirm(null);
    void run(resetPresets, "Built-in presets restored", (all) => {
      // Reload the open editor with the restored values.
      const current = draft?.id ? all.find((p) => p.id === draft.id) : undefined;
      if (current) open(toDraft(current));
    });
  }

  const nameMissing = draft !== null && draft.name.trim() === "";
  const ruleCount = draft ? parseRules(draft.rules).length : 0;

  return (
    <div className="flex flex-col gap-4 sm:flex-row">
      <div className="flex shrink-0 flex-col gap-2 sm:w-56">
        <ul className="flex flex-col gap-1" aria-label="Presets">
          {presets.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                onClick={() => open(toDraft(p))}
                aria-current={draft?.id === p.id ? "true" : undefined}
                className={cx(
                  "flex w-full items-center justify-between gap-2 rounded-md border px-3 py-2 text-left text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                  draft?.id === p.id
                    ? "border-accent bg-surface-raised text-text"
                    : "border-border bg-surface text-text hover:bg-surface-raised",
                )}
              >
                <span className="truncate">{p.name}</span>
                {p.builtIn && (
                  <span className="shrink-0 rounded-full border border-border px-2 py-0.5 text-xs text-muted">
                    Built-in
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
        <Button onClick={() => open(NEW_DRAFT)}>New preset</Button>
        <Button variant="ghost" onClick={() => setConfirm("reset")} disabled={busy}>
          Reset built-ins
        </Button>
      </div>

      <div className="min-w-0 flex-1">
        {draft === null ? (
          <p className="py-8 text-center text-muted">
            Select a preset to edit, or create a new one.
          </p>
        ) : (
          <form
            className="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4"
            onSubmit={(e) => {
              e.preventDefault();
              if (!nameMissing && !busy) save();
            }}
          >
            <TextField
              label="Name"
              value={draft.name}
              onChange={(name) => patch({ name })}
              error={nameMissing && dirty ? "Enter a name" : undefined}
            />
            <TextField
              label="Id"
              value={draft.id ?? uniqueId(draft.name, new Set(presets.map((p) => p.id)))}
              onChange={() => undefined}
              readOnly
              hint={draft.id ? "The id cannot change after creation." : "Generated from the name."}
            />
            <TextField
              label="Description"
              value={draft.description}
              onChange={(description) => patch({ description })}
            />
            <Select
              label="Default mode"
              options={MODE_OPTIONS}
              value={draft.defaultMode}
              onChange={(v) => patch({ defaultMode: v as Mode })}
            />
            <div className="flex flex-col gap-1">
              <label htmlFor="preset-rules" className="text-xs font-medium text-text">
                Rules
              </label>
              <TextArea
                id="preset-rules"
                value={draft.rules}
                rows={6}
                onChange={(e) => patch({ rules: e.target.value })}
              />
              <p className="text-xs text-muted">
                One rule per line (at most 30 rules, 500 characters each). {ruleCount} rule
                {ruleCount === 1 ? "" : "s"}.
              </p>
            </div>
            <Toggle
              label="Save to history"
              description="Save results made with this preset to History and Mistakes. Turn off for presets whose rewrites aren't real mistakes."
              checked={draft.saveToHistory}
              onChange={(saveToHistory) => patch({ saveToHistory })}
            />

            {error && <ErrorBanner error={error} />}

            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="submit"
                variant="primary"
                loading={busy}
                disabled={nameMissing || (!dirty && draft.id !== null)}
              >
                Save
              </Button>
              <Button onClick={() => setDraft(base)} disabled={!dirty || busy}>
                Revert
              </Button>
              {draft.id !== null && (
                <Button
                  variant="danger"
                  className="ml-auto"
                  onClick={() => setConfirm("delete")}
                  disabled={busy}
                >
                  Delete
                </Button>
              )}
            </div>
          </form>
        )}
        {draft === null && error && <ErrorBanner error={error} className="mt-3" />}
      </div>

      <ConfirmDialog
        open={confirm === "delete"}
        destructive
        title="Delete preset?"
        message={`"${draft?.name ?? ""}" will be deleted. This cannot be undone.`}
        confirmLabel="Delete"
        onConfirm={remove}
        onCancel={() => setConfirm(null)}
      />
      <ConfirmDialog
        open={confirm === "reset"}
        destructive
        title="Reset built-in presets?"
        message="Every built-in preset returns with its original name, description, mode, rules and history setting, including the ones you deleted. Presets you created are kept."
        confirmLabel="Reset"
        onConfirm={reset}
        onCancel={() => setConfirm(null)}
      />
    </div>
  );
}
