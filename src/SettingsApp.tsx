import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RestartBanner } from "@/components/restart-banner";
import { SettingsRow, SettingsSection } from "@/components/settings-section";
import { ShortcutRecorder } from "@/components/shortcut-recorder";
import type { SaveSettingsResult, SettingsPayload } from "@/lib/settings-types";
import { RESTART_REQUIRED_FIELDS } from "@/lib/settings-types";

type Status = { kind: "loading" } | { kind: "error"; message: string } | { kind: "ready" };

type SaveState =
  | { kind: "idle" }
  | { kind: "saving" }
  | { kind: "saved"; restartRequired: boolean }
  | { kind: "error"; message: string };

const inputBase =
  "h-7 rounded-md border border-[var(--border-primary)] bg-[var(--panel-bg)] px-2 text-xs text-[var(--text-primary)] outline-none focus:border-[var(--badge-focus-text)]";

const MIN_TOAST_DURATION_MS = 500;

export function SettingsApp() {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [original, setOriginal] = useState<SettingsPayload | null>(null);
  const [draft, setDraft] = useState<SettingsPayload | null>(null);
  const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });
  const [reservedShortcuts, setReservedShortcuts] = useState<string[]>([]);

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
  }, []);

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
      // Clear saved state when user edits again so the banner stays authoritative.
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

  const handleCancel = () => {
    if (original) setDraft(original);
    setSaveState({ kind: "idle" });
  };

  const handleClose = () => {
    void invoke("hide_settings");
  };

  if (status.kind === "loading") {
    return (
      <div className="flex h-screen items-center justify-center text-xs text-[var(--text-tertiary)]">
        Loading settings…
      </div>
    );
  }

  if (status.kind === "error" || !draft) {
    return (
      <div className="flex h-screen items-center justify-center px-6 text-center text-xs text-[var(--delete-hover-text)]">
        Failed to load settings: {status.kind === "error" ? status.message : "unknown error"}
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-[var(--panel-bg)] text-[var(--text-primary)]">
      {showRestartBanner && <RestartBanner onRestart={handleRestart} />}

      <div className="flex-1 overflow-y-auto">
        <SettingsSection
          title="Toast"
          description="Transient popup shown at the top-right when a new notification arrives."
        >
          <SettingsRow
            label="Display duration (ms)"
            hint="How long a toast stays visible. Ignored when 'Keep until clicked' is on."
            htmlFor="toast-duration"
          >
            <input
              id="toast-duration"
              type="number"
              min={MIN_TOAST_DURATION_MS}
              step={500}
              className={`${inputBase} w-24 text-right`}
              value={draft.toastDurationMs}
              onChange={(e) => {
                const next = Number(e.target.value);
                if (!Number.isNaN(next)) updateField("toastDurationMs", next);
              }}
              onBlur={(e) => {
                // A non-persistent toast with `duration_ms < 500` would be
                // dismissed before the user could see it. Snap back to the
                // floor on blur so the field never stays in an unusable state.
                const v = Number(e.target.value);
                if (Number.isNaN(v) || v < MIN_TOAST_DURATION_MS) {
                  updateField("toastDurationMs", MIN_TOAST_DURATION_MS);
                }
              }}
            />
          </SettingsRow>
          <SettingsRow
            label="Keep until clicked"
            hint="When enabled, toasts do not auto-dismiss."
            htmlFor="toast-persistent"
          >
            <input
              id="toast-persistent"
              type="checkbox"
              className="h-4 w-4 accent-[var(--badge-focus-text)]"
              checked={draft.toastPersistent}
              onChange={(e) => updateField("toastPersistent", e.target.checked)}
            />
          </SettingsRow>
        </SettingsSection>

        <SettingsSection
          title="Keyboard shortcut"
          description="Global shortcut that toggles the main panel."
        >
          <SettingsRow
            label="Toggle panel"
            hint="Click, then press the shortcut. Use Clear to disable."
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
          title="Editor"
          description="Editor used by `agentoast config`. Falls back to $EDITOR then vim when empty."
        >
          <SettingsRow label="Command" htmlFor="editor-cmd">
            <input
              id="editor-cmd"
              type="text"
              spellCheck={false}
              placeholder="(auto-detect)"
              className={`${inputBase} w-52`}
              value={draft.editor}
              onChange={(e) => updateField("editor", e.target.value)}
            />
          </SettingsRow>
        </SettingsSection>
      </div>

      <footer className="flex items-center justify-between gap-3 border-t border-[var(--border-primary)] px-5 py-3">
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
            onClick={handleClose}
            className="rounded-md px-3 py-1 text-xs text-[var(--text-secondary)] hover:bg-[var(--hover-bg-strong)]"
          >
            Close
          </button>
          <button
            type="button"
            onClick={handleCancel}
            disabled={!dirty || saveState.kind === "saving"}
            className="rounded-md px-3 py-1 text-xs text-[var(--text-secondary)] hover:bg-[var(--hover-bg-strong)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            Revert
          </button>
          <button
            type="button"
            onClick={() => {
              void handleSave();
            }}
            disabled={!dirty || saveState.kind === "saving"}
            className="rounded-md bg-[var(--badge-focus-text)] px-3 py-1 text-xs font-medium text-white hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            Save
          </button>
        </div>
      </footer>
    </div>
  );
}
