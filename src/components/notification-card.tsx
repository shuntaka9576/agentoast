import { X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { cn, formatRelativeTime } from "@/lib/utils";
import type { Notification } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";

interface NotificationCardProps {
  notification: Notification;
  isNew?: boolean;
  isSelected?: boolean;
  navIndex?: number;
  onDelete: (id: number) => void;
}

const badgeColorClasses: Record<string, string> = {
  green: "bg-[var(--badge-stop-bg)] text-[var(--badge-stop-text)]",
  blue: "bg-[var(--badge-notif-bg)] text-[var(--badge-notif-text)]",
  red: "bg-red-500/20 text-red-400",
  gray: "bg-[var(--hover-bg-strong)] text-[var(--text-tertiary)]",
};

export function NotificationCard({
  notification,
  isNew,
  isSelected,
  navIndex,
  onDelete,
}: NotificationCardProps) {
  const metaEntries = Object.entries(notification.metadata).filter(
    ([, v]) => v !== "",
  );
  const badgeClass = badgeColorClasses[notification.color] || badgeColorClasses.gray;
  return (
    <div
      data-nav-index={navIndex}
      className={cn(
        "group relative px-3 py-2.5 hover:bg-[var(--hover-bg)] transition-colors cursor-pointer",
        isNew && "animate-new-highlight",
        isSelected && "bg-[var(--hover-bg)]",
      )}
      onClick={() => {
        if (notification.tmuxPane) {
          void invoke("delete_notifications_by_group_tmux", {
            groupName: notification.groupName,
            tmuxPane: notification.tmuxPane,
          });
        } else {
          onDelete(notification.id);
        }
        void invoke("focus_terminal", {
          tmuxPane: notification.tmuxPane,
          terminalBundleId: notification.terminalBundleId,
        });
        void invoke("hide_panel");
      }}
    >
      <div className="flex items-start gap-2.5">
        {/* Icon */}
        <div className="flex-shrink-0 mt-1">
          <IconPreset
            icon={notification.icon}
            size={16}
            className="text-[var(--text-tertiary)]"
          />
        </div>

        <div className="flex-1 min-w-0">
          {/* Title badge + time */}
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-1.5">
              {notification.title && (
                <span
                  className={cn(
                    "px-1.5 py-0.5 text-[10px] font-medium rounded",
                    badgeClass,
                  )}
                >
                  {notification.title}
                </span>
              )}
            </div>
            <span className="text-[10px] text-[var(--text-muted)] flex-shrink-0">
              {formatRelativeTime(notification.createdAt)}
            </span>
          </div>

          {/* Metadata + tmux */}
          {(metaEntries.length > 0 || notification.tmuxPane) && (
            <div className="flex items-center gap-1 mt-1 text-[10px] text-[var(--text-muted)] truncate">
              {metaEntries.map(([key, value], i) => (
                <span key={key} className={i > 0 ? "ml-1" : ""}>
                  <span>{key}:</span>{" "}
                  {value}
                </span>
              ))}
              {metaEntries.length > 0 && notification.tmuxPane && (
                <span className="ml-1" />
              )}
              {notification.tmuxPane && (
                <>
                  <TmuxIcon size={10} className="flex-shrink-0" />
                  {notification.tmuxPane}
                </>
              )}
            </div>
          )}

          {/* Body */}
          {notification.body && (
            <p className="mt-1 text-[12px] text-[var(--text-secondary)] line-clamp-2">
              {notification.body}
            </p>
          )}
        </div>

        {/* Delete button */}
        <button
          tabIndex={-1}
          onClick={(e) => {
            e.stopPropagation();
            onDelete(notification.id);
          }}
          className="flex-shrink-0 p-1 rounded opacity-0 group-hover:opacity-100 text-[var(--delete-text)] hover:text-[var(--delete-text-hover)] hover:bg-[var(--hover-bg-strong)] transition-all"
        >
          <X size={12} />
        </button>
      </div>
    </div>
  );
}
