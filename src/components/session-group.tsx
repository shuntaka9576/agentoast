import { useState } from "react";
import { ChevronDown, ChevronRight, Folder } from "lucide-react";
import type { TmuxPane, TmuxPaneGroup } from "@/lib/types";
import { SessionCard } from "./session-card";

interface SessionGroupProps {
  group: TmuxPaneGroup;
  selectedPaneId: string | null;
  flatPanes: TmuxPane[];
}

export function SessionGroup({
  group,
  selectedPaneId,
  flatPanes,
}: SessionGroupProps) {
  const [expanded, setExpanded] = useState(true);
  const activeCount = group.panes.filter((p) => p.agentType !== null).length;

  return (
    <div className="border-b border-[var(--border-subtle)] last:border-b-0">
      <button
        tabIndex={-1}
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
          {group.repoName}
        </span>
        {activeCount > 0 && (
          <span className="text-[10px] text-green-500 font-medium">
            {activeCount} active
          </span>
        )}
        <span className="text-[10px] text-[var(--text-muted)]">
          {group.panes.length}
        </span>
      </button>

      {expanded && (
        <div>
          {group.panes.map((pane) => {
            const navIndex = flatPanes.findIndex(
              (f) => f.paneId === pane.paneId,
            );
            return (
              <SessionCard
                key={pane.paneId}
                pane={pane}
                isSelected={pane.paneId === selectedPaneId}
                navIndex={navIndex}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
