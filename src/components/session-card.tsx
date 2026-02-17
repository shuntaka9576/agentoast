import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import type { TmuxPane } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";
import { Circle } from "lucide-react";

interface SessionCardProps {
  pane: TmuxPane;
  isSelected?: boolean;
  navIndex?: number;
}

export function SessionCard({ pane, isSelected, navIndex }: SessionCardProps) {
  const handleClick = () => {
    void invoke("focus_terminal", {
      tmuxPane: pane.paneId,
      terminalBundleId: "",
    });
    void invoke("hide_panel");
  };

  return (
    <div
      data-nav-index={navIndex}
      className={cn(
        "group relative px-3 py-2 hover:bg-[var(--hover-bg)] transition-colors cursor-pointer",
        isSelected && "bg-[var(--hover-bg)]",
      )}
      onClick={handleClick}
    >
      <div className="flex items-center gap-2.5">
        <div className="flex-shrink-0">
          {pane.agentType ? (
            <IconPreset
              icon={pane.agentType}
              size={14}
              className="text-[var(--text-secondary)]"
            />
          ) : (
            <TmuxIcon size={14} className="text-[var(--text-muted)]" />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] text-[var(--text-secondary)] truncate">
              {pane.sessionName}:{pane.windowName}
            </span>
            <span className="text-[10px] text-[var(--text-muted)]">
              {pane.paneId}
            </span>
          </div>
        </div>

        <div className="flex-shrink-0 flex items-center gap-1">
          {pane.agentType ? (
            <>
              <Circle size={6} className="text-green-500 fill-green-500" />
              <span className="text-[10px] text-green-500 font-medium">
                {pane.agentType}
              </span>
            </>
          ) : (
            <>
              <Circle size={6} className="text-[var(--text-muted)]" />
              <span className="text-[10px] text-[var(--text-muted)]">Idle</span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
