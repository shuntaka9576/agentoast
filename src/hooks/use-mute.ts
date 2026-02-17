import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MuteState } from "@/lib/types";

export function useMute() {
  const [muteState, setMuteState] = useState<MuteState>({
    globalMuted: false,
    mutedRepos: [],
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

  const toggleRepoMute = useCallback(async (repoPath: string) => {
    try {
      const newState = await invoke<MuteState>("toggle_repo_mute", {
        repoPath,
      });
      setMuteState(newState);
    } catch (e) {
      console.error("Failed to toggle repo mute:", e);
    }
  }, []);

  const isRepoMuted = useCallback(
    (repoPath: string) => {
      return muteState.mutedRepos.includes(repoPath);
    },
    [muteState.mutedRepos],
  );

  return {
    globalMuted: muteState.globalMuted,
    mutedRepos: muteState.mutedRepos,
    isRepoMuted,
    toggleGlobalMute,
    toggleRepoMute,
  };
}
