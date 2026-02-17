import { useState } from "react";
import { Bell, BellOff, ChevronDown, ChevronRight, Folder, Trash2 } from "lucide-react";
import type { Notification, TmuxPane, FlatItem } from "@/lib/types";
import { NotificationCard } from "./notification-card";
import { SessionIndicator } from "./session-card";

interface RepoGroupProps {
  groupName: string;
  activeSessions: TmuxPane[];
  notifications: Notification[];
  isMuted: boolean;
  newIds: Set<number>;
  selectedId: number | null;
  selectedPaneId: string | null;
  flatItems: FlatItem[];
  onDelete: (id: number) => void;
  onDeleteGroup: (groupName: string) => void;
  onToggleGroupMute: (groupName: string) => void;
}

export function RepoGroup({
  groupName,
  activeSessions,
  notifications,
  isMuted,
  newIds,
  selectedId,
  selectedPaneId,
  flatItems,
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
          {groupName}
        </span>
        {activeSessions.length > 0 && (
          <span className="text-[10px] text-green-500 font-medium">
            {activeSessions.length} agent{activeSessions.length > 1 ? "s" : ""}
          </span>
        )}
        {notifications.length > 0 && (
          <span className="text-[10px] text-[var(--text-muted)]">
            {notifications.length}
          </span>
        )}
        {notifications.length > 0 && (
          <>
            <button
              tabIndex={-1}
              onClick={(e) => {
                e.stopPropagation();
                onToggleGroupMute(groupName);
              }}
              className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
              title={isMuted ? "Unmute group" : "Mute group"}
            >
              {isMuted ? <BellOff size={11} /> : <Bell size={11} />}
            </button>
            <button
              tabIndex={-1}
              onClick={(e) => {
                e.stopPropagation();
                onDeleteGroup(groupName);
              }}
              className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
            >
              <Trash2 size={11} />
            </button>
          </>
        )}
      </button>

      {expanded && (
        <div>
          {activeSessions.map((pane) => {
            const navIndex = flatItems.findIndex(
              (f) => f.type === "session" && f.pane.paneId === pane.paneId,
            );
            return (
              <SessionIndicator
                key={`session-${pane.paneId}`}
                pane={pane}
                isSelected={pane.paneId === selectedPaneId}
                navIndex={navIndex}
              />
            );
          })}
          {notifications.map((n) => {
            const navIndex = flatItems.findIndex(
              (f) => f.type === "notification" && f.notification.id === n.id,
            );
            return (
              <NotificationCard
                key={n.id}
                notification={n}
                isNew={newIds.has(n.id)}
                isSelected={n.id === selectedId}
                navIndex={navIndex}
                onDelete={onDelete}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
