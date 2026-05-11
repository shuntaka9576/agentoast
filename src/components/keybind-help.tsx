interface KeybindHelpProps {
  onClose: () => void;
}

interface Keybind {
  key: string;
  action: string;
}

interface Section {
  title: string;
  binds: Keybind[];
}

const SECTIONS: Section[] = [
  {
    title: "Navigation",
    binds: [
      { key: "j", action: "Next" },
      { key: "k", action: "Previous" },
      { key: "gg", action: "Jump to top" },
      { key: "G", action: "Jump to bottom" },
      { key: "Tab", action: "Next notif" },
      { key: "⇧Tab", action: "Prev notif" },
    ],
  },
  {
    title: "Search",
    binds: [
      { key: "/", action: "Search repos" },
      { key: "n", action: "Next match" },
      { key: "N", action: "Prev match" },
    ],
  },
  {
    title: "Actions",
    binds: [
      { key: "Enter", action: "Open / Fold" },
      { key: "d", action: "Delete notif" },
      { key: "D", action: "Delete all" },
      { key: "C", action: "Collapse all" },
      { key: "E", action: "Expand all" },
      { key: "F", action: "Filter notified" },
      { key: "T", action: "Non-agent panes" },
      { key: "t", action: "Jump to current pane" },
      { key: "y", action: "Copy $TMUX_PANE" },
      { key: "a", action: "Toggle apps" },
    ],
  },
  {
    title: "Misc",
    binds: [
      { key: "?", action: "Help" },
      { key: "Esc", action: "Close" },
    ],
  },
];

export function KeybindHelp({ onClose }: KeybindHelpProps) {
  return (
    <div
      className="absolute inset-0 z-10 flex items-start justify-center overflow-y-auto px-4 py-4 bg-[var(--panel-bg)]"
      onClick={onClose}
      tabIndex={-1}
    >
      <div className="w-full flex flex-col gap-3" onClick={(e) => e.stopPropagation()}>
        {SECTIONS.map((section) => (
          <div key={section.title} className="flex flex-col gap-1">
            <div className="text-[9px] uppercase tracking-wider font-semibold text-[var(--text-muted)] pl-1">
              {section.title}
            </div>
            <div className="grid grid-cols-2 gap-x-3 gap-y-1">
              {section.binds.map(({ key, action }) => (
                <div key={key} className="flex items-center gap-2">
                  <span className="min-w-[44px] text-center text-[10px] font-mono py-0.5 px-1.5 rounded bg-[var(--hover-bg-strong)] text-[var(--text-primary)] border border-[var(--border-subtle)]">
                    {key}
                  </span>
                  <span className="text-[11px] text-[var(--text-secondary)] truncate">
                    {action}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
