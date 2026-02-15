import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MuteState } from "@/lib/types";

export function useMute() {
  const [muteState, setMuteState] = useState<MuteState>({
    globalMuted: false,
    mutedGroups: [],
  });
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;

    invoke<MuteState>("get_mute_state")
      .then((state) => {
        if (mountedRef.current) {
          setMuteState(state);
        }
      })
      .catch((e) => {
        console.error("Failed to get mute state:", e);
      });

    const unlisten = listen<MuteState>("mute:changed", (event) => {
      if (mountedRef.current) {
        setMuteState(event.payload);
      }
    });

    return () => {
      mountedRef.current = false;
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  const toggleGlobalMute = useCallback(async () => {
    try {
      const newState = await invoke<MuteState>("toggle_global_mute");
      setMuteState(newState);
    } catch (e) {
      console.error("Failed to toggle global mute:", e);
    }
  }, []);

  const toggleGroupMute = useCallback(async (groupName: string) => {
    try {
      const newState = await invoke<MuteState>("toggle_group_mute", {
        groupName,
      });
      setMuteState(newState);
    } catch (e) {
      console.error("Failed to toggle group mute:", e);
    }
  }, []);

  const isGroupMuted = useCallback(
    (groupName: string) => {
      return muteState.mutedGroups.includes(groupName);
    },
    [muteState.mutedGroups],
  );

  return {
    globalMuted: muteState.globalMuted,
    mutedGroups: muteState.mutedGroups,
    isGroupMuted,
    toggleGlobalMute,
    toggleGroupMute,
  };
}
