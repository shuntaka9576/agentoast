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
import type { Notification, UnifiedGroup, FlatItem, PaneItem } from "@/lib/types";

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

  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [showHelp, setShowHelp] = useState(false);
  const [manuallyToggledGroups, setManuallyToggledGroups] = useState<Set<string>>(new Set());
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const toggleGroupExpanded = useCallback((groupKey: string) => {
    setManuallyToggledGroups((prev) => {
      const next = new Set(prev);
      if (next.has(groupKey)) {
        next.delete(groupKey);
      } else {
        next.add(groupKey);
      }
      return next;
    });
  }, []);

  // Merge notification groups and session groups into unified groups
  const unifiedGroups = useMemo(() => {
    // Build tmuxPane -> Notification map (latest notification per pane)
    const paneNotifMap = new Map<string, Notification>();
    const matchedNotifIds = new Set<number>();

    for (const g of groups) {
      for (const n of g.notifications) {
        if (n.tmuxPane) {
          const existing = paneNotifMap.get(n.tmuxPane);
          if (!existing || n.createdAt > existing.createdAt) {
            paneNotifMap.set(n.tmuxPane, n);
          }
        }
      }
    }

    const map = new Map<string, UnifiedGroup>();

    // Process session groups: create pane items with matched notifications
    for (const sg of sessionGroups) {
      // Use git repo root from panes as group key
      const groupKey = sg.currentPath;
      const repoName = sg.repoName;
      const gitBranch = sg.gitBranch;

      if (!map.has(groupKey)) {
        map.set(groupKey, {
          groupKey,
          repoName,
          gitBranch,
          paneItems: [],
          orphanNotifications: [],
        });
      }
      const ug = map.get(groupKey)!;

      for (const pane of sg.panes) {
        const notif = paneNotifMap.get(pane.paneId) ?? null;
        if (notif) {
          matchedNotifIds.add(notif.id);
        }
        ug.paneItems.push({ pane, notification: notif });
      }
    }

    // Collect orphan notifications (not matched to any pane)
    for (const g of groups) {
      for (const n of g.notifications) {
        if (matchedNotifIds.has(n.id)) continue;

        const groupKey = g.groupName;
        if (!map.has(groupKey)) {
          map.set(groupKey, {
            groupKey,
            repoName: groupKey,
            gitBranch: null,
            paneItems: [],
            orphanNotifications: [],
          });
        }
        map.get(groupKey)!.orphanNotifications.push(n);
      }
    }

    // Sort: groups with notifications first (by latest createdAt desc), then no-notification groups alphabetically
    const result = Array.from(map.values());
    result.sort((a, b) => {
      const aLatestTime = getLatestTime(a);
      const bLatestTime = getLatestTime(b);
      const aHasNotif = aLatestTime !== null;
      const bHasNotif = bLatestTime !== null;

      if (aHasNotif && bHasNotif) {
        return bLatestTime!.localeCompare(aLatestTime!);
      }
      if (aHasNotif && !bHasNotif) return -1;
      if (!aHasNotif && bHasNotif) return 1;
      return a.repoName.localeCompare(b.repoName);
    });

    return result;
  }, [groups, sessionGroups]);

  // Auto-collapse groups without notifications (respect manual toggles)
  const collapsedGroups = useMemo(() => {
    const collapsed = new Set<string>();
    for (const ug of unifiedGroups) {
      if (manuallyToggledGroups.has(ug.groupKey)) continue;
      if (!groupHasNotifications(ug)) {
        collapsed.add(ug.groupKey);
      }
    }
    // Apply manual toggles: toggled groups flip from their auto state
    for (const key of manuallyToggledGroups) {
      const ug = unifiedGroups.find((g) => g.groupKey === key);
      if (!ug) continue;
      const autoCollapsed = !groupHasNotifications(ug);
      // Manual toggle flips the auto state
      if (autoCollapsed) {
        // Auto would collapse → manual toggle means expanded (don't add)
      } else {
        // Auto would expand → manual toggle means collapsed
        collapsed.add(key);
      }
    }
    return collapsed;
  }, [unifiedGroups, manuallyToggledGroups]);

  // Build flat list of all items for keyboard navigation
  const flatItems = useMemo(() => {
    const result: FlatItem[] = [];
    for (const ug of unifiedGroups) {
      result.push({ type: "group-header", groupKey: ug.groupKey });
      if (!collapsedGroups.has(ug.groupKey)) {
        for (const pi of ug.paneItems) {
          result.push({ type: "pane-item", groupKey: ug.groupKey, paneItem: pi });
        }
        for (const n of ug.orphanNotifications) {
          result.push({ type: "orphan-notification", groupKey: ug.groupKey, notification: n });
        }
      }
    }
    return result;
  }, [unifiedGroups, collapsedGroups]);
  // Reset selection when panel is shown
  useEffect(() => {
    const unlisten = listen("notifications:refresh", () => {
      setSelectedIndex(-1);
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // Clamp selectedIndex when items change
  useEffect(() => {
    setSelectedIndex((prev) => {
      if (flatItems.length === 0) return -1;
      if (prev < 0) {
        const idx = flatItems.findIndex((f) => f.type !== "group-header");
        return idx >= 0 ? idx : 0;
      }
      return Math.min(prev, flatItems.length - 1);
    });
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

  const activatePaneItem = useCallback(
    (paneItem: PaneItem) => {
      if (paneItem.notification) {
        if (paneItem.notification.tmuxPane) {
          void invoke("delete_notifications_by_group_tmux", {
            groupName: paneItem.notification.groupName,
            tmuxPane: paneItem.notification.tmuxPane,
          });
        } else {
          void deleteNotification(paneItem.notification.id);
        }
      }
      void invoke("focus_terminal", {
        tmuxPane: paneItem.pane.paneId,
        terminalBundleId: paneItem.notification?.terminalBundleId ?? "",
      });
    },
    [deleteNotification],
  );

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
          if (item.type === "group-header") {
            toggleGroupExpanded(item.groupKey);
          } else if (item.type === "pane-item") {
            activatePaneItem(item.paneItem);
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
          if (!item || item.type === "group-header") break;
          if (item.type === "pane-item" && item.paneItem.notification) {
            void deleteNotification(item.paneItem.notification.id);
          } else if (item.type === "orphan-notification") {
            void deleteNotification(item.notification.id);
          }
          break;
        }
        case "D": {
          if (showHelp) break;
          e.preventDefault();
          const item = flatItems[selectedIndex];
          if (item) {
            void deleteGroup(item.groupKey);
          }
          break;
        }
        case "Tab": {
          if (showHelp) break;
          e.preventDefault();
          const direction = e.shiftKey ? -1 : 1;
          let nextIndex = selectedIndex < 0
            ? (direction === 1 ? 0 : flatItems.length - 1)
            : selectedIndex + direction;
          while (nextIndex >= 0 && nextIndex < flatItems.length) {
            const fi = flatItems[nextIndex];
            const hasNotif =
              (fi.type === "pane-item" && fi.paneItem.notification !== null) ||
              fi.type === "orphan-notification";
            if (hasNotif) {
              setSelectedIndex(nextIndex);
              break;
            }
            nextIndex += direction;
          }
          break;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [flatItems, selectedIndex, activatePaneItem, activateNotification, showHelp, deleteNotification, deleteGroup, toggleGroupExpanded]);

  // Derive selected IDs for highlighting
  const currentItem = flatItems[selectedIndex];
  const selectedNotificationId =
    currentItem?.type === "orphan-notification" ? currentItem.notification.id :
    currentItem?.type === "pane-item" && currentItem.paneItem.notification ? currentItem.paneItem.notification.id :
    null;
  const selectedPaneId =
    currentItem?.type === "pane-item" ? currentItem.paneItem.pane.paneId : null;
  const selectedGroupHeaderKey =
    currentItem?.type === "group-header" ? currentItem.groupKey : null;

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
                  key={ug.groupKey}
                  groupKey={ug.groupKey}
                  repoName={ug.repoName}
                  gitBranch={ug.gitBranch}
                  paneItems={ug.paneItems}
                  orphanNotifications={ug.orphanNotifications}
                  expanded={!collapsedGroups.has(ug.groupKey)}
                  isMuted={isGroupMuted(ug.groupKey)}
                  isHeaderSelected={selectedGroupHeaderKey === ug.groupKey}
                  headerNavIndex={flatItems.findIndex(
                    (f) => f.type === "group-header" && f.groupKey === ug.groupKey,
                  )}
                  newIds={newIds}
                  selectedId={selectedNotificationId}
                  selectedPaneId={selectedPaneId}
                  flatItems={flatItems}
                  onDeleteNotification={(id) => void deleteNotification(id)}
                  onDeleteGroup={(key) => void deleteGroup(key)}
                  onToggleGroupMute={(key) => void toggleGroupMute(key)}
                  onToggleExpand={() => toggleGroupExpanded(ug.groupKey)}
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

function groupHasNotifications(ug: UnifiedGroup): boolean {
  return ug.paneItems.some((pi) => pi.notification !== null) || ug.orphanNotifications.length > 0;
}

function getLatestTime(ug: UnifiedGroup): string | null {
  let latest: string | null = null;
  for (const pi of ug.paneItems) {
    if (pi.notification && (!latest || pi.notification.createdAt > latest)) {
      latest = pi.notification.createdAt;
    }
  }
  for (const n of ug.orphanNotifications) {
    if (!latest || n.createdAt > latest) {
      latest = n.createdAt;
    }
  }
  return latest;
}
