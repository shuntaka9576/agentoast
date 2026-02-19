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
    notifications,
    loading,
    deleteNotification,
    deleteByPanes,
    deleteAll,
    newIds,
  } = useNotifications();

  const {
    globalMuted,
    isRepoMuted,
    toggleGlobalMute,
    toggleRepoMute,
  } = useMute();

  const { groups: sessionGroups, fetchVersion } = useSessions();

  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [showHelp, setShowHelp] = useState(false);
  const [filterNotifiedOnly, setFilterNotifiedOnly] = useState(false);
  const [manuallyToggledGroups, setManuallyToggledGroups] = useState<Set<string>>(new Set());

  const needFetchVersionRef = useRef(-1);
  const fetchVersionRef = useRef(fetchVersion);
  fetchVersionRef.current = fetchVersion;

  // Load filter setting from config on mount
  useEffect(() => {
    invoke<boolean>("get_filter_notified_only").then((v) => setFilterNotifiedOnly(v)).catch(() => {});
  }, []);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const selectedIndexRef = useRef(selectedIndex);
  selectedIndexRef.current = selectedIndex;

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

  // Merge notifications and session groups into unified groups
  const unifiedGroups = useMemo(() => {
    // Build tmuxPane -> Notification map (latest notification per pane)
    const paneNotifMap = new Map<string, Notification>();

    for (const n of notifications) {
      if (n.tmuxPane) {
        const existing = paneNotifMap.get(n.tmuxPane);
        if (!existing || n.createdAt > existing.createdAt) {
          paneNotifMap.set(n.tmuxPane, n);
        }
      }
    }

    const map = new Map<string, UnifiedGroup>();

    // Process session groups: create pane items with matched notifications
    for (const sg of sessionGroups) {
      const groupKey = sg.currentPath;
      const repoName = sg.repoName;
      const gitBranch = sg.gitBranch;

      if (!map.has(groupKey)) {
        map.set(groupKey, {
          groupKey,
          repoName,
          gitBranch,
          paneItems: [],
        });
      }
      const ug = map.get(groupKey)!;

      for (const pane of sg.panes) {
        const notif = paneNotifMap.get(pane.paneId) ?? null;
        ug.paneItems.push({ pane, notification: notif });
      }
    }

    // Collect matched tmuxPanes
    const matchedPanes = new Set<string>();
    for (const ug of map.values()) {
      for (const pi of ug.paneItems) {
        if (pi.pane.paneId) matchedPanes.add(pi.pane.paneId);
      }
    }

    // Add orphaned notifications (not matched to any session pane) grouped by repo
    for (const n of notifications) {
      if (n.tmuxPane && matchedPanes.has(n.tmuxPane)) continue;

      const repoKey = n.repo || "__no_repo__";
      const repoLabel = n.repo || "Notifications";

      // Try to find existing group with same repoName
      let targetGroup: UnifiedGroup | undefined;
      for (const ug of map.values()) {
        if (ug.repoName === repoLabel) {
          targetGroup = ug;
          break;
        }
      }

      if (!targetGroup) {
        const groupKey = `orphan:${repoKey}`;
        if (!map.has(groupKey)) {
          map.set(groupKey, {
            groupKey,
            repoName: repoLabel,
            gitBranch: null,
            paneItems: [],
          });
        }
        targetGroup = map.get(groupKey)!;
      }

      targetGroup.paneItems.push({
        pane: {
          paneId: n.tmuxPane || `notif-${n.id}`,
          panePid: 0,
          sessionName: "",
          windowName: "",
          currentPath: "",
          isActive: false,
          agentType: null,
          agentStatus: null,
          agentModes: [],
          gitRepoRoot: null,
          gitBranch: null,
        },
        notification: n,
      });
    }

    const result = Array.from(map.values());

    // Sort panes within each group: notified panes first (latest notification on top)
    for (const ug of result) {
      ug.paneItems.sort((a, b) => {
        if (a.notification && b.notification) {
          return b.notification.createdAt.localeCompare(a.notification.createdAt);
        }
        if (a.notification && !b.notification) return -1;
        if (!a.notification && b.notification) return 1;
        return 0;
      });
    }

    // Sort: groups with notifications first (by latest createdAt desc), then no-notification groups alphabetically
    result.sort((a, b) => {
      const aLatestTime = getLatestTime(a);
      const bLatestTime = getLatestTime(b);
      if (aLatestTime && bLatestTime) {
        return bLatestTime.localeCompare(aLatestTime);
      }
      if (aLatestTime && !bLatestTime) return -1;
      if (!aLatestTime && bLatestTime) return 1;
      return a.repoName.localeCompare(b.repoName);
    });

    return result;
  }, [notifications, sessionGroups]);

  // Filter groups based on notification filter toggle
  const displayGroups = useMemo(() => {
    if (!filterNotifiedOnly) return unifiedGroups;
    return unifiedGroups
      .map((ug) => ({
        ...ug,
        paneItems: ug.paneItems.filter(
          (pi) => pi.notification !== null || pi.pane.agentStatus === "waiting",
        ),
      }))
      .filter((ug) => ug.paneItems.length > 0);
  }, [unifiedGroups, filterNotifiedOnly]);

  // Auto-collapse groups without notifications (respect manual toggles)
  // When not filtering, always expand the active pane's group
  const collapsedGroups = useMemo(() => {
    const collapsed = new Set<string>();

    // When not filtering, find the active pane's group to keep it expanded
    let activeGroupKey: string | null = null;
    if (!filterNotifiedOnly) {
      for (const ug of displayGroups) {
        if (ug.paneItems.some((pi) => pi.pane.isActive)) {
          activeGroupKey = ug.groupKey;
          break;
        }
      }
    }

    for (const ug of displayGroups) {
      if (manuallyToggledGroups.has(ug.groupKey)) continue;
      if (!groupHasNotifications(ug)) {
        // Don't auto-collapse the active pane's group
        if (ug.groupKey === activeGroupKey) continue;
        collapsed.add(ug.groupKey);
      }
    }
    // Apply manual toggles: toggled groups flip from their auto state
    for (const key of manuallyToggledGroups) {
      const ug = displayGroups.find((g) => g.groupKey === key);
      if (!ug) continue;
      const autoCollapsed = !groupHasNotifications(ug) && key !== activeGroupKey;
      if (autoCollapsed) {
        // Auto would collapse → manual toggle means expanded (don't add)
      } else {
        // Auto would expand → manual toggle means collapsed
        collapsed.add(key);
      }
    }
    return collapsed;
  }, [displayGroups, manuallyToggledGroups, filterNotifiedOnly]);

  // Build flat list of all items for keyboard navigation
  const flatItems = useMemo(() => {
    const result: FlatItem[] = [];
    for (const ug of displayGroups) {
      result.push({ type: "group-header", groupKey: ug.groupKey });
      if (!collapsedGroups.has(ug.groupKey)) {
        for (const pi of ug.paneItems) {
          result.push({ type: "pane-item", groupKey: ug.groupKey, paneItem: pi });
        }
      }
    }
    return result;
  }, [displayGroups, collapsedGroups]);

  // Reset selection when panel is shown
  useEffect(() => {
    const unlisten = listen("notifications:refresh", () => {
      setSelectedIndex(-1);
      needFetchVersionRef.current = fetchVersionRef.current;
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
        // Wait until sessions data is fresh after panel show
        if (needFetchVersionRef.current >= 0 && fetchVersion <= needFetchVersionRef.current) {
          return -1;
        }
        needFetchVersionRef.current = -1;

        // If notifications exist, focus the pane with the most recent notification
        let latestNotifIdx = -1;
        let latestCreatedAt = "";
        for (let i = 0; i < flatItems.length; i++) {
          const f = flatItems[i];
          if (f.type === "pane-item" && f.paneItem.notification) {
            if (f.paneItem.notification.createdAt > latestCreatedAt) {
              latestCreatedAt = f.paneItem.notification.createdAt;
              latestNotifIdx = i;
            }
          }
        }
        if (latestNotifIdx >= 0) return latestNotifIdx;

        if (!filterNotifiedOnly) {
          // No notifications: focus the active tmux pane
          const activeIdx = flatItems.findIndex(
            (f) => f.type === "pane-item" && f.paneItem.pane.isActive,
          );
          if (activeIdx >= 0) return activeIdx;
        }
        // No notifications and no active pane: focus first non-header item
        const idx = flatItems.findIndex((f) => f.type !== "group-header");
        return idx >= 0 ? idx : 0;
      }
      return Math.min(prev, flatItems.length - 1);
    });
  }, [flatItems, filterNotifiedOnly, fetchVersion]);

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
        void invoke("delete_notifications_by_pane", {
          tmuxPane: paneItem.notification.tmuxPane,
        });
      }
      void invoke("focus_terminal", {
        tmuxPane: paneItem.pane.paneId,
        terminalBundleId: paneItem.notification?.terminalBundleId ?? "",
      });
    },
    [],
  );

  // Keyboard navigation — ref callback pattern for stable listener
  const keyHandlerRef = useRef<(e: KeyboardEvent) => void>(() => {});
  keyHandlerRef.current = (e: KeyboardEvent) => {
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
        const item = flatItems[selectedIndexRef.current];
        if (!item) {
          void invoke("hide_panel");
          break;
        }
        if (item.type === "group-header") {
          toggleGroupExpanded(item.groupKey);
        } else if (item.type === "pane-item") {
          activatePaneItem(item.paneItem);
          void invoke("hide_panel");
        }
        break;
      }
      case "d": {
        if (showHelp || e.shiftKey) break;
        e.preventDefault();
        const item = flatItems[selectedIndexRef.current];
        if (!item) break;
        if (item.type === "group-header") {
          const ug = unifiedGroups.find((g) => g.groupKey === item.groupKey);
          if (ug) {
            const paneIds = ug.paneItems.map((pi) => pi.pane.paneId);
            void deleteByPanes(paneIds);
          }
        } else if (item.type === "pane-item" && item.paneItem.notification) {
          void deleteNotification(item.paneItem.notification.id);
        }
        break;
      }
      case "D": {
        if (showHelp) break;
        e.preventDefault();
        void deleteAll();
        break;
      }
      case "C": {
        if (showHelp) break;
        e.preventDefault();
        setManuallyToggledGroups(() => {
          const next = new Set<string>();
          for (const ug of unifiedGroups) {
            if (groupHasNotifications(ug)) {
              next.add(ug.groupKey);
            } else if (
              !filterNotifiedOnly &&
              ug.paneItems.some((pi) => pi.pane.isActive)
            ) {
              // Active group is auto-expanded, so toggle it to collapse
              next.add(ug.groupKey);
            }
          }
          return next;
        });
        setSelectedIndex(0);
        break;
      }
      case "E": {
        if (showHelp) break;
        e.preventDefault();
        setManuallyToggledGroups(() => {
          const next = new Set<string>();
          for (const ug of unifiedGroups) {
            if (!groupHasNotifications(ug)) {
              // Skip the active group — it's already auto-expanded
              if (
                !filterNotifiedOnly &&
                ug.paneItems.some((pi) => pi.pane.isActive)
              ) {
                continue;
              }
              next.add(ug.groupKey);
            }
          }
          return next;
        });
        break;
      }
      case "F": {
        if (showHelp) break;
        e.preventDefault();
        setFilterNotifiedOnly((prev) => {
          const next = !prev;
          void invoke("save_filter_notified_only", { value: next });
          return next;
        });
        setSelectedIndex(0);
        break;
      }
      case "Tab": {
        if (showHelp) break;
        e.preventDefault();
        const direction = e.shiftKey ? -1 : 1;
        const curIdx = selectedIndexRef.current;
        let nextIndex = curIdx < 0
          ? (direction === 1 ? 0 : flatItems.length - 1)
          : curIdx + direction;
        while (nextIndex >= 0 && nextIndex < flatItems.length) {
          const fi = flatItems[nextIndex];
          const hasNotif = fi.type === "pane-item" && fi.paneItem.notification !== null;
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
  useEffect(() => {
    const handler = (e: KeyboardEvent) => keyHandlerRef.current(e);
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Derive selected IDs for highlighting
  const currentItem = flatItems[selectedIndexRef.current];
  const selectedNotificationId =
    currentItem?.type === "pane-item" && currentItem.paneItem.notification ? currentItem.paneItem.notification.id :
    null;
  const selectedPaneId =
    currentItem?.type === "pane-item" ? currentItem.paneItem.pane.paneId : null;
  const selectedGroupHeaderKey =
    currentItem?.type === "group-header" ? currentItem.groupKey : null;

  const isEmpty = displayGroups.length === 0;

  return (
    <div className="h-screen flex flex-col items-center px-4 pb-4 pt-0.5 bg-transparent">
      <div className="tray-arrow" />
      <div className="w-full flex-1 min-h-0 flex flex-col bg-[var(--panel-bg)] backdrop-blur-xl rounded-xl border border-[var(--border-primary)] shadow-2xl overflow-hidden">
        <PanelHeader
          globalMuted={globalMuted}
          filterNotifiedOnly={filterNotifiedOnly}
          onToggleFilter={() => {
            setFilterNotifiedOnly((prev) => {
              const next = !prev;
              void invoke("save_filter_notified_only", { value: next });
              return next;
            });
          }}
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
              displayGroups.map((ug) => (
                <RepoGroup
                  key={ug.groupKey}
                  groupKey={ug.groupKey}
                  repoName={ug.repoName}
                  gitBranch={ug.gitBranch}
                  paneItems={ug.paneItems}
                  expanded={!collapsedGroups.has(ug.groupKey)}
                  isMuted={isRepoMuted(ug.groupKey)}
                  isHeaderSelected={selectedGroupHeaderKey === ug.groupKey}
                  headerNavIndex={flatItems.findIndex(
                    (f) => f.type === "group-header" && f.groupKey === ug.groupKey,
                  )}
                  newIds={newIds}
                  selectedId={selectedNotificationId}
                  selectedPaneId={selectedPaneId}
                  flatItems={flatItems}
                  onDeleteNotification={(id) => void deleteNotification(id)}
                  onDeleteByPanes={(paneIds) => void deleteByPanes(paneIds)}
                  onToggleRepoMute={(path) => void toggleRepoMute(path)}
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
  return ug.paneItems.some((pi) => pi.notification !== null);
}

function getLatestTime(ug: UnifiedGroup): string | null {
  let latest: string | null = null;
  for (const pi of ug.paneItems) {
    if (pi.notification && (!latest || pi.notification.createdAt > latest)) {
      latest = pi.notification.createdAt;
    }
  }
  return latest;
}
