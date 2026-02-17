import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TmuxPaneGroup } from "@/lib/types";

const POLL_INTERVAL = 3000;

export function useSessions() {
  const [groups, setGroups] = useState<TmuxPaneGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<TmuxPaneGroup[]>("get_sessions");
      if (mountedRef.current) {
        setGroups(result);
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

    if (intervalRef.current) {
      clearInterval(intervalRef.current);
    }
    intervalRef.current = setInterval(() => {
      void refresh();
    }, POLL_INTERVAL);

    return () => {
      mountedRef.current = false;
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
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

  return { groups, loading, error, refresh };
}
