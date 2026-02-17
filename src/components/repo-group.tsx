import { Bell, BellOff, ChevronDown, ChevronRight, GitBranch, Folder, FolderGit2, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { PaneItem, FlatItem } from "@/lib/types";
import { PaneCard } from "./pane-card";

interface RepoGroupProps {
  groupKey: string;
  repoName: string;
  gitBranch: string | null;
  paneItems: PaneItem[];
  expanded: boolean;
  isMuted: boolean;
  isHeaderSelected: boolean;
  headerNavIndex: number;
  newIds: Set<number>;
  selectedId: number | null;
  selectedPaneId: string | null;
  flatItems: FlatItem[];
  onDeleteNotification: (id: number) => void;
  onDeleteByPanes: (paneIds: string[]) => void;
  onToggleRepoMute: (repoPath: string) => void;
  onToggleExpand: () => void;
}

export function RepoGroup({
  groupKey,
  repoName,
  gitBranch,
  paneItems,
  expanded,
  isMuted,
  isHeaderSelected,
  headerNavIndex,
  newIds,
  selectedId,
  selectedPaneId,
  flatItems,
  onDeleteNotification,
  onDeleteByPanes,
  onToggleRepoMute,
  onToggleExpand,
}: RepoGroupProps) {

  const totalNotifications =
    paneItems.filter((pi) => pi.notification !== null).length;

  const activeSessions = paneItems.filter((pi) => pi.pane.agentType !== null).length;

  return (
    <div className="border-b border-[var(--border-subtle)] last:border-b-0">
      <button
        tabIndex={-1}
        data-nav-index={headerNavIndex}
        onClick={onToggleExpand}
        className={cn(
          "w-full px-4 py-2 text-left hover:bg-[var(--hover-bg)]",
          isHeaderSelected && "bg-[var(--hover-bg)]",
        )}
      >
        {/* Line 1: chevron + icon + repoName + notification actions */}
        <div className="flex items-center gap-2">
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
          <button
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              onToggleRepoMute(groupKey);
            }}
            className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
            title={isMuted ? "Unmute repo" : "Mute repo"}
          >
            {isMuted ? <BellOff size={11} /> : <Bell size={11} />}
          </button>
          {totalNotifications > 0 && (
            <button
              tabIndex={-1}
              onClick={(e) => {
                e.stopPropagation();
                const paneIds = paneItems.map((pi) => pi.pane.paneId);
                onDeleteByPanes(paneIds);
              }}
              className="p-0.5 rounded hover:bg-[var(--hover-bg-strong)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
            >
              <Trash2 size={11} />
            </button>
          )}
        </div>
        {/* Line 2: branch */}
        {gitBranch && (
          <div className="flex items-center pl-[33px] mt-0.5">
            <span className="flex items-center gap-0.5 text-[10px] text-[var(--text-muted)] truncate">
              <GitBranch size={10} className="flex-shrink-0" />
              {gitBranch}
            </span>
          </div>
        )}
        {/* Line 3: notification count + agent count */}
        {(totalNotifications > 0 || activeSessions > 0) && (
          <div className="flex items-center gap-3 pl-[33px] mt-0.5">
            {totalNotifications > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[#FF9500] font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-[#FF9500]" />
                {totalNotifications}
              </span>
            )}
            {activeSessions > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-green-500 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-green-500" />
                {activeSessions}
              </span>
            )}
          </div>
        )}
      </button>

      {expanded && (
        <div className="py-0.5">
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
        </div>
      )}
    </div>
  );
}
