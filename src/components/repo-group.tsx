import { Bell, BellOff, ChevronDown, ChevronRight, GitBranch, Folder, FolderGit2, Trash2, Users } from "lucide-react";
import { useMemo } from "react";
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

  const runningCount = paneItems.filter((pi) => pi.pane.agentStatus === "running").length;
  const idleCount = paneItems.filter((pi) => pi.pane.agentStatus === "idle").length;
  const waitingCount = paneItems.filter((pi) => pi.pane.agentStatus === "waiting").length;

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
        {/* Line 3: agent status counts */}
        {(runningCount > 0 || idleCount > 0 || waitingCount > 0) && (
          <div className="flex items-center gap-3 pl-[33px] mt-0.5">
            {runningCount > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-green-500 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-green-500" />
                {runningCount}
              </span>
            )}
            {idleCount > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-[var(--text-muted)] font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-[var(--text-muted)]" />
                {idleCount}
              </span>
            )}
            {waitingCount > 0 && (
              <span className="flex items-center gap-1 text-[10px] text-amber-500 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />
                {waitingCount}
              </span>
            )}
          </div>
        )}
      </button>

      {expanded && (
        <ExpandedPanes
          paneItems={paneItems}
          flatItems={flatItems}
          newIds={newIds}
          selectedId={selectedId}
          selectedPaneId={selectedPaneId}
          onDeleteNotification={onDeleteNotification}
        />
      )}
    </div>
  );
}

interface ExpandedPanesProps {
  paneItems: PaneItem[];
  flatItems: FlatItem[];
  newIds: Set<number>;
  selectedId: number | null;
  selectedPaneId: string | null;
  onDeleteNotification: (id: number) => void;
}

function ExpandedPanes({
  paneItems,
  flatItems,
  newIds,
  selectedId,
  selectedPaneId,
  onDeleteNotification,
}: ExpandedPanesProps) {
  // Group panes by Agent Teams membership (session:window key)
  const { teamGroups, soloItems } = useMemo(() => {
    const teamMap = new Map<string, PaneItem[]>();
    const solo: PaneItem[] = [];
    for (const pi of paneItems) {
      const { teamRole, sessionName, windowName } = pi.pane;
      if (teamRole) {
        const key = `${sessionName}:${windowName}`;
        if (!teamMap.has(key)) teamMap.set(key, []);
        teamMap.get(key)!.push(pi);
      } else {
        solo.push(pi);
      }
    }
    return { teamGroups: Array.from(teamMap.values()), soloItems: solo };
  }, [paneItems]);

  const renderPaneCard = (pi: PaneItem) => {
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
  };

  return (
    <div className="py-0.5">
      {teamGroups.map((teamPanes) => {
        const lead = teamPanes.find((pi) => pi.pane.teamRole === "lead");
        const teammates = teamPanes.filter((pi) => pi.pane.teamRole === "teammate");
        const teamKey = lead
          ? `${lead.pane.sessionName}:${lead.pane.windowName}`
          : teamPanes[0].pane.paneId;
        const memberCount = teamPanes.length;
        const hasSelectedMember = teamPanes.some(
          (pi) =>
            pi.pane.paneId === selectedPaneId ||
            (pi.notification !== null && pi.notification.id === selectedId),
        );
        return (
          <div key={teamKey} className={cn(
            "mx-3 my-1 border rounded-md",
            hasSelectedMember ? "border-violet-500/60" : "border-[var(--border-subtle)]",
          )}>
            {/* Team sub-header */}
            <div className="flex items-center gap-1.5 px-2 py-1 border-b border-[var(--border-subtle)]">
              <Users size={10} className="text-[var(--text-muted)] flex-shrink-0" />
              <span className="text-[10px] text-[var(--text-muted)]">
                Agent Teams({memberCount})
              </span>
            </div>
            {/* Sort: lead first, then teammates */}
            {[
              ...(lead ? [lead] : []),
              ...teammates,
              ...teamPanes.filter((pi) => pi.pane.teamRole !== "lead" && pi.pane.teamRole !== "teammate"),
            ].map(renderPaneCard)}
          </div>
        );
      })}
      {soloItems.map(renderPaneCard)}
    </div>
  );
}
