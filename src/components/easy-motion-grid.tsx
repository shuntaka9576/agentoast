import { useLayoutEffect, useRef } from "react";
import { Folder, FolderGit2, Circle } from "lucide-react";
import { cn } from "@/lib/utils";
import type { UnifiedGroup, PaneItem, TmuxPane } from "@/lib/types";

interface EasyMotionGridProps {
  groups: UnifiedGroup[];
  labels: Map<string, string>;
  prefix: string;
  onVisibilityComputed: (visibleIds: Set<string>) => void;
}

const PANE_ID_ATTR = "data-em-pane-id";

function statusDotClass(status: TmuxPane["agentStatus"]): string {
  switch (status) {
    case "running":
      return "text-green-500 fill-green-500";
    case "idle":
      return "text-[var(--text-muted)] fill-[var(--text-muted)]";
    case "waiting":
      return "text-amber-500 fill-amber-500";
    default:
      return "text-[var(--text-muted)] fill-[var(--text-muted)]";
  }
}

export function EasyMotionGrid({
  groups,
  labels,
  prefix,
  onVisibilityComputed,
}: EasyMotionGridProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  // After entering mode (once `groups` are settled), measure tile positions
  // and only keep tiles that are fully inside the viewport as jump targets.
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const containerRect = container.getBoundingClientRect();
    const visibleIds = new Set<string>();
    container.querySelectorAll(`[${PANE_ID_ATTR}]`).forEach((el) => {
      const rect = (el as HTMLElement).getBoundingClientRect();
      // Only count tiles fully inside the viewport. Labeling partially
      // clipped tiles would force the user to guess at hidden labels.
      if (rect.top >= containerRect.top && rect.bottom <= containerRect.bottom) {
        const id = (el as HTMLElement).getAttribute(PANE_ID_ATTR);
        if (id) visibleIds.add(id);
      }
    });
    onVisibilityComputed(visibleIds);
  }, [groups, onVisibilityComputed]);

  const totalTargets = groups.reduce((acc, g) => acc + g.paneItems.length, 0);

  return (
    <div ref={containerRef} className="h-full overflow-y-auto overflow-x-hidden p-1.5">
      <div className="text-[10px] text-[var(--text-muted)] mb-1 px-0.5 flex items-center justify-between">
        <span>Press label to jump · Esc to cancel</span>
        <span className="text-[var(--text-faint)]">
          {totalTargets} target{totalTargets === 1 ? "" : "s"}
        </span>
      </div>
      {groups.length === 0 ? (
        <div className="text-[11px] text-[var(--text-muted)] text-center py-8">
          No running, waiting, or notified panes
        </div>
      ) : (
        <div className="flex flex-col gap-1.5">
          {groups.map((g) => (
            <RepoSection key={g.groupKey} group={g} labels={labels} prefix={prefix} />
          ))}
        </div>
      )}
    </div>
  );
}

function RepoSection({
  group,
  labels,
  prefix,
}: {
  group: UnifiedGroup;
  labels: Map<string, string>;
  prefix: string;
}) {
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1 min-w-0 px-0.5 py-0.5 mb-0.5 border-b border-[var(--border-subtle)]">
        {group.gitBranch ? (
          <FolderGit2 size={10} className="text-[var(--text-tertiary)] flex-shrink-0" />
        ) : (
          <Folder size={10} className="text-[var(--text-tertiary)] flex-shrink-0" />
        )}
        <span
          className="text-[10px] font-medium text-[var(--text-secondary)] truncate"
          title={group.repoName}
        >
          {group.repoName}
        </span>
      </div>
      <div className="flex flex-wrap gap-1">
        {group.paneItems.map((pi) => (
          <PaneTile
            key={pi.pane.paneId}
            paneItem={pi}
            label={labels.get(pi.pane.paneId) ?? ""}
            prefix={prefix}
          />
        ))}
      </div>
    </div>
  );
}

function PaneTile({
  paneItem,
  label,
  prefix,
}: {
  paneItem: PaneItem;
  label: string;
  prefix: string;
}) {
  const { pane, notification } = paneItem;
  const hasLabel = label.length > 0;
  const matchesPrefix = prefix.length === 0 || (hasLabel && label.startsWith(prefix));
  const dimmed = !matchesPrefix;
  const subtitle =
    pane.teamName ??
    (pane.windowName && pane.windowName.length > 0 ? pane.windowName : pane.sessionName) ??
    pane.paneId;
  const tooltip = `${subtitle} (${pane.paneId})`;

  return (
    <div
      {...{ [PANE_ID_ATTR]: pane.paneId }}
      className={cn(
        "relative rounded border border-[var(--border-subtle)] bg-[var(--hover-bg)]",
        "flex flex-col items-center justify-center px-1 py-1 min-h-[44px] w-[76px]",
        "transition-opacity",
        dimmed && "opacity-20",
        notification && "ring-1 ring-blue-500/50",
      )}
      title={tooltip}
    >
      <LabelBadge label={label} prefix={prefix} dimmed={dimmed} />
      <div className="mt-0.5 flex items-center gap-0.5 max-w-full min-w-0">
        {pane.agentType && (
          <Circle size={5} className={cn("flex-shrink-0", statusDotClass(pane.agentStatus))} />
        )}
        <span className="text-[9px] text-[var(--text-muted)] truncate leading-tight">
          {subtitle}
        </span>
      </div>
    </div>
  );
}

function LabelBadge({ label, prefix, dimmed }: { label: string; prefix: string; dimmed: boolean }) {
  if (label.length === 0) {
    return (
      <span className="inline-flex items-center justify-center w-[26px] h-[22px] rounded text-[11px] font-mono border border-dashed border-[var(--border-subtle)] text-[var(--text-faint)]">
        ·
      </span>
    );
  }
  const isTwoChar = label.length === 2;
  const consumed = !dimmed && prefix.length > 0 ? prefix : "";
  const remaining = label.slice(consumed.length);

  return (
    <span
      className={cn(
        "inline-flex items-center justify-center h-[22px] rounded font-mono font-bold leading-none",
        "bg-yellow-400 text-black border border-yellow-500 shadow-[0_0_4px_rgba(250,204,21,0.4)]",
        isTwoChar ? "px-1.5 text-[13px] gap-px" : "w-[26px] text-[14px]",
      )}
    >
      {consumed && <span className="opacity-50">{consumed}</span>}
      <span>{remaining}</span>
    </span>
  );
}
