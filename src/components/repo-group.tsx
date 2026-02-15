import { useState } from "react";
import { Bell, BellOff, ChevronDown, ChevronRight, Folder, Trash2 } from "lucide-react";
import type { NotificationGroup } from "@/lib/types";
import { NotificationCard } from "./notification-card";

interface RepoGroupProps {
  group: NotificationGroup;
  isMuted: boolean;
  newIds: Set<number>;
  onDelete: (id: number) => void;
  onDeleteGroup: (groupName: string) => void;
  onToggleGroupMute: (groupName: string) => void;
}

export function RepoGroup({
  group,
  isMuted,
  newIds,
  onDelete,
  onDeleteGroup,
  onToggleGroupMute,
}: RepoGroupProps) {
  const [expanded, setExpanded] = useState(true);

  return (
    <div className="border-b border-[var(--border-subtle)] last:border-b-0">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-4 py-2 text-left hover:bg-[var(--hover-bg)] transition-colors"
      >
        {expanded ? (
          <ChevronDown size={12} className="text-[var(--text-muted)]" />
        ) : (
          <ChevronRight size={12} className="text-[var(--text-muted)]" />
        )}
        <Folder size={13} className="text-[var(--text-tertiary)]" />
        <span className="text-xs font-medium text-[var(--text-secondary)] truncate flex-1">
          {group.groupName}
        </span>
        <span className="text-[10px] text-[var(--text-muted)]">
          {group.notifications.length}
        </span>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggleGroupMute(group.groupName);
          }}
          className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
          title={isMuted ? "Unmute group" : "Mute group"}
        >
          {isMuted ? <BellOff size={11} /> : <Bell size={11} />}
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onDeleteGroup(group.groupName);
          }}
          className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
        >
          <Trash2 size={11} />
        </button>
      </button>

      {expanded && (
        <div>
          {group.notifications.map((n) => (
            <NotificationCard
              key={n.id}
              notification={n}
              isNew={newIds.has(n.id)}
              onDelete={onDelete}
            />
          ))}
        </div>
      )}
    </div>
  );
}
