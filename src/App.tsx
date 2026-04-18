import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useNotifications } from "@/hooks/use-notifications";
import { useMute } from "@/hooks/use-mute";
import { useSessions } from "@/hooks/use-sessions";
import { useAppUpdate } from "@/hooks/use-app-update";
import { PanelHeader } from "@/components/panel-header";
import { RepoGroup } from "@/components/repo-group";
import { KeybindHelp } from "@/components/keybind-help";
import { Bell } from "lucide-react";
import type { Notification, UnifiedGroup, FlatItem, PaneItem } from "@/lib/types";

export function App() {
  const { notifications, loading, deleteNotification, deleteByPanes, deleteAll, newIds } =
    useNotifications();

  const { globalMuted, isRepoMuted, toggleGlobalMute, toggleRepoMute } = useMute();

  const { groups: sessionGroups, fetchVersion, statusReady } = useSessions();

  const [appVersion, setAppVersion] = useState("");
  const { updateStatus, triggerInstall, checkForUpdates } = useAppUpdate();

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, []);

  const [selectedKey, setSelectedKey] = useState<SelectedKey | null>(null);
  const [showHelp, setShowHelp] = useState(false);
  const [filterNotifiedOnly, setFilterNotifiedOnly] = useState(false);
  const [showNonAgentPanes, setShowNonAgentPanes] = useState(false);
  const [manuallyToggledGroups, setManuallyToggledGroups] = useState<Set<string>>(new Set());
  const [autoExpandedPaneId, setAutoExpandedPaneId] = useState<string | null>(null);
  const autoExpandTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const needFetchVersionRef = useRef(-1);
  const repositionCancelledRef = useRef(false);
  const pendingJumpToActiveRef = useRef(false);
  const fetchVersionRef = useRef(fetchVersion);
  fetchVersionRef.current = fetchVersion;

  // Load filter setting from config on mount
  useEffect(() => {
    invoke<boolean>("get_filter_notified_only")
      .then((v) => setFilterNotifiedOnly(v))
      .catch(() => {});
    invoke<boolean>("get_show_non_agent_panes")
      .then((v) => setShowNonAgentPanes(v))
      .catch(() => {});
  }, []);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const selectedKeyRef = useRef(selectedKey);
  selectedKeyRef.current = selectedKey;
  const lastIndexRef = useRef(-1);

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
          waitingReason: null,
          agentModes: [],
          teamRole: null,
          teamName: null,
          gitRepoRoot: null,
          gitBranch: null,
        },
        notification: n,
      });
    }

    const result = Array.from(map.values());

    // Sort panes within each group: notified panes first (latest notification on top),
    // then waiting panes, then everything else in a stable order driven by pane
    // identity so running↔idle flips during polling don't reshuffle siblings.
    for (const ug of result) {
      ug.paneItems.sort((a, b) => {
        if (a.notification && b.notification) {
          return b.notification.createdAt.localeCompare(a.notification.createdAt);
        }
        if (a.notification && !b.notification) return -1;
        if (!a.notification && b.notification) return 1;

        const aPri = getPaneAgentPriority(a);
        const bPri = getPaneAgentPriority(b);
        if (aPri !== bPri) return aPri - bPri;

        const bySession = a.pane.sessionName.localeCompare(b.pane.sessionName);
        if (bySession !== 0) return bySession;
        const byWindow = a.pane.windowName.localeCompare(b.pane.windowName);
        if (byWindow !== 0) return byWindow;
        return a.pane.paneId.localeCompare(b.pane.paneId);
      });
    }

    // Consolidate team panes: all teams first (lead first per team), then solo panes.
    // This ensures flatItems order matches the visual render order in ExpandedPanes,
    // which always renders teamGroups before soloItems.
    for (const ug of result) {
      if (!ug.paneItems.some((pi) => pi.pane.teamRole)) continue;

      const teamMap = new Map<string, PaneItem[]>();
      const solos: PaneItem[] = [];
      for (const pi of ug.paneItems) {
        if (!pi.pane.teamRole) {
          solos.push(pi);
        } else {
          const key = `${pi.pane.sessionName}:${pi.pane.windowName}`;
          if (!teamMap.has(key)) teamMap.set(key, []);
          teamMap.get(key)!.push(pi);
        }
      }

      const reordered: PaneItem[] = [];
      for (const members of teamMap.values()) {
        const lead = members.find((p) => p.pane.teamRole === "lead");
        const teammates = members
          .filter((p) => p.pane.teamRole === "teammate")
          .sort((a, b) => (a.pane.teamName ?? "").localeCompare(b.pane.teamName ?? ""));
        if (lead) reordered.push(lead);
        reordered.push(...teammates);
      }
      reordered.push(...solos);
      ug.paneItems = reordered;
    }

    // Sort: notifications first (createdAt desc), then waiting groups, then
    // alphabetical by repoName with groupKey as a stable secondary key so
    // worktree siblings (same repoName, different paths) don't swap on polls.
    result.sort((a, b) => {
      const aLatestTime = getLatestTime(a);
      const bLatestTime = getLatestTime(b);
      if (aLatestTime && bLatestTime) {
        return bLatestTime.localeCompare(aLatestTime);
      }
      if (aLatestTime && !bLatestTime) return -1;
      if (!aLatestTime && bLatestTime) return 1;

      const aPriority = getGroupAgentPriority(a);
      const bPriority = getGroupAgentPriority(b);
      if (aPriority !== bPriority) return aPriority - bPriority;

      const byRepo = a.repoName.localeCompare(b.repoName);
      if (byRepo !== 0) return byRepo;
      return a.groupKey.localeCompare(b.groupKey);
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

  // Resolve the numeric index of the currently-selected key. Falls back to the
  // last known index (clamped) when the key disappears so the cursor stays near
  // its old position instead of snapping to the top or activating a random item.
  const selectedIndex = useMemo(() => {
    if (flatItems.length === 0) {
      lastIndexRef.current = -1;
      return -1;
    }
    const hit = indexOfKey(flatItems, selectedKey);
    if (hit >= 0) {
      lastIndexRef.current = hit;
      return hit;
    }
    if (selectedKey === null) {
      return -1;
    }
    const clamped = Math.min(Math.max(lastIndexRef.current, 0), flatItems.length - 1);
    return clamped;
  }, [flatItems, selectedKey]);

  // Reset selection when panel is shown
  useEffect(() => {
    const unlisten = listen("panel:shown", () => {
      setSelectedKey(null);
      lastIndexRef.current = -1;
      repositionCancelledRef.current = false;
      needFetchVersionRef.current = fetchVersionRef.current;
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // Pick the initial cursor position when fresh data arrives and no selection
  // exists yet. Once a key is set, we never overwrite it here — the derived
  // selectedIndex memo handles "item disappeared" via nearest-index fallback.
  useEffect(() => {
    if (flatItems.length === 0) {
      if (selectedKey !== null) setSelectedKey(null);
      return;
    }
    if (selectedKey !== null) return;
    if (repositionCancelledRef.current) return;
    // Wait until sessions data is fresh after panel show
    if (needFetchVersionRef.current >= 0 && fetchVersion <= needFetchVersionRef.current) {
      return;
    }

    // Hold selection if the active pane's group is manually collapsed.
    // The auto-expand useEffect will remove it from manuallyToggledGroups,
    // re-render will include the active pane in flatItems, and this effect
    // will re-run and pick the active pane below.
    if (!filterNotifiedOnly) {
      const activeGroup = displayGroups.find((ug) => ug.paneItems.some((pi) => pi.pane.isActive));
      if (
        activeGroup &&
        manuallyToggledGroups.has(activeGroup.groupKey) &&
        collapsedGroups.has(activeGroup.groupKey)
      ) {
        return;
      }
    }

    needFetchVersionRef.current = -1;

    // If notifications exist, focus the pane with the most recent notification
    let latest: { idx: number; at: string } | null = null;
    for (let i = 0; i < flatItems.length; i++) {
      const f = flatItems[i];
      if (f.type === "pane-item" && f.paneItem.notification) {
        if (!latest || f.paneItem.notification.createdAt > latest.at) {
          latest = { idx: i, at: f.paneItem.notification.createdAt };
        }
      }
    }
    if (latest) {
      setSelectedKey(keyFromItem(flatItems[latest.idx]));
      return;
    }

    if (!filterNotifiedOnly) {
      const activeIdx = flatItems.findIndex(
        (f) => f.type === "pane-item" && f.paneItem.pane.isActive,
      );
      if (activeIdx >= 0) {
        setSelectedKey(keyFromItem(flatItems[activeIdx]));
        return;
      }
    }
    const idx = flatItems.findIndex((f) => f.type !== "group-header");
    const fallback = idx >= 0 ? idx : 0;
    setSelectedKey(keyFromItem(flatItems[fallback]));
  }, [
    flatItems,
    filterNotifiedOnly,
    fetchVersion,
    selectedKey,
    displayGroups,
    manuallyToggledGroups,
    collapsedGroups,
  ]);

  // On panel show, auto-expand the active pane's group if it was manually
  // collapsed, so the cursor can reach the active pane. Runs only while the
  // initial cursor positioning is pending (no selection, no user interaction).
  useEffect(() => {
    if (selectedKeyRef.current !== null) return;
    if (repositionCancelledRef.current) return;
    if (filterNotifiedOnly) return;
    if (needFetchVersionRef.current >= 0 && fetchVersion <= needFetchVersionRef.current) {
      return;
    }
    const activeGroup = displayGroups.find((ug) => ug.paneItems.some((pi) => pi.pane.isActive));
    if (!activeGroup) return;
    if (!manuallyToggledGroups.has(activeGroup.groupKey)) return;
    if (!collapsedGroups.has(activeGroup.groupKey)) return;
    setManuallyToggledGroups((prev) => {
      if (!prev.has(activeGroup.groupKey)) return prev;
      const next = new Set(prev);
      next.delete(activeGroup.groupKey);
      return next;
    });
  }, [displayGroups, manuallyToggledGroups, collapsedGroups, filterNotifiedOnly, fetchVersion]);

  // Shift+T: jump cursor to the currently active tmux pane once it appears in flatItems
  useEffect(() => {
    if (!pendingJumpToActiveRef.current) return;
    const activeIdx = flatItems.findIndex(
      (f) => f.type === "pane-item" && f.paneItem.pane.isActive,
    );
    if (activeIdx >= 0) {
      pendingJumpToActiveRef.current = false;
      repositionCancelledRef.current = true;
      setSelectedKey(keyFromItem(flatItems[activeIdx]));
    }
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

  // Delayed auto-expand: after 300ms on a selected pane with body text, expand it
  useEffect(() => {
    if (autoExpandTimerRef.current) {
      clearTimeout(autoExpandTimerRef.current);
      autoExpandTimerRef.current = null;
    }
    setAutoExpandedPaneId(null);

    if (selectedIndex < 0) return;
    const item = flatItems[selectedIndex];
    if (item?.type !== "pane-item" || !item.paneItem.notification?.body) return;

    const paneId = item.paneItem.pane.paneId;
    autoExpandTimerRef.current = setTimeout(() => {
      setAutoExpandedPaneId(paneId);
      autoExpandTimerRef.current = null;
    }, 300);

    return () => {
      if (autoExpandTimerRef.current) {
        clearTimeout(autoExpandTimerRef.current);
        autoExpandTimerRef.current = null;
      }
    };
  }, [selectedIndex, flatItems]);

  // Scroll into view after auto-expand (card height may change)
  useEffect(() => {
    if (!autoExpandedPaneId) return;
    const container = scrollContainerRef.current;
    if (!container) return;
    const el = container.querySelector(`[data-nav-index="${selectedIndex}"]`);
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [autoExpandedPaneId, selectedIndex]);

  const activatePaneItem = useCallback((paneItem: PaneItem) => {
    if (paneItem.notification) {
      void invoke("delete_notifications_by_pane", {
        tmuxPane: paneItem.notification.tmuxPane,
      });
    }
    void invoke("focus_terminal", {
      tmuxPane: paneItem.pane.paneId,
      terminalBundleId: paneItem.notification?.terminalBundleId ?? "",
    });
  }, []);

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
        repositionCancelledRef.current = true;
        if (flatItems.length > 0) {
          const cur = indexOfKey(flatItems, selectedKeyRef.current);
          const base = cur >= 0 ? cur : lastIndexRef.current;
          const nextIdx = Math.min(base + 1, flatItems.length - 1);
          setSelectedKey(keyFromItem(flatItems[nextIdx]));
        }
        break;
      case "k":
        if (showHelp) break;
        e.preventDefault();
        repositionCancelledRef.current = true;
        if (flatItems.length > 0) {
          const cur = indexOfKey(flatItems, selectedKeyRef.current);
          const base = cur >= 0 ? cur : lastIndexRef.current;
          const nextIdx = Math.max(base - 1, 0);
          setSelectedKey(keyFromItem(flatItems[nextIdx]));
        }
        break;
      case "Enter": {
        if (showHelp) break;
        e.preventDefault();
        repositionCancelledRef.current = true;
        // Resolve by key at keypress time — protects against flatItems having
        // been re-sorted (e.g. a new notification) between last render and now.
        const resolvedIdx = indexOfKey(flatItems, selectedKeyRef.current);
        const item = resolvedIdx >= 0 ? flatItems[resolvedIdx] : undefined;
        if (!item) {
          void invoke("hide_panel");
          break;
        }
        if (item.type === "group-header") {
          toggleGroupExpanded(item.groupKey);
        } else if (item.type === "pane-item") {
          void invoke("hide_panel");
          activatePaneItem(item.paneItem);
        }
        break;
      }
      case "d": {
        if (showHelp || e.shiftKey) break;
        e.preventDefault();
        const resolvedIdx = indexOfKey(flatItems, selectedKeyRef.current);
        const item = resolvedIdx >= 0 ? flatItems[resolvedIdx] : undefined;
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
            } else if (!filterNotifiedOnly && ug.paneItems.some((pi) => pi.pane.isActive)) {
              // Active group is auto-expanded, so toggle it to collapse
              next.add(ug.groupKey);
            }
          }
          return next;
        });
        if (flatItems.length > 0) {
          setSelectedKey(keyFromItem(flatItems[0]));
        } else {
          setSelectedKey(null);
        }
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
              if (!filterNotifiedOnly && ug.paneItems.some((pi) => pi.pane.isActive)) {
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
        // flatItems will be rebuilt on the next render; clear key so the
        // reposition effect picks the new cursor the same way it does on open.
        setSelectedKey(null);
        repositionCancelledRef.current = false;
        break;
      }
      case "T": {
        if (showHelp) break;
        e.preventDefault();
        pendingJumpToActiveRef.current = true;
        setShowNonAgentPanes((prev) => {
          const next = !prev;
          void invoke("save_show_non_agent_panes", { value: next });
          return next;
        });
        break;
      }
      case "Tab": {
        if (showHelp) break;
        e.preventDefault();
        repositionCancelledRef.current = true;
        const direction = e.shiftKey ? -1 : 1;
        const curIdx = indexOfKey(flatItems, selectedKeyRef.current);
        let nextIndex =
          curIdx < 0 ? (direction === 1 ? 0 : flatItems.length - 1) : curIdx + direction;
        while (nextIndex >= 0 && nextIndex < flatItems.length) {
          const fi = flatItems[nextIndex];
          const hasNotif = fi.type === "pane-item" && fi.paneItem.notification !== null;
          if (hasNotif) {
            setSelectedKey(keyFromItem(fi));
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
  const currentItem = selectedIndex >= 0 ? flatItems[selectedIndex] : undefined;
  const selectedNotificationId =
    currentItem?.type === "pane-item" && currentItem.paneItem.notification
      ? currentItem.paneItem.notification.id
      : null;
  const selectedPaneId =
    currentItem?.type === "pane-item" ? currentItem.paneItem.pane.paneId : null;
  const selectedGroupHeaderKey = currentItem?.type === "group-header" ? currentItem.groupKey : null;

  const isEmpty = displayGroups.length === 0;

  return (
    <div className="h-screen flex flex-col items-center px-4 pb-4 pt-0.5 bg-transparent">
      <div className="tray-arrow" />
      <div className="w-full flex-1 min-h-0 flex flex-col bg-[var(--panel-bg)] backdrop-blur-xl rounded-xl border border-[var(--border-primary)] shadow-2xl overflow-hidden">
        <PanelHeader
          globalMuted={globalMuted}
          filterNotifiedOnly={filterNotifiedOnly}
          showNonAgentPanes={showNonAgentPanes}
          onToggleFilter={() => {
            setFilterNotifiedOnly((prev) => {
              const next = !prev;
              void invoke("save_filter_notified_only", { value: next });
              return next;
            });
          }}
          onToggleShowNonAgentPanes={() => {
            setShowNonAgentPanes((prev) => {
              const next = !prev;
              void invoke("save_show_non_agent_panes", { value: next });
              return next;
            });
          }}
          onDeleteAll={() => void deleteAll()}
          onToggleGlobalMute={() => void toggleGlobalMute()}
          appVersion={appVersion}
          updateStatus={updateStatus}
          onUpdateInstall={triggerInstall}
          onUpdateCheck={checkForUpdates}
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
                  autoExpandedPaneId={autoExpandedPaneId}
                  statusReady={statusReady}
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

type SelectedKey = { kind: "pane"; paneId: string } | { kind: "group"; groupKey: string };

function keyFromItem(f: FlatItem): SelectedKey {
  if (f.type === "group-header") {
    return { kind: "group", groupKey: f.groupKey };
  }
  return { kind: "pane", paneId: f.paneItem.pane.paneId };
}

function indexOfKey(items: FlatItem[], key: SelectedKey | null): number {
  if (key === null) return -1;
  if (key.kind === "pane") {
    return items.findIndex((f) => f.type === "pane-item" && f.paneItem.pane.paneId === key.paneId);
  }
  return items.findIndex((f) => f.type === "group-header" && f.groupKey === key.groupKey);
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

// Only `waiting` is surfaced by sort order. `running`/`idle` share a bucket so
// status flips during polling don't shuffle neighboring rows. `none` sinks to
// the bottom when shell panes are mixed in via show_non_agent.
function getPaneAgentPriority(pi: PaneItem): number {
  const s = pi.pane.agentStatus;
  if (s === "waiting") return 1;
  if (s === "running" || s === "idle") return 2;
  return 3;
}

function getGroupAgentPriority(ug: UnifiedGroup): number {
  let best = 3;
  for (const pi of ug.paneItems) {
    const s = pi.pane.agentStatus;
    if (s === "waiting") return 1;
    if ((s === "running" || s === "idle") && best > 2) best = 2;
  }
  return best;
}
