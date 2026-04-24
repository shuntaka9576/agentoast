import { RefreshCcw } from "lucide-react";

interface RestartBannerProps {
  onRestart: () => void;
}

export function RestartBanner({ onRestart }: RestartBannerProps) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-[var(--border-focus)] bg-[var(--toast-focus-bg)] px-5 py-2.5">
      <div className="flex items-center gap-2 text-xs text-[var(--text-primary)]">
        <RefreshCcw size={14} className="text-[var(--badge-focus-text)]" />
        <span>
          Some changes require restarting Agentoast to take effect.
        </span>
      </div>
      <button
        type="button"
        onClick={onRestart}
        className="rounded-md bg-[var(--badge-focus-text)] px-2.5 py-1 text-xs font-medium text-white hover:opacity-90"
      >
        Quit &amp; Relaunch
      </button>
    </div>
  );
}
