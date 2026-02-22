import { X, Circle, GitBranch } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn, formatRelativeTime } from "@/lib/utils";
import type { PaneItem, AgentStatus, TmuxPane } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";

interface PaneCardProps {
  paneItem: PaneItem;
  isNew?: boolean;
  isSelected?: boolean;
  navIndex?: number;
  onDeleteNotification: (id: number) => void;
}

function statusDotClass(status: AgentStatus | null): string {
  switch (status) {
    case "running":
      return "text-green-500 fill-green-500";
    case "idle":
      return "text-[var(--text-muted)] fill-[var(--text-muted)]";
    case "waiting":
      return "text-amber-500 fill-amber-500";
    default:
      return "text-green-500 fill-green-500";
  }
}

function statusTooltip(pane: TmuxPane): string {
  const agent = pane.agentType ?? "agent";
  const modeStr = pane.agentModes.length > 0 ? ` (${pane.agentModes.join(", ")})` : "";
  switch (pane.agentStatus) {
    case "running":
      return `${agent}: running${modeStr}`;
    case "idle":
      return `${agent}: idle${modeStr}`;
    case "waiting": {
      const label = pane.waitingReason ? "waiting for response" : "waiting for input";
      return `${agent}: ${label}${modeStr}`;
    }
    default:
      return agent;
  }
}

function badgeGlowStyle(badgeColor: string): React.CSSProperties | undefined {
  switch (badgeColor) {
    case "green":
      return { boxShadow: "0 0 6px rgba(34, 197, 94, 0.3)" };
    case "blue":
      return { boxShadow: "0 0 6px rgba(59, 130, 246, 0.3)" };
    case "red":
      return { boxShadow: "0 0 6px rgba(239, 68, 68, 0.3)" };
    default:
      return undefined;
  }
}

const badgeColorClasses: Record<string, string> = {
  green: "bg-[var(--badge-stop-bg)] text-[var(--badge-stop-text)]",
  blue: "bg-[var(--badge-notif-bg)] text-[var(--badge-notif-text)]",
  red: "bg-red-500/20 text-red-400",
  gray: "bg-[var(--hover-bg-strong)] text-[var(--text-tertiary)]",
};

export function PaneCard({
  paneItem,
  isNew,
  isSelected,
  navIndex,
  onDeleteNotification,
}: PaneCardProps) {
  const { pane, notification } = paneItem;
  const metaEntries = notification
    ? Object.entries(notification.metadata).filter(([, v]) => v !== "")
    : [];

  const handleClick = () => {
    if (notification) {
      void invoke("delete_notifications_by_pane", {
        tmuxPane: notification.tmuxPane,
      });
    }
    void invoke("focus_terminal", {
      tmuxPane: pane.paneId,
      terminalBundleId: notification?.terminalBundleId ?? "",
    });
    void invoke("hide_panel");
  };

  const icon = pane.agentType ?? notification?.icon ?? "agentoast";
  const badgeClass = notification
    ? badgeColorClasses[notification.badgeColor] || badgeColorClasses.gray
    : null;

  return (
    <div
      data-nav-index={navIndex}
      className={cn(
        "group relative ml-6 mr-2 my-0.5 px-2.5 py-2 min-h-[52px] rounded-lg hover:bg-[var(--hover-bg)] cursor-pointer pane-separator",
        isNew && "animate-new-highlight",
        isSelected && "bg-[var(--hover-bg)]",
      )}
      onClick={handleClick}
    >
      <div className="flex items-start gap-2.5">
        <div className="flex-shrink-0 mt-0.5">
          <IconPreset
            icon={icon}
            size={14}
            className="text-[var(--text-tertiary)]"
          />
        </div>

        <div className="flex-1 min-w-0">
          {/* Line 1: Running status + notification badge + session info + time */}
          <div className="flex items-center gap-1.5">
            {pane.agentType && (
              <span className="flex-shrink-0 flex items-center gap-1" title={statusTooltip(pane)}>
                <Circle size={7} className={statusDotClass(pane.agentStatus)} />
                {pane.waitingReason && (
                  <span className="text-[10px] text-amber-500 font-medium">
                    {pane.waitingReason}
                  </span>
                )}
              </span>
            )}
            {pane.agentModes.map((mode) => (
              <span key={mode} className="text-[10px] text-[var(--text-muted)] font-medium flex-shrink-0">
                {mode}
              </span>
            ))}
            {notification?.badge && badgeClass && (
              <span
                className={cn(
                  "px-1.5 py-0.5 text-[10px] font-medium rounded flex-shrink-0",
                  badgeClass,
                )}
                style={badgeGlowStyle(notification.badgeColor)}
              >
                {notification.badge}
              </span>
            )}
            {notification && (
              <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0 ml-auto">
                {formatRelativeTime(notification.createdAt)}
              </span>
            )}
          </div>

          {/* Line 2: body (1 line) */}
          {notification?.body && (
            <p className="mt-0.5 text-[11px] text-[var(--text-secondary)] line-clamp-1">
              {notification.body}
            </p>
          )}

          {/* Line 3: metadata + tmux pane */}
          {(metaEntries.length > 0 || pane.paneId) && (
            <div className="flex items-center gap-1 mt-0.5 text-[10px] text-[var(--text-muted)] truncate">
              {metaEntries.map(([key, value], i) => (
                <span key={key} className={cn("flex items-center gap-0.5", i > 0 ? "ml-1" : "")}>
                  {key === "branch" ? (
                    <GitBranch size={10} className="flex-shrink-0" />
                  ) : (
                    <span>{key}:</span>
                  )}
                  {" "}{value}
                </span>
              ))}
              {pane.paneId && (
                <span className={cn("flex items-center gap-0.5", metaEntries.length > 0 ? "ml-1" : "")}>
                  <TmuxIcon size={10} className="flex-shrink-0" /> {pane.paneId}
                </span>
              )}
            </div>
          )}
        </div>

        {/* Delete button: only when notification exists */}
        {notification && (
          <button
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              onDeleteNotification(notification.id);
            }}
            className="flex-shrink-0 p-1 rounded opacity-0 group-hover:opacity-100 text-[var(--delete-text)] hover:text-[var(--delete-text-hover)] hover:bg-[var(--hover-bg-strong)] transition-all"
          >
            <X size={12} />
          </button>
        )}
      </div>
    </div>
  );
}
