import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import type { TmuxPane } from "@/lib/types";
import { IconPreset } from "@/components/icons/source-icon";
import { Circle } from "lucide-react";

interface SessionIndicatorProps {
  pane: TmuxPane;
  isSelected?: boolean;
  navIndex?: number;
}

export function SessionIndicator({ pane, isSelected, navIndex }: SessionIndicatorProps) {
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
        "group relative px-3 py-1.5 hover:bg-[var(--hover-bg)] transition-colors cursor-pointer",
        isSelected && "bg-[var(--hover-bg)]",
      )}
      onClick={handleClick}
    >
      <div className="flex items-center gap-2">
        <div className="flex-shrink-0">
          {pane.agentType && (
            <IconPreset
              icon={pane.agentType}
              size={13}
              className="text-[var(--text-secondary)]"
            />
          )}
        </div>

        <span className="text-[11px] text-[var(--text-secondary)] truncate flex-1">
          {pane.agentType}
          <span className="text-[var(--text-muted)]">
            {" "}&middot; {pane.sessionName}:{pane.windowName} {pane.paneId}
          </span>
        </span>

        <div className="flex-shrink-0 flex items-center gap-1">
          <Circle size={5} className="text-green-500 fill-green-500" />
          <span className="text-[10px] text-green-500 font-medium">Running</span>
        </div>
      </div>
    </div>
  );
}
