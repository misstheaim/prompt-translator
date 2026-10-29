import { useState } from "react";

import type { ProviderKind } from "../../bindings/ProviderKind";
import type { Settings } from "../../bindings/Settings";
import { Button } from "../../components/Button";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Select, type SelectOption } from "../../components/Select";
import { Banner } from "../../components/Banner";
import { historyClear, pickFile, saveSettings, toAppError, type AppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { HotkeyField, Section, TextField, Toggle } from "./fields";

const TIMEOUT_MIN = 5;
const TIMEOUT_MAX = 600;
const NUM_CTX_MIN = 2048;
const NUM_CTX_MAX = 32768;

type Errors = Partial<Record<string, string>>;

/** Mirrors `Settings::validate` in Rust so errors show next to the field before saving. */
function validate(s: Settings): Errors {
  const errors: Errors = {};
  if (!/^https?:\/\/[^\s/:?#][^\s]*$/i.test(s.ollama.endpoint.trim())) {
    errors.endpoint = "Use a URL like http://localhost:11434";
  }
  if (
    !Number.isInteger(s.ollama.timeoutSecs) ||
    s.ollama.timeoutSecs < TIMEOUT_MIN ||
    s.ollama.timeoutSecs > TIMEOUT_MAX
  ) {
    errors.ollamaTimeout = `Enter ${TIMEOUT_MIN} to ${TIMEOUT_MAX} seconds`;
  }
  if (
    !Number.isInteger(s.claude.timeoutSecs) ||
    s.claude.timeoutSecs < TIMEOUT_MIN ||
    s.claude.timeoutSecs > TIMEOUT_MAX
  ) {
    errors.claudeTimeout = `Enter ${TIMEOUT_MIN} to ${TIMEOUT_MAX} seconds`;
  }
  if (
    !Number.isInteger(s.ollama.numCtx) ||
    s.ollama.numCtx < NUM_CTX_MIN ||
    s.ollama.numCtx > NUM_CTX_MAX
  ) {
    errors.numCtx = `Enter ${NUM_CTX_MIN} to ${NUM_CTX_MAX}`;
  }
  if (s.hotkey.trim() === "") errors.hotkey = "Press a key combination";
  return errors;
}

function StatusBadge({ tone, children }: { tone: "ok" | "bad" | "neutral"; children: string }) {
  const color =
    tone === "ok"
      ? "border-success text-success"
      : tone === "bad"
        ? "border-danger text-danger"
        : "border-border text-muted";
  return (
    <span className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs ${color}`}>
      {children}
    </span>
  );
}

function toNumber(value: string): number {
  return value.trim() === "" ? Number.NaN : Number(value);
}

function SettingsForm({ initial }: { initial: Settings }) {
  const presets = useAppStore((s) => s.presets);
  const status = useAppStore((s) => s.providerStatus);
  const refreshProviderStatus = useAppStore((s) => s.refreshProviderStatus);
  const setSettings = useAppStore((s) => s.setSettings);
  const showToast = useAppStore((s) => s.showToast);

  const [base, setBase] = useState(initial);
  const [draft, setDraft] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<AppError | null>(null);
  const [testing, setTesting] = useState<"ollama" | "claude" | null>(null);
  const [tested, setTested] = useState<{ ollama: boolean; claude: boolean }>({
    ollama: false,
    claude: false,
  });
  const [confirmClear, setConfirmClear] = useState(false);

  const dirty = JSON.stringify(draft) !== JSON.stringify(base);

  // Settings changed elsewhere (saved here, tray, another window): take them unless there are
  // unsaved edits. Adjusting state during render avoids an effect round trip.
  if (initial !== base) {
    setBase(initial);
    if (!dirty) setDraft(initial);
  }

  const errors = validate(draft);
  const hasErrors = Object.keys(errors).length > 0;

  function patch(update: Partial<Settings>) {
    setDraft((d) => ({ ...d, ...update }));
  }
  function patchOllama(update: Partial<Settings["ollama"]>) {
    setDraft((d) => ({ ...d, ollama: { ...d.ollama, ...update } }));
  }
  function patchClaude(update: Partial<Settings["claude"]>) {
    setDraft((d) => ({ ...d, claude: { ...d.claude, ...update } }));
  }

  async function save() {
    setSaving(true);
    setSaveError(null);
    try {
      const saved = await saveSettings(draft);
      setSettings(saved);
      setBase(saved);
      setDraft(saved);
      showToast("success", "Settings saved");
      void refreshProviderStatus();
    } catch (e) {
      setSaveError(toAppError(e));
    } finally {
      setSaving(false);
    }
  }

  function revert() {
    setDraft(base);
    setSaveError(null);
  }

  async function test(which: "ollama" | "claude") {
    setTesting(which);
    await refreshProviderStatus();
    setTested((t) => ({ ...t, [which]: true }));
    setTesting(null);
  }

  async function browse() {
    try {
      const path = await pickFile(["exe"]);
      if (path) patchClaude({ executablePath: path });
    } catch (e) {
      showToast("error", toAppError(e).message);
    }
  }

  async function clearHistory() {
    setConfirmClear(false);
    try {
      await historyClear();
      showToast("success", "History cleared");
    } catch (e) {
      showToast("error", toAppError(e).message);
    }
  }

  const providerOptions: SelectOption[] = [
    { value: "ollama", label: "Ollama (local)" },
    { value: "claudeCode", label: "Claude Code" },
  ];
  if (draft.defaultProvider === "mock")
    providerOptions.push({ value: "mock", label: "Mock (dev)" });

  const presetOptions: SelectOption[] = presets.map((p) => ({ value: p.id, label: p.name }));
  if (!presetOptions.some((o) => o.value === draft.defaultPresetId)) {
    presetOptions.push({ value: draft.defaultPresetId, label: draft.defaultPresetId });
  }

  const ollama = status?.ollama;
  const claude = status?.claude ?? null;
  const modelListId = "ollama-models";
  const testNeedsSave = dirty
    ? "Save your changes first; the test uses the saved settings."
    : undefined;

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        if (!hasErrors && dirty) void save();
      }}
    >
      <Section title="Ollama" description="The local model. Text generation only.">
        <TextField
          label="Endpoint"
          value={draft.ollama.endpoint}
          onChange={(v) => patchOllama({ endpoint: v })}
          error={errors.endpoint}
          placeholder="http://localhost:11434"
        />
        <TextField
          label="Model"
          value={draft.ollama.model}
          onChange={(v) => patchOllama({ model: v })}
          list={modelListId}
          hint={
            ollama?.reachable
              ? "Pick an installed model or type a name."
              : "Type a model name. Installed models appear here once Ollama is reachable."
          }
        />
        <datalist id={modelListId}>
          {(ollama?.models ?? []).map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <TextField
            label="Keep alive"
            value={draft.ollama.keepAlive}
            onChange={(v) => patchOllama({ keepAlive: v })}
            hint="e.g. 30m"
          />
          <TextField
            label="Context size"
            type="number"
            value={Number.isNaN(draft.ollama.numCtx) ? "" : draft.ollama.numCtx}
            onChange={(v) => patchOllama({ numCtx: toNumber(v) })}
            error={errors.numCtx}
            min={NUM_CTX_MIN}
            max={NUM_CTX_MAX}
          />
          <TextField
            label="Timeout (seconds)"
            type="number"
            value={Number.isNaN(draft.ollama.timeoutSecs) ? "" : draft.ollama.timeoutSecs}
            onChange={(v) => patchOllama({ timeoutSecs: toNumber(v) })}
            error={errors.ollamaTimeout}
            min={TIMEOUT_MIN}
            max={TIMEOUT_MAX}
          />
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            loading={testing === "ollama"}
            disabled={dirty}
            title={testNeedsSave}
            onClick={() => void test("ollama")}
          >
            Test
          </Button>
          {testing === "ollama" ? (
            <StatusBadge tone="neutral">Checking...</StatusBadge>
          ) : !ollama ? (
            <StatusBadge tone="neutral">Not checked</StatusBadge>
          ) : !ollama.reachable ? (
            <StatusBadge tone="bad">Not reachable</StatusBadge>
          ) : !ollama.selectedModelInstalled ? (
            <StatusBadge tone="bad">Reachable, model not installed</StatusBadge>
          ) : (
            <StatusBadge tone="ok">{`Ready (${ollama.models.length} models)`}</StatusBadge>
          )}
          {tested.ollama && ollama && !ollama.reachable && (
            <span className="text-xs text-muted">Is Ollama running? Check the endpoint.</span>
          )}
        </div>
      </Section>

      <Section title="Claude Code" description="Runs `claude -p` with your normal login.">
        <TextField
          label="Executable path"
          value={draft.claude.executablePath ?? ""}
          onChange={(v) => patchClaude({ executablePath: v.trim() === "" ? null : v })}
          placeholder="Automatic"
          hint="Leave empty to find claude.exe automatically."
          trailing={
            <Button size="sm" onClick={() => void browse()}>
              Browse
            </Button>
          }
        />
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <TextField
            label="Model"
            value={draft.claude.model}
            onChange={(v) => patchClaude({ model: v })}
            hint="Alias or full model name, e.g. sonnet."
          />
          <TextField
            label="Timeout (seconds)"
            type="number"
            value={Number.isNaN(draft.claude.timeoutSecs) ? "" : draft.claude.timeoutSecs}
            onChange={(v) => patchClaude({ timeoutSecs: toNumber(v) })}
            error={errors.claudeTimeout}
            min={TIMEOUT_MIN}
            max={TIMEOUT_MAX}
          />
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            loading={testing === "claude"}
            disabled={dirty}
            title={testNeedsSave}
            onClick={() => void test("claude")}
          >
            Check
          </Button>
          {testing === "claude" ? (
            <StatusBadge tone="neutral">Checking...</StatusBadge>
          ) : !claude ? (
            <StatusBadge tone="neutral">Not checked</StatusBadge>
          ) : !claude.found ? (
            <StatusBadge tone="bad">Not found</StatusBadge>
          ) : (
            <>
              <StatusBadge tone="ok">{`Found${claude.version ? ` (${claude.version})` : ""}`}</StatusBadge>
              <StatusBadge
                tone={
                  claude.signedIn === true ? "ok" : claude.signedIn === false ? "bad" : "neutral"
                }
              >
                {claude.signedIn === true
                  ? "Signed in"
                  : claude.signedIn === false
                    ? "Not signed in"
                    : "Sign-in unknown"}
              </StatusBadge>
            </>
          )}
        </div>
        {claude?.found && claude.signedIn === false && (
          <Banner variant="warning">
            Run <code className="font-mono">claude</code> once in a terminal and sign in.
          </Banner>
        )}
        {claude && !claude.found && (
          <Banner variant="warning">
            Claude Code wasn't found. Install it, or set the executable path above.
          </Banner>
        )}
      </Section>

      <Section title="Translation" description="Used when you open the app or the popup.">
        <div className="flex flex-wrap gap-4">
          <Select
            label="Default provider"
            value={draft.defaultProvider}
            options={providerOptions}
            onChange={(v) => patch({ defaultProvider: v as ProviderKind })}
          />
          <Select
            label="Default preset"
            value={draft.defaultPresetId}
            options={presetOptions}
            onChange={(v) => patch({ defaultPresetId: v })}
          />
        </div>
      </Section>

      <Section title="History">
        <Toggle
          label="Save translations to history"
          description="Turn off to stop recording new entries. Existing ones stay."
          checked={draft.historyEnabled}
          onChange={(v) => patch({ historyEnabled: v })}
        />
        <div>
          <Button variant="danger" size="sm" onClick={() => setConfirmClear(true)}>
            Clear all history
          </Button>
        </div>
      </Section>

      <Section title="System">
        <HotkeyField
          label="Global hotkey"
          value={draft.hotkey}
          onChange={(v) => patch({ hotkey: v })}
          error={errors.hotkey}
        />
        <Toggle
          label="Copy the selected text when the hotkey is pressed"
          description="Sends Ctrl+C to the app you were in, unless it is a terminal."
          checked={draft.autoCopySelection}
          onChange={(v) => patch({ autoCopySelection: v })}
        />
        <Toggle
          label="Start with Windows"
          checked={draft.autostart}
          onChange={(v) => patch({ autostart: v })}
        />
        <Toggle
          label="Keep running in the tray when the window is closed"
          checked={draft.closeToTray}
          onChange={(v) => patch({ closeToTray: v })}
        />
      </Section>

      {saveError && <ErrorBanner error={saveError} />}

      <div className="sticky bottom-0 flex items-center justify-end gap-2 border-t border-border bg-bg py-3">
        {dirty && <span className="mr-auto text-xs text-muted">Unsaved changes</span>}
        <Button disabled={!dirty || saving} onClick={revert}>
          Revert
        </Button>
        <Button type="submit" variant="primary" loading={saving} disabled={!dirty || hasErrors}>
          Save
        </Button>
      </div>

      <ConfirmDialog
        open={confirmClear}
        destructive
        title="Clear all history?"
        message="This deletes every saved translation and your mistakes log. It can't be undone."
        confirmLabel="Clear history"
        onConfirm={() => void clearHistory()}
        onCancel={() => setConfirmClear(false)}
      />
    </form>
  );
}

export function GeneralSettings() {
  const settings = useAppStore((s) => s.settings);
  if (!settings) return <p className="py-8 text-center text-muted">Loading settings...</p>;
  return <SettingsForm initial={settings} />;
}
