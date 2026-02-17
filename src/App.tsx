import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useNotifications } from "@/hooks/use-notifications";
import { useMute } from "@/hooks/use-mute";
import { useSessions } from "@/hooks/use-sessions";
import { PanelHeader } from "@/components/panel-header";
import { RepoGroup } from "@/components/repo-group";
import { KeybindHelp } from "@/components/keybind-help";
import { Bell } from "lucide-react";
import type { Notification, UnifiedGroup, FlatItem } from "@/lib/types";

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

  const { groups: sessionGroups } = useSessions();

  const [selectedIndex, setSelectedIndex] = useState(0);
  const [showHelp, setShowHelp] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Merge notification groups and session groups into unified groups
  const unifiedGroups = useMemo(() => {
    const map = new Map<string, UnifiedGroup>();

    // 1. Expand notification groups
    for (const g of groups) {
      map.set(g.groupName, {
        groupName: g.groupName,
        activeSessions: [],
        notifications: g.notifications,
      });
    }

    // 2. Merge session panes
    for (const sg of sessionGroups) {
      for (const pane of sg.panes) {
        // Try to match by tmux_pane in existing notification groups
        let matched = false;
        for (const [, ug] of map) {
          if (ug.notifications.some((n) => n.tmuxPane === pane.paneId)) {
            ug.activeSessions.push(pane);
            matched = true;
            break;
          }
        }
        if (matched) continue;

        // Try to match by repoName == groupName
        if (map.has(sg.repoName)) {
          map.get(sg.repoName)!.activeSessions.push(pane);
          continue;
        }

        // Create new group for session-only pane
        if (!map.has(sg.repoName)) {
          map.set(sg.repoName, {
            groupName: sg.repoName,
            activeSessions: [],
            notifications: [],
          });
        }
        map.get(sg.repoName)!.activeSessions.push(pane);
      }
    }

    // 3. Sort: groups with notifications first (by latest createdAt desc), then session-only groups alphabetically
    const result = Array.from(map.values());
    result.sort((a, b) => {
      const aHasNotif = a.notifications.length > 0;
      const bHasNotif = b.notifications.length > 0;
      if (aHasNotif && bHasNotif) {
        const aLatest = a.notifications[0]?.createdAt ?? "";
        const bLatest = b.notifications[0]?.createdAt ?? "";
        return bLatest.localeCompare(aLatest);
      }
      if (aHasNotif && !bHasNotif) return -1;
      if (!aHasNotif && bHasNotif) return 1;
      return a.groupName.localeCompare(b.groupName);
    });

    return result;
  }, [groups, sessionGroups]);

  // Build flat list of all items for keyboard navigation
  const flatItems = useMemo(() => {
    const result: FlatItem[] = [];
    for (const ug of unifiedGroups) {
      for (const pane of ug.activeSessions) {
        result.push({ type: "session", groupName: ug.groupName, pane });
      }
      for (const n of ug.notifications) {
        result.push({ type: "notification", groupName: ug.groupName, notification: n });
      }
    }
    return result;
  }, [unifiedGroups]);

  // Reset selection when panel is shown
  useEffect(() => {
    const unlisten = listen("notifications:refresh", () => {
      setSelectedIndex(0);
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // Clamp selectedIndex when items change
  useEffect(() => {
    setSelectedIndex((prev) =>
      flatItems.length === 0 ? 0 : Math.min(prev, flatItems.length - 1)
    );
  }, [flatItems]);

  // Scroll selected item into view
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
          if (flatItems.length > 0) {
            setSelectedIndex((prev) => Math.min(prev + 1, flatItems.length - 1));
          }
          break;
        case "k":
          if (showHelp) break;
          e.preventDefault();
          if (flatItems.length > 0) {
            setSelectedIndex((prev) => Math.max(prev - 1, 0));
          }
          break;
        case "Enter": {
          if (showHelp) break;
          e.preventDefault();
          const item = flatItems[selectedIndex];
          if (!item) {
            void invoke("hide_panel");
            break;
          }
          if (item.type === "session") {
            void invoke("focus_terminal", {
              tmuxPane: item.pane.paneId,
              terminalBundleId: "",
            });
            void invoke("hide_panel");
          } else {
            activateNotification(item.notification);
            void invoke("hide_panel");
          }
          break;
        }
        case "d": {
          if (showHelp || e.shiftKey) break;
          e.preventDefault();
          const item = flatItems[selectedIndex];
          if (item?.type === "notification") {
            void deleteNotification(item.notification.id);
          }
          break;
        }
        case "D": {
          if (showHelp) break;
          e.preventDefault();
          const item = flatItems[selectedIndex];
          if (item) {
            void deleteGroup(item.groupName);
          }
          break;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [flatItems, selectedIndex, activateNotification, showHelp, deleteNotification, deleteGroup]);

  // Derive selected IDs for highlighting
  const currentItem = flatItems[selectedIndex];
  const selectedNotificationId = currentItem?.type === "notification" ? currentItem.notification.id : null;
  const selectedPaneId = currentItem?.type === "session" ? currentItem.pane.paneId : null;

  const isEmpty = unifiedGroups.length === 0;

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
            ) : isEmpty ? (
              <div className="flex flex-col items-center justify-center h-full gap-3">
                <Bell size={32} className="text-[var(--text-faint)]" />
                <p className="text-xs text-[var(--text-muted)]">No notifications yet</p>
              </div>
            ) : (
              unifiedGroups.map((ug) => (
                <RepoGroup
                  key={ug.groupName}
                  groupName={ug.groupName}
                  activeSessions={ug.activeSessions}
                  notifications={ug.notifications}
                  isMuted={isGroupMuted(ug.groupName)}
                  newIds={newIds}
                  selectedId={selectedNotificationId}
                  selectedPaneId={selectedPaneId}
                  flatItems={flatItems}
                  onDelete={(id) => void deleteNotification(id)}
                  onDeleteGroup={(name) => void deleteGroup(name)}
                  onToggleGroupMute={(name) => void toggleGroupMute(name)}
                />
              ))
            )}
          </div>
          {showHelp && <KeybindHelp onClose={() => setShowHelp(false)} />}
          {!showHelp && (
            <button
              type="button"
              tabIndex={-1}
              onClick={() => setShowHelp(true)}
              className="absolute bottom-2 right-2 w-4 h-4 rounded-full border border-[var(--text-tertiary)] flex items-center justify-center text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:border-[var(--text-secondary)] cursor-pointer bg-transparent p-0"
              style={{ fontSize: "10px", lineHeight: 1 }}
            >
              ?
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
