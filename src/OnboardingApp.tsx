import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight } from "lucide-react";
import { IconPreset } from "@/components/icons/source-icon";
import { ShortcutRecorder } from "@/components/shortcut-recorder";
import { Toggle } from "@/components/toggle";
import type { Icon } from "@/lib/types";
import type { CliInstallStatus, SettingsPayload } from "@/lib/settings-types";

type Step = 0 | 1 | 2;

interface AgentEntry {
  id: string;
  label: string;
  icon: Icon;
}

const AGENTS: AgentEntry[] = [
  { id: "claude-code", label: "Claude Code", icon: "claude-code" },
  { id: "codex", label: "Codex", icon: "codex" },
  { id: "copilot-cli", label: "Copilot CLI", icon: "copilot-cli" },
  { id: "opencode", label: "opencode", icon: "opencode" },
];

export function OnboardingApp() {
  const [step, setStep] = useState<Step>(0);
  const [shortcut, setShortcut] = useState<string>("");
  const [autostart, setAutostart] = useState<boolean>(false);
  const [reservedShortcuts, setReservedShortcuts] = useState<string[]>([]);
  const [installCli, setInstallCli] = useState<boolean>(true);
  const [cliStatus, setCliStatus] = useState<CliInstallStatus | null>(null);
  const [saving, setSaving] = useState(false);
  // Block "Get Started" until initial state is fetched. Without this, a user
  // can finish before `shortcut` is hydrated, persisting an empty string and
  // wiping their existing shortcut.
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => {
    Promise.all([
      invoke<SettingsPayload>("get_settings").then((payload) => {
        setShortcut(payload.togglePanelShortcut);
        setAutostart(payload.autostartEnabled);
      }),
      invoke<string[]>("get_reserved_shortcuts").then((list) => {
        setReservedShortcuts(list);
      }),
      invoke<CliInstallStatus>("get_cli_install_status").then((status) => {
        setCliStatus(status);
      }),
    ])
      .catch((err) => console.warn("Onboarding hydrate failed:", err))
      .finally(() => setHydrated(true));
  }, []);

  // `cli` lets Skip force CLI install off without waiting for setState to
  // settle (avoids a stale-state race when calling finish from Skip handler).
  const finish = async (cli: boolean) => {
    if (saving || !hydrated) return;
    setSaving(true);
    try {
      const current = await invoke<SettingsPayload>("get_settings");
      await invoke("save_settings", {
        payload: {
          ...current,
          togglePanelShortcut: shortcut,
          autostartEnabled: autostart,
        },
      });
      if (cli && !cliStatus?.pointsToCurrentExe) {
        try {
          await invoke("install_cli_symlink");
        } catch (cliErr) {
          // Non-fatal: GUI works without CLI on PATH. Log so users can debug
          // via the Settings → "Install CLI" button later.
          console.warn("Failed to install CLI symlink:", cliErr);
        }
      }
      await invoke("complete_onboarding");
      // Reveal the main panel right after the onboarding window hides so the
      // user notices where agentoast lives in the menu bar.
      await invoke("show_panel");
    } catch (err) {
      console.error("Failed to complete onboarding:", err);
    } finally {
      setSaving(false);
    }
  };

  const busy = !hydrated || saving;

  return (
    <div className="flex h-screen flex-col overflow-hidden rounded-2xl border border-[var(--border-primary)] bg-[var(--panel-bg)] text-[var(--text-primary)] shadow-2xl">
      <div data-tauri-drag-region className="h-8 shrink-0" />
      <div className="flex flex-1 flex-col overflow-hidden">
        {step === 0 && <WelcomeStep onContinue={() => setStep(1)} />}
        {step === 1 && (
          <ShortcutStep
            shortcut={shortcut}
            onShortcutChange={setShortcut}
            reservedShortcuts={reservedShortcuts}
            autostart={autostart}
            onAutostartChange={setAutostart}
            onContinue={() => setStep(2)}
            disabled={busy}
          />
        )}
        {step === 2 && (
          <HooksStep
            installCli={installCli}
            onInstallCliChange={setInstallCli}
            cliStatus={cliStatus}
            onSkip={() => {
              void finish(false);
            }}
            onFinish={() => {
              void finish(installCli);
            }}
            disabled={busy}
          />
        )}
      </div>
    </div>
  );
}

