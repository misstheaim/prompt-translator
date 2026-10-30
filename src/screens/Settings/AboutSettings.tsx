import { useEffect, useState } from "react";

import type { AppInfo } from "../../bindings/AppInfo";
import { Button } from "../../components/Button";
import { Spinner } from "../../components/Spinner";
import { UpdateNotes, UpdateProgressBar } from "../../components/UpdateBanner";
import { appInfo, checkUpdate, openFolder, saveSettings, toAppError } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { useUpdateStore } from "../../store/update";
import { Section, Toggle } from "./fields";

type CheckState = "idle" | "checking" | "none" | "available" | "error";

function FolderRow({
  label,
  path,
  folder,
}: {
  label: string;
  path: string;
  folder: "config" | "data";
}) {
  const showToast = useAppStore((s) => s.showToast);
  return (
    <div className="flex items-center gap-2">
      <div className="min-w-0 flex-1">
        <div className="text-xs text-muted">{label}</div>
        <div className="truncate font-mono text-xs text-text" title={path}>
          {path}
        </div>
      </div>
      <Button
        size="sm"
        onClick={() =>
          openFolder(folder).catch((e: unknown) => showToast("error", toAppError(e).message))
        }
      >
        Open
      </Button>
    </div>
  );
}

export function AboutSettings() {
  const settings = useAppStore((s) => s.settings);
  const setSettings = useAppStore((s) => s.setSettings);
  const openSetup = useAppStore((s) => s.openSetup);
  const showToast = useAppStore((s) => s.showToast);

  const available = useUpdateStore((s) => s.available);
  const installing = useUpdateStore((s) => s.installing);
  const progress = useUpdateStore((s) => s.progress);
  const installError = useUpdateStore((s) => s.installError);
  const install = useUpdateStore((s) => s.install);
  const setAvailable = useUpdateStore((s) => s.setAvailable);

  const [info, setInfo] = useState<AppInfo | null>(null);
  const [check, setCheck] = useState<CheckState>(available ? "available" : "idle");
  const [checkError, setCheckError] = useState("");
  const [savingToggle, setSavingToggle] = useState(false);

  useEffect(() => {
    appInfo()
      .then(setInfo)
      .catch((e: unknown) => showToast("error", toAppError(e).message));
  }, [showToast]);

  async function runCheck() {
    setCheck("checking");
    setCheckError("");
    try {
      const result = await checkUpdate();
      setAvailable(result);
      setCheck(result.available ? "available" : "none");
    } catch (e) {
      setCheckError(toAppError(e).message);
      setCheck("error");
    }
  }

  async function setAutoCheck(value: boolean) {
    if (!settings) return;
    setSavingToggle(true);
    try {
      setSettings(await saveSettings({ ...settings, autoCheckUpdates: value }));
    } catch (e) {
      showToast("error", toAppError(e).message);
    } finally {
      setSavingToggle(false);
    }
  }

  const showUpdate = available !== null && (check === "available" || installing);

  return (
    <div className="flex flex-col gap-4">
      <Section title="About">
        <div className="text-sm text-text">
          Prompt Translator <span className="font-mono">{info ? `v${info.version}` : ""}</span>
        </div>
        {info && (
          <>
            <FolderRow label="Settings folder" path={info.configDir} folder="config" />
            <FolderRow label="Data folder (history, logs)" path={info.dataDir} folder="data" />
          </>
        )}
      </Section>

      <Section title="Updates">
        <div className="flex items-center gap-3">
          <Button
            onClick={() => void runCheck()}
            loading={check === "checking"}
            disabled={installing}
          >
            Check for updates
          </Button>
          {check === "checking" && (
            <span className="flex items-center gap-2 text-sm text-muted">
              <Spinner /> Checking...
            </span>
          )}
          {check === "none" && (
            <span className="text-sm text-success">You&apos;re up to date.</span>
          )}
        </div>
        {check === "error" && (
          <p role="alert" className="text-sm text-danger">
            Couldn&apos;t check for updates: {checkError}
          </p>
        )}
        {showUpdate && (
          <div className="rounded-md border border-accent bg-accent/10 p-3">
            <p className="text-sm font-medium text-text">
              Version {available.version ?? ""} is available.
            </p>
            <UpdateNotes notes={available.notes} />
            {installError && (
              <p className="mt-1 text-xs text-danger">Update failed: {installError}</p>
            )}
            {installing ? (
              <UpdateProgressBar progress={progress} />
            ) : (
              <div className="mt-2">
                <Button size="sm" variant="primary" onClick={() => void install()}>
                  Install and restart
                </Button>
              </div>
            )}
          </div>
        )}
        <Toggle
          label="Check for updates on startup"
          checked={settings?.autoCheckUpdates ?? true}
          disabled={!settings || savingToggle}
          onChange={(v) => void setAutoCheck(v)}
        />
      </Section>

      <Section title="Setup" description="Reopens the setup wizard: provider, model and hotkey.">
        <div>
          <Button onClick={openSetup}>Run setup again</Button>
        </div>
      </Section>
    </div>
  );
}
