import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { NotificationGroup } from "@/lib/types";

export function useNotifications() {
  const [groups, setGroups] = useState<NotificationGroup[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [grouped, count] = await Promise.all([
        invoke<NotificationGroup[]>("get_notifications_grouped", { limit: 100 }),
        invoke<number>("get_unread_count"),
      ]);
      setGroups(grouped);
      setUnreadCount(count);
    } catch (e) {
      console.error("Failed to fetch notifications:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();

    const unlisten1 = listen("notifications:new", () => {
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
    };
  }, [refresh]);

  const deleteNotification = useCallback(
    async (id: number) => {
      await invoke("delete_notification", { id });
      await refresh();
    },
    [refresh],
  );

  const deleteGroup = useCallback(
    async (groupName: string) => {
      await invoke("delete_notifications_by_group", { groupName });
      await refresh();
    },
    [refresh],
  );

  const deleteAll = useCallback(async () => {
    await invoke("delete_all_notifications");
    await refresh();
  }, [refresh]);

  return {
    groups,
    unreadCount,
    loading,
    refresh,
    deleteNotification,
    deleteGroup,
    deleteAll,
  };
}