function WelcomeStep({ onContinue }: { onContinue: () => void }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-6 px-8 text-center">
      <div className="flex h-24 w-24 items-center justify-center rounded-3xl bg-[var(--accent)] text-white shadow-md">
        <IconPreset icon="agentoast" size={64} className="text-white" />
      </div>
      <div className="flex flex-col items-center gap-2">
        <h1 className="text-3xl font-bold tracking-tight">agentoast</h1>
        <p className="max-w-xs text-sm text-[var(--text-tertiary)]">
          Manage notifications from your AI coding agents right in the menu bar.
        </p>
      </div>
      <button
        type="button"
        onClick={onContinue}
        className="mt-2 inline-flex h-11 items-center gap-2 rounded-full bg-[var(--accent)] px-12 text-sm font-medium text-white shadow-sm transition-colors hover:bg-[var(--accent-hover)]"
      >
        Get Started
        <ArrowRight size={16} />
      </button>
    </div>
  );
}

interface ShortcutStepProps {
  shortcut: string;
  onShortcutChange: (value: string) => void;
  reservedShortcuts: string[];
  autostart: boolean;
  onAutostartChange: (value: boolean) => void;
  onContinue: () => void;
  disabled: boolean;
}

function ShortcutStep({
  shortcut,
  onShortcutChange,
  reservedShortcuts,
  autostart,
  onAutostartChange,
  onContinue,
  disabled,
}: ShortcutStepProps) {
  return (
    <div className="flex h-full flex-col px-6 pt-3 pb-6">
      <div className="flex flex-col gap-2">
        <h1 className="text-xl font-bold">Set up your shortcut</h1>
        <p className="text-sm text-[var(--text-tertiary)]">
          Pick a keyboard shortcut to quickly open the panel from the menu bar.
        </p>
      </div>

      <div className="mt-6 flex flex-col items-center gap-3 rounded-2xl bg-[var(--hover-bg)] px-6 py-8">
        <ShortcutRecorder
          value={shortcut}
          onChange={onShortcutChange}
          reservedShortcuts={reservedShortcuts}
          variant="large"
        />
        <p className="text-xs text-[var(--text-tertiary)]">
          You can change this anytime in Settings
        </p>
      </div>

      <div className="mt-auto flex items-center justify-between gap-3 pt-6">
        <div className="flex items-center gap-2">
          <Toggle checked={autostart} onChange={onAutostartChange} ariaLabel="Launch at login" />
          <span className="text-sm text-[var(--text-primary)]">Launch at login</span>
          <span className="rounded-full bg-[rgba(59,130,246,0.15)] px-2 py-0.5 text-[10px] font-medium text-[#3b82f6]">
            Recommended
          </span>
        </div>
        <button
          type="button"
          onClick={onContinue}
          disabled={disabled}
          className="inline-flex h-10 items-center gap-1.5 rounded-full bg-[var(--text-primary)] px-6 text-sm font-medium text-[var(--panel-bg)] shadow-sm transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
        >
          Continue
          <ArrowRight size={14} />
        </button>
      </div>
    </div>
  );
}

interface HooksStepProps {
  installCli: boolean;
  onInstallCliChange: (value: boolean) => void;
  cliStatus: CliInstallStatus | null;
  onSkip: () => void;
  onFinish: () => void;
  disabled: boolean;
}

