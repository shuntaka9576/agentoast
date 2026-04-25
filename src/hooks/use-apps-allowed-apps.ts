import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AllowedApp } from "@/lib/types";

export function useAppsAllowedApps() {
  const [allowedApps, setAllowedApps] = useState<AllowedApp[]>([]);
  const [iconMap, setIconMap] = useState<Map<string, string>>(new Map());
  const [loaded, setLoaded] = useState(false);
  const mountedRef = useRef(true);

  // Resolve only the icons for the current allowlist. Far cheaper than
  // enumerating every running app — pinned apps usually number in single digits
  // while `runningApplications()` can return 50+ entries on a busy desktop.
  const resolveIconsFor = useCallback((apps: AllowedApp[]) => {
    if (apps.length === 0) {
      setIconMap(new Map());
      return;
    }
    const bundleIds = apps.map((a) => a.bundleId);
    invoke<Record<string, string>>("resolve_app_icons", { bundleIds })
      .then((map) => {
        if (!mountedRef.current) return;
        setIconMap(new Map(Object.entries(map)));
      })
      .catch((e) => {
        console.warn("resolve_app_icons failed:", e);
      });
  }, []);

  useEffect(() => {
    mountedRef.current = true;

    invoke<AllowedApp[]>("get_apps_allowed_apps")
      .then((apps) => {
        if (!mountedRef.current) return;
        setAllowedApps(apps);
        setLoaded(true);
        resolveIconsFor(apps);
      })
      .catch((e) => {
        console.error("Failed to get allowed apps:", e);
        if (mountedRef.current) setLoaded(true);
      });

    const unlisten = listen<AllowedApp[]>("apps:allowed_apps_changed", (event) => {
      if (!mountedRef.current) return;
      setAllowedApps(event.payload);
      resolveIconsFor(event.payload);
    });

    return () => {
      mountedRef.current = false;
      unlisten.then((f) => f()).catch(() => {});
    };
  }, [resolveIconsFor]);

  // Caller can request a manual refresh (e.g. on panel show, but normally not
  // needed because icons are stable between launches).
  const refreshIcons = useCallback(() => {
    resolveIconsFor(allowedApps);
  }, [resolveIconsFor, allowedApps]);

  return { allowedApps, iconMap, loaded, refreshIcons };
}
