import { Bell, BellOff, Trash2 } from "lucide-react";

interface PanelHeaderProps {
  unreadCount: number;
  globalMuted: boolean;
  onDeleteAll: () => void;
  onToggleGlobalMute: () => void;
}

export function PanelHeader({
  unreadCount,
  globalMuted,
  onDeleteAll,
  onToggleGlobalMute,
}: PanelHeaderProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-primary)]">
      <div className="flex items-center gap-2">
        {unreadCount > 0 && (
          <span className="px-1.5 py-0.5 text-[10px] font-medium bg-[var(--badge-count-bg)] text-[var(--badge-count-text)] rounded-full">
            {unreadCount}
          </span>
        )}
      </div>
      <div className="flex items-center gap-1">
        <button
          tabIndex={-1}
          onClick={onToggleGlobalMute}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--hover-bg-strong)] transition-colors"
          title={globalMuted ? "Unmute notifications" : "Mute notifications"}
        >
          {globalMuted ? <BellOff size={14} /> : <Bell size={14} />}
        </button>
        <button
          tabIndex={-1}
          onClick={onDeleteAll}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:text-[var(--delete-hover-text)] hover:bg-[var(--hover-bg-strong)] transition-colors"
          title="Delete all"
        >
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}
