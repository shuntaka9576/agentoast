import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TmuxPane, TmuxPaneGroup } from "@/lib/types";

function shallowEqualPanes(a: TmuxPane, b: TmuxPane): boolean {
  return (
    a.paneId === b.paneId &&
    a.agentType === b.agentType &&
    a.agentStatus === b.agentStatus &&
    a.waitingReason === b.waitingReason &&
    a.teamRole === b.teamRole &&
    a.teamName === b.teamName &&
    a.isActive === b.isActive &&
    a.agentModes.length === b.agentModes.length &&
    a.agentModes.every((m, i) => m === b.agentModes[i])
  );
}

function shallowEqualGroups(
  a: TmuxPaneGroup[],
  b: TmuxPaneGroup[],
): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i].currentPath !== b[i].currentPath) return false;
    if (a[i].gitBranch !== b[i].gitBranch) return false;
    if (a[i].panes.length !== b[i].panes.length) return false;
    for (let j = 0; j < a[i].panes.length; j++) {
      if (!shallowEqualPanes(a[i].panes[j], b[i].panes[j])) return false;
    }
  }
  return true;
}

export function useSessions() {
  const [groups, setGroups] = useState<TmuxPaneGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [fetchVersion, setFetchVersion] = useState(0);
  const mountedRef = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<TmuxPaneGroup[]>("get_sessions");
      if (mountedRef.current) {
        setGroups((prev) => {
          if (shallowEqualGroups(prev, result)) return prev;
          return result;
        });
        setFetchVersion((v) => v + 1);
        setError(null);
      }
    } catch (e) {
      if (mountedRef.current) {
        setError(String(e));
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    void refresh();

    return () => {
      mountedRef.current = false;
    };
  }, [refresh]);

  useEffect(() => {
    const unlisten = listen("notifications:refresh", () => {
      void refresh();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [refresh]);

  return { groups, loading, error, refresh, fetchVersion };
}
