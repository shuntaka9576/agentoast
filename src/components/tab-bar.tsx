import { cn } from "@/lib/utils";

export type TabId = "notifications" | "sessions";

interface TabBarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  notificationCount: number;
  sessionActiveCount: number;
}

export function TabBar({
  activeTab,
  onTabChange,
  notificationCount,
  sessionActiveCount,
}: TabBarProps) {
  return (
    <div className="flex border-b border-[var(--border-primary)]">
      <button
        tabIndex={-1}
        onClick={() => onTabChange("notifications")}
        className={cn(
          "flex-1 py-2 text-[11px] font-medium transition-colors relative",
          activeTab === "notifications"
            ? "text-[var(--text-primary)]"
            : "text-[var(--text-muted)] hover:text-[var(--text-secondary)]",
        )}
      >
        Notifications
        {notificationCount > 0 && (
          <span className="ml-1 text-[10px] text-[var(--text-muted)]">
            ({notificationCount})
          </span>
        )}
        {activeTab === "notifications" && (
          <div className="absolute bottom-0 left-1/4 right-1/4 h-0.5 bg-[var(--text-primary)] rounded-full" />
        )}
      </button>
      <button
        tabIndex={-1}
        onClick={() => onTabChange("sessions")}
        className={cn(
          "flex-1 py-2 text-[11px] font-medium transition-colors relative",
          activeTab === "sessions"
            ? "text-[var(--text-primary)]"
            : "text-[var(--text-muted)] hover:text-[var(--text-secondary)]",
        )}
      >
        Sessions
        {sessionActiveCount > 0 && (
          <span className="ml-1 text-[10px] text-green-500">
            ({sessionActiveCount})
          </span>
        )}
        {activeTab === "sessions" && (
          <div className="absolute bottom-0 left-1/4 right-1/4 h-0.5 bg-[var(--text-primary)] rounded-full" />
        )}
      </button>
    </div>
  );
}
