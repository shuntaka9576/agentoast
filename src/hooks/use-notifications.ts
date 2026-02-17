import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Notification } from "@/lib/types";

export function useNotifications() {
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  const [loading, setLoading] = useState(true);
  const [newIds, setNewIds] = useState<Set<number>>(new Set());
  const clearNewTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [notifs, count] = await Promise.all([
        invoke<Notification[]>("get_notifications", { limit: 100 }),
        invoke<number>("get_unread_count"),
      ]);
      setNotifications(notifs);
      setUnreadCount(count);
    } catch (e) {
      console.error("Failed to fetch notifications:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();

    const unlisten1 = listen<Notification[]>("notifications:new", (event) => {
      const ids = new Set(event.payload.map((n) => n.id));
      setNewIds((prev) => new Set([...prev, ...ids]));

      if (clearNewTimerRef.current) {
        clearTimeout(clearNewTimerRef.current);
      }
      clearNewTimerRef.current = setTimeout(() => {
        setNewIds(new Set());
      }, 3000);

      void refresh();
    });

    const unlisten2 = listen<number>("notifications:unread-count", (event) => {
      setUnreadCount(event.payload);
      void refresh();
    });

    const unlisten3 = listen("notifications:refresh", () => {
      void refresh();
    });

    return () => {
      unlisten1.then((f) => f()).catch(() => {});
      unlisten2.then((f) => f()).catch(() => {});
      unlisten3.then((f) => f()).catch(() => {});
      if (clearNewTimerRef.current) {
        clearTimeout(clearNewTimerRef.current);
      }
    };
  }, [refresh]);

  const deleteNotification = useCallback(
    async (id: number) => {
      await invoke("delete_notification", { id });
      await refresh();
    },
    [refresh],
  );

  const deleteByPanes = useCallback(
    async (paneIds: string[]) => {
      await invoke("delete_notifications_by_panes", { paneIds });
      await refresh();
    },
    [refresh],
  );

  const deleteAll = useCallback(async () => {
    await invoke("delete_all_notifications");
    await refresh();
  }, [refresh]);

  return {
    notifications,
    unreadCount,
    loading,
    refresh,
    deleteNotification,
    deleteByPanes,
    deleteAll,
    newIds,
  };
}
