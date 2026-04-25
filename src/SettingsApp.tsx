import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppsSetup } from "@/components/apps-setup";
import { NotificationSetup } from "@/components/notification-setup";
import type { CliInstallState } from "@/components/notification-setup";
import { NumberStepper } from "@/components/number-stepper";
import { RestartBanner } from "@/components/restart-banner";
import { SettingsRow, SettingsSection } from "@/components/settings-section";
import { ShortcutRecorder } from "@/components/shortcut-recorder";
import { Toggle } from "@/components/toggle";
import type { AllowedApp } from "@/lib/types";
import type {
  CliInstallResult,
  CliInstallStatus,
  SaveSettingsResult,
  SettingsPayload,
} from "@/lib/settings-types";
import { RESTART_REQUIRED_FIELDS } from "@/lib/settings-types";

type Status = { kind: "loading" } | { kind: "error"; message: string } | { kind: "ready" };

type SaveState =
  | { kind: "idle" }
  | { kind: "saving" }
  | { kind: "saved"; restartRequired: boolean }
  | { kind: "error"; message: string };

const MIN_TOAST_DURATION_MS = 500;

export function SettingsApp() {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [original, setOriginal] = useState<SettingsPayload | null>(null);
  const [draft, setDraft] = useState<SettingsPayload | null>(null);
  const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });
  const [reservedShortcuts, setReservedShortcuts] = useState<string[]>([]);
  const [cliStatus, setCliStatus] = useState<CliInstallStatus | null>(null);
  const [cliInstallState, setCliInstallState] = useState<CliInstallState>({ kind: "idle" });
  const [allowedApps, setAllowedApps] = useState<AllowedApp[]>([]);

  const refreshCliStatus = useCallback(() => {
    invoke<CliInstallStatus>("get_cli_install_status")
      .then(setCliStatus)
      .catch((err) => {
        console.warn("get_cli_install_status failed:", err);
      });
  }, []);

  useEffect(() => {
    invoke<SettingsPayload>("get_settings")
      .then((payload) => {
        setOriginal(payload);
        setDraft(payload);
        setStatus({ kind: "ready" });
      })
      .catch((err) => {
        setStatus({ kind: "error", message: String(err) });
      });

    invoke<string[]>("get_reserved_shortcuts")
      .then(setReservedShortcuts)
      .catch((err) => {
        console.warn("get_reserved_shortcuts failed:", err);
      });

    invoke<AllowedApp[]>("get_apps_allowed_apps")
      .then(setAllowedApps)
      .catch((err) => {
        console.warn("get_apps_allowed_apps failed:", err);
      });

    refreshCliStatus();
  }, [refreshCliStatus]);

  const handleAllowedAppsChange = useCallback((next: AllowedApp[]) => {
    setAllowedApps(next);
    void invoke("save_apps_allowed_apps", { allowedApps: next }).catch((err) => {
      console.warn("save_apps_allowed_apps failed:", err);
    });
  }, []);

  const handleInstallCli = async () => {
    setCliInstallState({ kind: "installing" });
    try {
      const result = await invoke<CliInstallResult>("install_cli_symlink");
      setCliInstallState({ kind: "installed", replaced: result.replacedExisting });
      refreshCliStatus();
    } catch (err) {
      setCliInstallState({ kind: "error", message: String(err) });
    }
  };

  const dirty = useMemo(() => {
    if (!original || !draft) return false;
    return (Object.keys(draft) as (keyof SettingsPayload)[]).some(
      (key) => draft[key] !== original[key],
    );
  }, [original, draft]);

  const restartWillBeRequired = useMemo(() => {
    if (!original || !draft) return false;
    return RESTART_REQUIRED_FIELDS.some((key) => draft[key] !== original[key]);
  }, [original, draft]);

  const showRestartBanner = saveState.kind === "saved" && saveState.restartRequired;

  const updateField = useCallback(
    <K extends keyof SettingsPayload>(key: K, value: SettingsPayload[K]) => {
      setDraft((prev) => (prev ? { ...prev, [key]: value } : prev));
      setSaveState((prev) => (prev.kind === "saved" ? { kind: "idle" } : prev));
    },
    [],
  );

  const handleSave = async () => {
    if (!draft) return;
    setSaveState({ kind: "saving" });
    try {
      const result = await invoke<SaveSettingsResult>("save_settings", {
        payload: draft,
      });
      setOriginal(draft);
      setSaveState({ kind: "saved", restartRequired: result.restartRequired });
    } catch (err) {
      setSaveState({ kind: "error", message: String(err) });
    }
  };

  const handleRestart = () => {
    void invoke("restart_app");
  };

  const handleRevert = () => {
    if (original) setDraft(original);
    setSaveState({ kind: "idle" });
  };

  if (status.kind === "loading") {
    return (
      <div className="flex h-screen items-center justify-center bg-[var(--panel-bg)] text-xs text-[var(--text-tertiary)]">
        Loading settings…
      </div>
    );
  }

  if (status.kind === "error" || !draft) {
    return (
      <div className="flex h-screen items-center justify-center bg-[var(--panel-bg)] px-6 text-center text-xs text-[var(--delete-hover-text)]">
        Failed to load settings: {status.kind === "error" ? status.message : "unknown error"}
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-[var(--panel-bg)] text-[var(--text-primary)]">
      {showRestartBanner && <RestartBanner onRestart={handleRestart} />}

      <div className="flex-1 overflow-y-auto px-5 py-5">
        <SettingsSection
          title="Keyboard shortcut"
          description="Global shortcut that toggles the main panel."
        >
          <SettingsRow
            label="Toggle panel"
            hint="Click, then press the shortcut. Use ✕ to clear."
            htmlFor="toggle-panel"
          >
            <ShortcutRecorder
              id="toggle-panel"
              value={draft.togglePanelShortcut}
              onChange={(v) => updateField("togglePanelShortcut", v)}
              reservedShortcuts={reservedShortcuts}
            />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection
          title="Toast"
          description="Transient popup at the top-right when a new notification arrives."
        >
          <SettingsRow
            label="Display duration"
            hint="How long a toast stays visible (ms). Ignored when 'Keep until clicked' is on."
            htmlFor="toast-duration"
          >
            <NumberStepper
              id="toast-duration"
              value={draft.toastDurationMs}
              min={MIN_TOAST_DURATION_MS}
              step={500}
              onChange={(v) => updateField("toastDurationMs", v)}
            />
          </SettingsRow>
          <SettingsRow
            label="Keep until clicked"
            hint="Toasts do not auto-dismiss when enabled."
            htmlFor="toast-persistent"
          >
            <Toggle
              id="toast-persistent"
              checked={draft.toastPersistent}
              onChange={(v) => updateField("toastPersistent", v)}
              ariaLabel="Keep toast until clicked"
            />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="Startup" description="Control how agentoast launches on this Mac.">
          <SettingsRow
            label="Launch at login"
            hint="Start agentoast automatically when you log in to macOS."
            htmlFor="autostart-enabled"
          >
            <Toggle
              id="autostart-enabled"
              checked={draft.autostartEnabled}
              onChange={(v) => updateField("autostartEnabled", v)}
              ariaLabel="Launch at login"
            />
          </SettingsRow>
        </SettingsSection>

        <section className="mb-5">
          <header className="mb-2 px-1">
            <h2 className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
              Notifications
            </h2>
            <p className="mt-1 text-[11px] text-[var(--text-tertiary)]">
              Connect your AI coding agent so it can send notifications to agentoast.
            </p>
          </header>
          <NotificationSetup
            variant="settings"
            cliStatus={cliStatus}
            cliInstallState={cliInstallState}
            onInstallCliClick={() => {
              void handleInstallCli();
            }}
          />
        </section>

        <section className="mb-5">
          <header className="mb-2 px-1">
            <h2 className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
              Apps
            </h2>
            <p className="mt-1 text-[11px] text-[var(--text-tertiary)]">
              Pin frequently-used apps so you can switch to them from the panel’s Apps tab.
            </p>
          </header>
          <AppsSetup allowedApps={allowedApps} onChange={handleAllowedAppsChange} />
        </section>

        <SettingsSection
          title="Editor"
          description="Editor used by `agentoast config`. Falls back to $EDITOR then vim when empty."
        >
          <SettingsRow label="Command" htmlFor="editor-cmd">
            <input
              id="editor-cmd"
              type="text"
              spellCheck={false}
              placeholder="(auto-detect)"
              className="h-7 w-52 rounded-md border border-[var(--border-primary)] bg-[var(--panel-bg)] px-2 text-xs text-[var(--text-primary)] outline-none focus:border-[var(--accent)]"
              value={draft.editor}
              onChange={(e) => updateField("editor", e.target.value)}
            />
          </SettingsRow>
        </SettingsSection>
      </div>

      <footer className="flex h-[48px] shrink-0 items-center justify-between gap-3 border-t border-[var(--border-subtle)] px-5">
        <div className="text-[11px] text-[var(--text-tertiary)]">
          {saveState.kind === "saving" && "Saving…"}
          {saveState.kind === "saved" &&
            (saveState.restartRequired ? "Saved. Restart required for some changes." : "Saved.")}
          {saveState.kind === "error" && (
            <span className="text-[var(--delete-hover-text)]">
              Save failed: {saveState.message}
            </span>
          )}
          {saveState.kind === "idle" &&
            dirty &&
            restartWillBeRequired &&
            "Unsaved changes will require a restart."}
          {saveState.kind === "idle" && dirty && !restartWillBeRequired && "Unsaved changes."}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleRevert}
            disabled={!dirty || saveState.kind === "saving"}
            className="rounded-md px-3 py-1.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--hover-bg)] disabled:cursor-not-allowed disabled:opacity-30"
          >
            Revert
          </button>
          <button
            type="button"
            onClick={() => {
              void handleSave();
            }}
            disabled={!dirty || saveState.kind === "saving"}
            className="rounded-md bg-[var(--accent)] px-3.5 py-1.5 text-xs font-medium text-white shadow-sm transition-colors hover:bg-[var(--accent-hover)] disabled:cursor-not-allowed disabled:bg-[var(--text-faint)] disabled:text-[var(--text-tertiary)] disabled:shadow-none"
          >
            Save
          </button>
        </div>
      </footer>
    </div>
  );
}
