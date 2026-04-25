import { invoke } from "@tauri-apps/api/core";
import { IconPreset } from "@/components/icons/source-icon";
import { Toggle } from "@/components/toggle";
import type { Icon } from "@/lib/types";
import type { CliInstallStatus } from "@/lib/settings-types";

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

export type CliInstallState =
  | { kind: "idle" }
  | { kind: "installing" }
  | { kind: "installed"; replaced: boolean }
  | { kind: "error"; message: string };

interface OnboardingProps {
  variant: "onboarding";
  installCli: boolean;
  onInstallCliChange: (value: boolean) => void;
  cliStatus: CliInstallStatus | null;
}

interface SettingsProps {
  variant: "settings";
  cliStatus: CliInstallStatus | null;
  cliInstallState: CliInstallState;
  onInstallCliClick: () => void;
}

type NotificationSetupProps = OnboardingProps | SettingsProps;

export function NotificationSetup(props: NotificationSetupProps) {
  return (
    <div className="flex flex-col gap-3">
      <CliBlock {...props} />
      <AgentBlock />
    </div>
  );
}

function CliBlock(props: NotificationSetupProps) {
  const { cliStatus } = props;
  const installed = cliStatus?.pointsToCurrentExe ?? false;
  // GUI app sees launchd's PATH, not the shell's. If the symlink already
  // points to the current exe, hooks (run from the user's terminal) will
  // find it via the shell's PATH — suppress the false-positive warning.
  const showPathWarning =
    cliStatus !== null &&
    !cliStatus.onPath &&
    !installed &&
    (props.variant === "settings" || props.installCli);

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-[var(--border-subtle)] p-4">
      <div className="flex flex-col gap-0.5">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-[var(--text-primary)]">
            Install <code className="font-mono text-xs">agentoast</code> CLI
          </span>
          {installed ? (
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
          Hook scripts call <code className="font-mono">agentoast hook</code>, so the CLI must be on
          your <code className="font-mono">PATH</code>.
        </span>
      </div>

      {props.variant === "onboarding" && !installed && (
        <div className="flex items-center gap-2">
          <Toggle
            checked={props.installCli}
            onChange={props.onInstallCliChange}
            ariaLabel="Install CLI symlink"
          />
          <span className="text-[12px] text-[var(--text-secondary)]">
            Symlink to <code className="font-mono text-[11px]">~/.local/bin/agentoast</code>
          </span>
        </div>
      )}

      {props.variant === "settings" && (
        <SettingsCliControls
          cliStatus={cliStatus}
          cliInstallState={props.cliInstallState}
          onInstallCliClick={props.onInstallCliClick}
        />
      )}

      {showPathWarning && (
        <p className="rounded-md bg-[var(--hover-bg)] px-3 py-2 text-[11px] leading-relaxed text-[var(--text-tertiary)]">
          <span className="text-[var(--text-secondary)]">
            ⚠ <code className="font-mono">~/.local/bin</code> is not in your{" "}
            <code className="font-mono">PATH</code>.
          </span>{" "}
          Add <code className="font-mono">{`export PATH="$HOME/.local/bin:$PATH"`}</code> to your
          shell rc.
        </p>
      )}
    </div>
  );
}

interface SettingsCliControlsProps {
  cliStatus: CliInstallStatus | null;
  cliInstallState: CliInstallState;
  onInstallCliClick: () => void;
}

function SettingsCliControls({
  cliStatus,
  cliInstallState,
  onInstallCliClick,
}: SettingsCliControlsProps) {
  const installed = cliStatus?.pointsToCurrentExe ?? false;
  const installing = cliInstallState.kind === "installing";

  const statusHint = cliStatus
    ? installed
      ? `Linked at ${cliStatus.targetPath}`
      : cliStatus.installed
        ? `${cliStatus.targetPath} exists but points elsewhere — reinstall to update.`
        : `Not installed (will create ${cliStatus.targetPath})`
    : "Checking…";

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onInstallCliClick}
          disabled={installing || cliStatus === null}
          className="rounded-md bg-[var(--accent)] px-3 py-1.5 text-xs font-medium text-white shadow-sm transition-colors hover:bg-[var(--accent-hover)] disabled:cursor-not-allowed disabled:bg-[var(--text-faint)] disabled:text-[var(--text-tertiary)] disabled:shadow-none"
        >
          {installing ? "Installing…" : installed ? "Reinstall" : "Install"}
        </button>
        <span className="text-[11px] text-[var(--text-tertiary)]">{statusHint}</span>
      </div>
      {cliInstallState.kind === "installed" && (
        <span className="text-[11px] text-[var(--text-tertiary)]">
          {cliInstallState.replaced ? "Symlink replaced." : "Symlink created."}
        </span>
      )}
      {cliInstallState.kind === "error" && (
        <span className="text-[11px] text-[var(--delete-hover-text)]">
          Install failed: {cliInstallState.message}
        </span>
      )}
    </div>
  );
}

function AgentBlock() {
  return (
    <div className="flex flex-col gap-2 rounded-xl border border-[var(--border-subtle)] p-4">
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
  );
}
