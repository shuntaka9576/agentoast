import { Bell, BellOff, ChevronDown, ChevronRight, GitBranch, Folder, FolderGit2, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Notification, PaneItem, FlatItem } from "@/lib/types";
import { NotificationCard } from "./notification-card";
import { PaneCard } from "./pane-card";

interface RepoGroupProps {
  groupKey: string;
  repoName: string;
  gitBranch: string | null;
  paneItems: PaneItem[];
  orphanNotifications: Notification[];
  expanded: boolean;
  isMuted: boolean;
  isHeaderSelected: boolean;
  headerNavIndex: number;
  newIds: Set<number>;
  selectedId: number | null;
  selectedPaneId: string | null;
  flatItems: FlatItem[];
  onDeleteNotification: (id: number) => void;
  onDeleteGroup: (groupKey: string) => void;
  onToggleGroupMute: (groupKey: string) => void;
  onToggleExpand: () => void;
}

export function RepoGroup({
  groupKey,
  repoName,
  gitBranch,
  paneItems,
  orphanNotifications,
  expanded,
  isMuted,
  isHeaderSelected,
  headerNavIndex,
  newIds,
  selectedId,
  selectedPaneId,
  flatItems,
  onDeleteNotification,
  onDeleteGroup,
  onToggleGroupMute,
  onToggleExpand,
}: RepoGroupProps) {

  const totalNotifications =
    paneItems.filter((pi) => pi.notification !== null).length +
    orphanNotifications.length;

  const activeSessions = paneItems.filter((pi) => pi.pane.agentType !== null).length;

  return (
    <div className="border-b border-[var(--border-subtle)] last:border-b-0">
      <button
        data-nav-index={headerNavIndex}
        onClick={onToggleExpand}
        className={cn(
          "w-full flex items-center gap-2 px-4 py-2 text-left hover:bg-[var(--hover-bg)] transition-colors",
          isHeaderSelected && "bg-[var(--hover-bg)]",
        )}
      >
        {expanded ? (
          <ChevronDown size={12} className="text-[var(--text-muted)]" />
        ) : (
          <ChevronRight size={12} className="text-[var(--text-muted)]" />
        )}
        {gitBranch ? (
          <FolderGit2 size={13} className="text-[var(--text-tertiary)]" />
        ) : (
          <Folder size={13} className="text-[var(--text-tertiary)]" />
        )}
        <span className="text-xs font-medium text-[var(--text-secondary)] truncate flex-1">
          {repoName}
        </span>
        {gitBranch && (
          <span className="flex items-center gap-0.5 text-[10px] text-[var(--text-muted)] flex-shrink-0">
            <GitBranch size={10} />
            <span className="max-w-[80px] truncate">{gitBranch}</span>
          </span>
        )}
        {activeSessions > 0 && (
          <span className="text-[10px] text-green-500 font-medium">
            {activeSessions} agent{activeSessions > 1 ? "s" : ""}
          </span>
        )}
        {totalNotifications > 0 && (
          <span className="text-[10px] text-[var(--text-muted)]">
            {totalNotifications}
          </span>
        )}
        {totalNotifications > 0 && (
          <>
            <button
              tabIndex={-1}
              onClick={(e) => {
                e.stopPropagation();
                onToggleGroupMute(groupKey);
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
                onDeleteGroup(groupKey);
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
          {paneItems.map((pi) => {
            const navIndex = flatItems.findIndex(
              (f) => f.type === "pane-item" && f.paneItem.pane.paneId === pi.pane.paneId,
            );
            const isSelected =
              pi.pane.paneId === selectedPaneId ||
              (pi.notification !== null && pi.notification.id === selectedId);
            return (
              <PaneCard
                key={`pane-${pi.pane.paneId}`}
                paneItem={pi}
                isNew={pi.notification !== null && newIds.has(pi.notification.id)}
                isSelected={isSelected}
                navIndex={navIndex}
                onDeleteNotification={onDeleteNotification}
              />
            );
          })}
          {orphanNotifications.map((n) => {
            const navIndex = flatItems.findIndex(
              (f) => f.type === "orphan-notification" && f.notification.id === n.id,
            );
            return (
              <NotificationCard
                key={n.id}
                notification={n}
                isNew={newIds.has(n.id)}
                isSelected={n.id === selectedId}
                navIndex={navIndex}
                onDelete={onDeleteNotification}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
