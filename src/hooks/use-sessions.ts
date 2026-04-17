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

function shallowEqualGroups(a: TmuxPaneGroup[], b: TmuxPaneGroup[]): boolean {
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
  const [statusReady, setStatusReady] = useState(false);
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
        setStatusReady(true);
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

  // Receive cached sessions emitted by show_panel before the live get_sessions completes.
  // This populates the UI immediately on panel re-open, avoiding a blank flash while
  // get_sessions runs (150-300ms). Skip setLoading/setFetchVersion so the cache is treated
  // as a render hint, not a data source — the live refresh still drives canonical state.
  useEffect(() => {
    const unlisten = listen<TmuxPaneGroup[]>("sessions:cached", (event) => {
      if (!mountedRef.current) return;
      const cached = event.payload;
      setGroups((prev) => {
        if (shallowEqualGroups(prev, cached)) return prev;
        return cached;
      });
      setLoading(false);
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  // Push updates from the topology-driven event loop in the backend. Treated as a
  // render hint (like sessions:cached) — never bumps fetchVersion. Cursor reposition
  // on panel open is driven exclusively by the invoke(get_sessions) round-trip via
  // notifications:refresh, so backend-initiated pushes must not race with the
  // panel:shown → needFetchVersionRef capture sequence in App.tsx.
  useEffect(() => {
    const unlisten = listen<TmuxPaneGroup[]>("sessions:updated", (event) => {
      if (!mountedRef.current) return;
      const next = event.payload;
      setGroups((prev) => {
        if (shallowEqualGroups(prev, next)) return prev;
        return next;
      });
      setLoading(false);
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  return { groups, loading, error, refresh, fetchVersion, statusReady };
}
