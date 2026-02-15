import { useNotifications } from "@/hooks/use-notifications";
import { useMute } from "@/hooks/use-mute";
import { PanelHeader } from "@/components/panel-header";
import { RepoGroup } from "@/components/repo-group";
import { Bell } from "lucide-react";

export function App() {
  const {
    groups,
    unreadCount,
    loading,
    deleteNotification,
    deleteGroup,
    deleteAll,
    newIds,
  } = useNotifications();

  const {
    globalMuted,
    isGroupMuted,
    toggleGlobalMute,
    toggleGroupMute,
  } = useMute();

  return (
    <div className="h-screen flex flex-col bg-[var(--panel-bg)] backdrop-blur-xl rounded-xl border border-[var(--border-primary)] shadow-2xl overflow-hidden">
      {/* Tray arrow */}
      <div className="flex justify-center -mt-[7px]">
        <div className="tray-arrow" />
      </div>

      <PanelHeader
        unreadCount={unreadCount}
        globalMuted={globalMuted}
        onDeleteAll={() => void deleteAll()}
        onToggleGlobalMute={() => void toggleGlobalMute()}
      />

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-xs text-[var(--text-muted)]">Loading...</div>
          </div>
        ) : groups.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-3">
            <Bell size={32} className="text-[var(--text-faint)]" />
            <p className="text-xs text-[var(--text-muted)]">No notifications yet</p>
          </div>
        ) : (
          groups.map((group) => (
            <RepoGroup
              key={group.groupName}
              group={group}
              isMuted={isGroupMuted(group.groupName)}
              newIds={newIds}
              onDelete={(id) => void deleteNotification(id)}
              onDeleteGroup={(name) => void deleteGroup(name)}
              onToggleGroupMute={(name) => void toggleGroupMute(name)}
            />
          ))
        )}
      </div>
    </div>
  );
}
