import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AllowedApp } from "@/lib/types";
import { cn } from "@/lib/utils";

interface AppsViewProps {
  allowedApps: AllowedApp[];
  iconMap: Map<string, string>;
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  onActivate: (bundleId: string) => void;
}

export function AppsView({
  allowedApps,
  iconMap,
  selectedIndex,
  onSelectIndex,
  onActivate,
}: AppsViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current?.querySelector(`[data-app-index="${selectedIndex}"]`);
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  if (allowedApps.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
        <p className="text-xs text-[var(--text-muted)]">No apps pinned yet.</p>
        <p className="text-[11px] text-[var(--text-tertiary)]">
          Pin apps from Settings → Apps so you can launch them from here.
        </p>
        <button
          type="button"
          onClick={() => void invoke("show_settings")}
          className="rounded-md bg-[var(--accent)] px-3 py-1.5 text-xs font-medium text-white hover:bg-[var(--accent-hover)]"
        >
          Open Settings
        </button>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="grid grid-cols-3 gap-2 p-3">
      {allowedApps.map((app, idx) => {
        const icon = iconMap.get(app.bundleId);
        const isSelected = idx === selectedIndex;
        return (
          <button
            key={app.bundleId}
            type="button"
            data-app-index={idx}
            onMouseEnter={() => onSelectIndex(idx)}
            onClick={() => onActivate(app.bundleId)}
            className={cn(
              "flex flex-col items-center justify-center gap-1.5 rounded-xl border px-2 py-3 text-[11px] transition-colors",
              isSelected
                ? "border-[var(--accent)] bg-[var(--hover-bg)] text-[var(--text-primary)]"
                : "border-[var(--border-subtle)] bg-[var(--panel-bg)] text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)]",
            )}
          >
            {icon ? (
              <img src={icon} alt="" width={36} height={36} className="rounded-md" />
            ) : (
              <div className="flex h-9 w-9 items-center justify-center rounded-md bg-[var(--hover-bg)] text-[14px] text-[var(--text-tertiary)]">
                {(app.displayName || app.bundleId).slice(0, 1).toUpperCase()}
              </div>
            )}
            <span className="line-clamp-1 max-w-full break-all">{app.displayName}</span>
          </button>
        );
      })}
    </div>
  );
}
