import { useState, useRef, useEffect } from "react";
import {
  Bell,
  BellOff,
  Filter,
  Trash2,
  Loader2,
  RefreshCw,
} from "lucide-react";
import type { UpdateStatus } from "@/lib/types";

interface PanelHeaderProps {
  globalMuted: boolean;
  filterNotifiedOnly: boolean;
  onToggleFilter: () => void;
  onDeleteAll: () => void;
  onToggleGlobalMute: () => void;
  appVersion: string;
  updateStatus: UpdateStatus;
  onUpdateInstall: () => void;
  onUpdateCheck: () => void;
}

function getUpdateTitle(status: UpdateStatus, version: string): string {
  switch (status.status) {
    case "idle":
    case "checking":
      return version ? `Agentoast v${version}` : "Agentoast";
    case "up-to-date":
      return version ? `Up to date (v${version})` : "Up to date";
    case "downloading":
      return status.progress >= 0
        ? `Downloading update ${status.progress}%`
        : "Downloading update...";
    case "ready":
      return "Update ready — click to restart";
    case "installing":
      return "Installing...";
    case "error":
      return status.message === "Update check failed"
        ? "Update check failed"
        : "Update failed";
  }
}

export function PanelHeader({
  globalMuted,
  filterNotifiedOnly,
  onToggleFilter,
  onDeleteAll,
  onToggleGlobalMute,
  appVersion,
  updateStatus,
  onUpdateInstall,
  onUpdateCheck,
}: PanelHeaderProps) {
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!dropdownOpen) return;
    const handleMouseDown = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [dropdownOpen]);

  const handleUpdateClick = () => {
    if (updateStatus.status === "ready") {
      setDropdownOpen((prev) => !prev);
    } else if (updateStatus.status === "error") {
      onUpdateCheck();
    }
  };

  const isReady = updateStatus.status === "ready";
  const isDownloading = updateStatus.status === "downloading";
  const isInstalling = updateStatus.status === "installing";
  const isError = updateStatus.status === "error";
  const showUpdateIcon = isReady || isDownloading || isInstalling || isError;

  const iconColorClass = isDownloading || isInstalling
    ? "text-[var(--text-secondary)]"
    : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]";

  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-primary)]">
      <button
        tabIndex={-1}
        onClick={onToggleFilter}
        className={`p-1.5 rounded-md hover:bg-[var(--hover-bg-strong)] transition-colors ${
          filterNotifiedOnly
            ? "text-[var(--text-primary)]"
            : "text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
        }`}
        title={filterNotifiedOnly ? "Show all" : "Show notified only"}
      >
        <Filter
          size={14}
          fill={filterNotifiedOnly ? "currentColor" : "none"}
        />
      </button>
      <div className="flex items-center gap-1">
        {showUpdateIcon && (
          <div className="relative" ref={dropdownRef}>
            <button
              tabIndex={-1}
              onClick={handleUpdateClick}
              className={`p-1.5 rounded-md hover:bg-[var(--hover-bg-strong)] transition-colors ${iconColorClass}`}
              title={getUpdateTitle(updateStatus, appVersion)}
            >
              {isDownloading || isInstalling ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <RefreshCw size={14} />
              )}
            </button>
            {dropdownOpen && isReady && (
              <div className="absolute right-0 top-full mt-1 z-50 bg-[var(--panel-bg)] border border-[var(--border-primary)] rounded-lg shadow-lg p-2 min-w-[160px]">
                <button
                  tabIndex={-1}
                  onClick={() => {
                    setDropdownOpen(false);
                    onUpdateInstall();
                  }}
                  className="w-full text-xs px-3 py-1.5 rounded bg-green-600 text-white hover:bg-green-700 cursor-pointer"
                >
                  Restart to update
                </button>
              </div>
            )}
          </div>
        )}
        <button
          tabIndex={-1}
          onClick={onToggleGlobalMute}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--hover-bg-strong)] transition-colors"
          title={`${globalMuted ? "Unmute notifications" : "Mute notifications"} — Agentoast v${appVersion}`}
        >
          {globalMuted ? <BellOff size={14} /> : <Bell size={14} />}
        </button>
        <button
          tabIndex={-1}
          onClick={onDeleteAll}
          className="p-1.5 rounded-md text-[var(--text-tertiary)] hover:text-[var(--delete-hover-text)] hover:bg-[var(--hover-bg-strong)] transition-colors"
          title="Delete all"
        >
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}
