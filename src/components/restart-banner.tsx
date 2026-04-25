import { Sparkles } from "lucide-react";

interface RestartBannerProps {
  onRestart: () => void;
}

export function RestartBanner({ onRestart }: RestartBannerProps) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-[var(--border-subtle)] bg-[var(--banner-warn-bg)] px-5 py-2.5">
      <div className="flex items-center gap-2 text-[11px] text-[var(--text-secondary)]">
        <Sparkles size={13} className="text-[var(--accent)]" />
        <span>Some changes need a restart to take effect.</span>
      </div>
      <button
        type="button"
        onClick={onRestart}
        className="rounded-md bg-[var(--accent)] px-2.5 py-1 text-[11px] font-medium text-white transition-colors hover:bg-[var(--accent-hover)]"
      >
        Restart
      </button>
    </div>
  );
}
