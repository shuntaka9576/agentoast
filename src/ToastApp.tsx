import { useEffect, useState, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import type { Notification } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";

const TOAST_DURATION = 4000;
const FADE_DURATION = 300;

const badgeColorClasses: Record<string, string> = {
  green: "bg-[var(--badge-stop-bg)] text-[var(--badge-stop-text)]",
  blue: "bg-[var(--badge-notif-bg)] text-[var(--badge-notif-text)]",
  red: "bg-red-500/20 text-red-400",
  gray: "bg-[var(--hover-bg-strong)] text-[var(--text-tertiary)]",
};

export function ToastApp() {
  const [notification, setNotification] = useState<Notification | null>(null);
  const [isVisible, setIsVisible] = useState(false);
  const [isFadingOut, setIsFadingOut] = useState(false);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const hideToast = useCallback(() => {
    setIsFadingOut(true);
    fadeTimerRef.current = setTimeout(() => {
      setIsVisible(false);
      setIsFadingOut(false);
      setNotification(null);
      void invoke("hide_toast");
    }, FADE_DURATION);
  }, []);

  useEffect(() => {
    const unlisten = listen<Notification[]>("toast:show", (event) => {
      const notifications = event.payload;
      if (notifications.length === 0) return;

      // Show the latest notification
      const latest = notifications[notifications.length - 1];
      setNotification(latest);
      setIsVisible(true);
      setIsFadingOut(false);

      // Clear existing timers
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current);
      }
      if (fadeTimerRef.current) {
        clearTimeout(fadeTimerRef.current);
        fadeTimerRef.current = null;
      }

      // Auto-hide after TOAST_DURATION
      hideTimerRef.current = setTimeout(() => {
        hideToast();
      }, TOAST_DURATION);
    });

    return () => {
      void unlisten.then((fn) => fn());
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current);
      }
      if (fadeTimerRef.current) {
        clearTimeout(fadeTimerRef.current);
      }
    };
  }, [hideToast]);

  const handleClick = () => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
    }
    if (notification) {
      if (!notification.forceFocus) {
        if (notification.tmuxPane) {
          void invoke("delete_notifications_by_group_tmux", {
            groupName: notification.groupName,
            tmuxPane: notification.tmuxPane,
          });
        } else {
          void invoke("delete_notification", { id: notification.id });
        }
      }
      void invoke("focus_terminal", {
        tmuxPane: notification.tmuxPane,
      });
    }
    hideToast();
  };

  if (!isVisible || !notification) {
    return <div className="h-screen bg-transparent" />;
  }

  const badgeClass = badgeColorClasses[notification.color] || badgeColorClasses.gray;
  const metaEntries = Object.entries(notification.metadata).filter(
    ([, v]) => v !== "",
  );
  return (
    <div
      className={cn(
        "h-screen p-2 bg-transparent cursor-pointer",
        isFadingOut ? "animate-toast-out" : "animate-toast-in",
      )}
      onClick={handleClick}
    >
      <div className={cn(
        "relative backdrop-blur-xl rounded-xl border shadow-2xl p-3 h-full",
        notification.forceFocus
          ? "bg-[var(--toast-focus-bg)] border-[var(--border-focus)]"
          : "bg-[var(--toast-bg)] border-[var(--border-primary)]"
      )}>
        <div className="flex items-start gap-2.5">
          <div className="flex-shrink-0 mt-0.5">
            <IconPreset
              icon={notification.icon}
              size={20}
              className="text-[var(--text-secondary)]"
            />
          </div>
          <div className="flex-1 min-w-0">
            {/* Title badge + group name */}
            <div className="flex items-center gap-2">
              {notification.title && (
                <span
                  className={cn(
                    "px-1.5 py-0.5 text-[10px] font-medium rounded flex-shrink-0",
                    badgeClass,
                  )}
                >
                  {notification.title}
                </span>
              )}
              <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
                {notification.groupName}
              </span>
            </div>

            {/* Metadata */}
            {metaEntries.length > 0 && (
              <div className="flex items-center gap-1 mt-1 text-[11px] text-[var(--text-tertiary)] truncate">
                {metaEntries.map(([key, value], i) => (
                  <span key={key}>
                    {i > 0 && <span className="mx-0.5">·</span>}
                    <span className="text-[var(--text-muted)]">{key}:</span>{" "}
                    {value}
                  </span>
                ))}
              </div>
            )}

            {/* tmux info */}
            {notification.tmuxPane && (
              <div className="flex items-center gap-1 mt-1 text-[11px] text-[var(--text-tertiary)] truncate">
                <TmuxIcon size={11} className="flex-shrink-0" />
                {notification.tmuxPane}
              </div>
            )}

            {/* Body */}
            {notification.body && (
              <p className="mt-1 text-[11px] text-[var(--text-secondary)] line-clamp-2">
                {notification.body}
              </p>
            )}

          </div>
        </div>
        {/* Focused: no history badge (absolute bottom-right) */}
        {notification.forceFocus && (
          <span className="absolute bottom-2 right-3 px-1.5 py-0.5 text-[10px] font-medium rounded bg-[var(--badge-focus-bg)] text-[var(--badge-focus-text)]">
            Focused: no history
          </span>
        )}
      </div>
    </div>
  );
}