function HooksStep({
  installCli,
  onInstallCliChange,
  cliStatus,
  onSkip,
  onFinish,
  disabled,
}: HooksStepProps) {
  const showPathWarning = installCli && cliStatus !== null && !cliStatus.onPath;

  return (
    <div className="flex h-full flex-col px-6 pt-3 pb-6">
      <div className="flex flex-col gap-2">
        <h1 className="text-xl font-bold">Enable notifications</h1>
        <p className="text-sm text-[var(--text-tertiary)]">
          Connect your AI coding agent so it can send notifications to agentoast. You can skip this
          and set it up later.
        </p>
      </div>

      <div className="mt-5 flex flex-col gap-2 rounded-xl border border-[var(--border-subtle)] p-4">
        <div className="flex items-start gap-3">
          <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-[10px] font-bold text-white">
            1
          </span>
          <div className="flex flex-1 flex-col gap-2">
            <div className="flex flex-col gap-0.5">
              <div className="flex items-center gap-2">
                <span className="text-sm font-semibold text-[var(--text-primary)]">
                  Install <code className="font-mono text-xs">agentoast</code> CLI
                </span>
                {cliStatus?.pointsToCurrentExe ? (
                  <span className="rounded-full bg-[rgba(34,197,94,0.15)] px-2 py-0.5 text-[10px] font-medium text-[#22c55e]">
                    Already installed
                  </span>
                ) : (
                  <span className="rounded-full bg-[rgba(239,68,68,0.15)] px-2 py-0.5 text-[10px] font-medium text-[var(--delete-hover-text)]">
                    Required for hooks
                  </span>
                )}
              </div>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                Hook scripts call <code className="font-mono">agentoast hook</code>, so the CLI must
                be on your <code className="font-mono">PATH</code>.
              </span>
            </div>
            {!cliStatus?.pointsToCurrentExe && (
              <div className="flex items-center gap-2">
                <Toggle
                  checked={installCli}
                  onChange={onInstallCliChange}
                  ariaLabel="Install CLI symlink"
                />
                <span className="text-[12px] text-[var(--text-secondary)]">
                  Symlink to <code className="font-mono text-[11px]">~/.local/bin/agentoast</code>
                </span>
              </div>
            )}
            {showPathWarning && (
              <p className="rounded-md bg-[var(--hover-bg)] px-3 py-2 text-[11px] leading-relaxed text-[var(--text-tertiary)]">
                <span className="text-[var(--text-secondary)]">
                  ⚠ <code className="font-mono">~/.local/bin</code> is not in your{" "}
                  <code className="font-mono">PATH</code>.
                </span>{" "}
                Add <code className="font-mono">{`export PATH="$HOME/.local/bin:$PATH"`}</code> to
                your shell rc.
              </p>
            )}
          </div>
        </div>
      </div>

      <div className="mt-3 flex flex-col gap-2 rounded-xl border border-[var(--border-subtle)] p-4">
        <div className="flex items-start gap-3">
          <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-[10px] font-bold text-white">
            2
          </span>
          <div className="flex flex-1 flex-col gap-2">
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-semibold text-[var(--text-primary)]">
                Configure your agent
              </span>
              <span className="text-[11px] text-[var(--text-tertiary)]">
                Open the README for the agent you use and follow the hook setup steps.
              </span>
            </div>
            <div className="grid grid-cols-4 gap-2">
              {AGENTS.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  onClick={() => {
                    void invoke("open_hook_readme", { agent: a.id });
                  }}
                  className="flex flex-col items-center gap-1.5 rounded-xl border border-[var(--border-subtle)] bg-[var(--panel-bg)] py-3 text-[11px] text-[var(--text-secondary)] transition-colors hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)]"
                >
                  <IconPreset icon={a.icon} size={22} className="text-[var(--text-primary)]" />
                  {a.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="mt-auto flex items-center justify-between gap-3 pt-6">
        <button
          type="button"
          onClick={onSkip}
          disabled={disabled}
          className="rounded-full px-4 py-2 text-sm text-[var(--text-secondary)] transition-colors hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-60"
        >
          Skip
        </button>
        <button
          type="button"
          onClick={onFinish}
          disabled={disabled}
          className="inline-flex h-10 items-center gap-1.5 rounded-full bg-[var(--text-primary)] px-6 text-sm font-medium text-[var(--panel-bg)] shadow-sm transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
        >
          Finish
          <ArrowRight size={14} />
        </button>
      </div>
    </div>
  );
}
