import { X, Circle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn, formatRelativeTime } from "@/lib/utils";
import type { PaneItem } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";

interface PaneCardProps {
  paneItem: PaneItem;
  isNew?: boolean;
  isSelected?: boolean;
  navIndex?: number;
  onDeleteNotification: (id: number) => void;
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
      if (notification.tmuxPane) {
        void invoke("delete_notifications_by_group_tmux", {
          groupName: notification.groupName,
          tmuxPane: notification.tmuxPane,
        });
      } else {
        onDeleteNotification(notification.id);
      }
    }
    void invoke("focus_terminal", {
      tmuxPane: pane.paneId,
      terminalBundleId: notification?.terminalBundleId ?? "",
    });
    void invoke("hide_panel");
  };

  const icon = pane.agentType ?? notification?.icon ?? "agentoast";
  const badgeClass = notification
    ? badgeColorClasses[notification.color] || badgeColorClasses.gray
    : null;

  return (
    <div
      data-nav-index={navIndex}
      className={cn(
        "group relative px-3 py-2 hover:bg-[var(--hover-bg)] transition-colors cursor-pointer",
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
              <span className="flex items-center gap-1 flex-shrink-0">
                <Circle size={5} className="text-green-500 fill-green-500" />
                <span className="text-[10px] text-green-500 font-medium">
                  {pane.agentType}
                </span>
              </span>
            )}
            {notification?.title && badgeClass && (
              <span
                className={cn(
                  "px-1.5 py-0.5 text-[10px] font-medium rounded flex-shrink-0",
                  badgeClass,
                )}
              >
                {notification.title}
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
                <span key={key} className={i > 0 ? "ml-1" : ""}>
                  <span>{key}:</span> {value}
                </span>
              ))}
              {metaEntries.length > 0 && <span className="ml-1" />}
              <TmuxIcon size={10} className="flex-shrink-0" />
              {pane.paneId}
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
