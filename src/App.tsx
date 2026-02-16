import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useNotifications } from "@/hooks/use-notifications";
import { useMute } from "@/hooks/use-mute";
import { PanelHeader } from "@/components/panel-header";
import { RepoGroup } from "@/components/repo-group";
import { KeybindHelp } from "@/components/keybind-help";
import { Bell } from "lucide-react";
import type { Notification } from "@/lib/types";

export function App() {
  const {
    groups,
    loading,
    deleteNotification,
    deleteGroup,
    deleteAll,
    newIds,
  } = useNotifications();

  const {
    globalMuted,
    isGroupMuted,
    toggleGlobalMute,
    toggleGroupMute,
  } = useMute();

  const [selectedIndex, setSelectedIndex] = useState(0);
  const [showHelp, setShowHelp] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Flatten all visible notifications into a single list for keyboard navigation
  const flatNotifications = useMemo(() => {
    const result: Notification[] = [];
    for (const group of groups) {
      for (const n of group.notifications) {
        result.push(n);
      }
    }
    return result;
  }, [groups]);

  // Reset selection when panel is shown (notifications:refresh is emitted on panel show)
  useEffect(() => {
    const unlisten = listen("notifications:refresh", () => {
      setSelectedIndex(0);
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // Clamp selectedIndex when notifications change
  useEffect(() => {
    setSelectedIndex((prev) =>
      flatNotifications.length === 0 ? 0 : Math.min(prev, flatNotifications.length - 1)
    );
  }, [flatNotifications]);

  // Scroll selected card into view
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    const el = container.querySelector(`[data-nav-index="${selectedIndex}"]`);
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  const activateNotification = useCallback(
    (notification: Notification) => {
      if (notification.tmuxPane) {
        void invoke("delete_notifications_by_group_tmux", {
          groupName: notification.groupName,
          tmuxPane: notification.tmuxPane,
        });
      } else {
        void deleteNotification(notification.id);
      }
      void invoke("focus_terminal", {
        tmuxPane: notification.tmuxPane,
        terminalBundleId: notification.terminalBundleId,
      });
    },
    [deleteNotification],
  );

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case "?":
          e.preventDefault();
          setShowHelp((prev) => !prev);
          break;
        case "Escape":
          e.preventDefault();
          if (showHelp) {
            setShowHelp(false);
          } else {
            void invoke("hide_panel");
          }
          break;
        case "j":
          if (showHelp) break;
          e.preventDefault();
          if (flatNotifications.length > 0) {
            setSelectedIndex((prev) => Math.min(prev + 1, flatNotifications.length - 1));
          }
          break;
        case "k":
          if (showHelp) break;
          e.preventDefault();
          if (flatNotifications.length > 0) {
            setSelectedIndex((prev) => Math.max(prev - 1, 0));
          }
          break;
        case "Enter": {
          if (showHelp) break;
          e.preventDefault();
          const n = flatNotifications[selectedIndex];
          if (n) {
            activateNotification(n);
          } else {
            void invoke("hide_panel");
          }
          break;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [flatNotifications, selectedIndex, activateNotification, showHelp]);

  // Build a set of selected notification IDs for highlighting
  const selectedId = flatNotifications[selectedIndex]?.id ?? null;

  return (
    <div className="h-screen flex flex-col items-center px-4 pb-4 pt-0.5 bg-transparent">
      <div className="tray-arrow" />
      <div className="w-full flex-1 min-h-0 flex flex-col bg-[var(--panel-bg)] backdrop-blur-xl rounded-xl border border-[var(--border-primary)] shadow-2xl overflow-hidden">
        <PanelHeader
          globalMuted={globalMuted}
          onDeleteAll={() => void deleteAll()}
          onToggleGlobalMute={() => void toggleGlobalMute()}
        />

        <div className="relative flex-1 min-h-0">
          <div className="h-full overflow-y-auto" ref={scrollContainerRef}>
            {loading ? (
              <div className="flex items-center justify-center h-full">
                <div className="text-xs text-[var(--text-muted)]">Loading...</div>
              </div>
            ) : groups.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full gap-3">
                <Bell size={32} className="text-[var(--text-faint)]" />
                <p className="text-xs text-[var(--text-muted)]">No notifications yet</p>
              </div>
            ) : (
              groups.map((group) => (
                <RepoGroup
                  key={group.groupName}
                  group={group}
                  isMuted={isGroupMuted(group.groupName)}
                  newIds={newIds}
                  selectedId={selectedId}
                  flatNotifications={flatNotifications}
                  onDelete={(id) => void deleteNotification(id)}
                  onDeleteGroup={(name) => void deleteGroup(name)}
                  onToggleGroupMute={(name) => void toggleGroupMute(name)}
                />
              ))
            )}
          </div>
          {showHelp && <KeybindHelp onClose={() => setShowHelp(false)} />}
          {!showHelp && (
            <div
              className="absolute bottom-2 right-2 w-4 h-4 rounded-full border border-[var(--text-tertiary)] flex items-center justify-center text-[var(--text-tertiary)]"
              style={{ fontSize: "10px", lineHeight: 1 }}
            >
              ?
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
