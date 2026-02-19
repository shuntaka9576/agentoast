import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TmuxPaneGroup } from "@/lib/types";

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
          if (JSON.stringify(prev) === JSON.stringify(result)) return prev;
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

  useEffect(() => {
    const unlisten = listen("notifications:new", () => {
      void refresh();
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [refresh]);

  return { groups, loading, error, refresh, fetchVersion };
}
