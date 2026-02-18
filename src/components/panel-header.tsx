import { Bell, BellOff, Filter, Trash2 } from "lucide-react";

interface PanelHeaderProps {
  globalMuted: boolean;
  filterNotifiedOnly: boolean;
  onToggleFilter: () => void;
  onDeleteAll: () => void;
  onToggleGlobalMute: () => void;
}

export function PanelHeader({
  globalMuted,
  filterNotifiedOnly,
  onToggleFilter,
  onDeleteAll,
  onToggleGlobalMute,
}: PanelHeaderProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-primary)]">
      <button
        tabIndex={-1}
        onClick={onToggleFilter}
        className={`p-1.5 rounded-md hover:bg-[var(--hover-bg-strong)] transition-colors ${
          filterNotifiedOnly
            ? "text-[var(--text-primary)]"
            : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
        }`}
        title={filterNotifiedOnly ? "Show all" : "Show notified only"}
      >
        <Filter
          size={14}
          fill={filterNotifiedOnly ? "currentColor" : "none"}
        />
      </button>
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
