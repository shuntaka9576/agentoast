import { useEffect, useRef } from "react";

interface KeybindHelpProps {
  onClose: () => void;
}

interface Keybind {
  key: string;
  action: string;
}

interface SubGroup {
  title?: string;
  binds: Keybind[];
}

interface Section {
  title: string;
  groups: SubGroup[];
}

const SECTIONS: Section[] = [
  {
    title: "Navigation",
    groups: [
      {
        title: "Step",
        binds: [
          { key: "j", action: "Next" },
          { key: "k", action: "Previous" },
          { key: "gg", action: "Jump to top" },
          { key: "G", action: "Jump to bottom" },
        ],
      },
      {
        title: "By status",
        binds: [
          { key: "Tab", action: "Next notif" },
          { key: "S-Tab", action: "Prev notif" },
          { key: "r", action: "Next running" },
          { key: "R", action: "Prev running" },
        ],
      },
      {
        title: "tmux",
        binds: [{ key: "t", action: "Jump to current pane" }],
      },
    ],
  },
  {
    title: "Actions",
    groups: [
      {
        binds: [
          { key: "d", action: "Delete notif" },
          { key: "D", action: "Delete all" },
          { key: "C", action: "Collapse all" },
          { key: "E", action: "Expand all" },
          { key: "F", action: "Filter notified" },
          { key: "T", action: "Show all tmux panes" },
          { key: "y", action: "Copy $TMUX_PANE" },
          { key: "a", action: "Toggle apps" },
          { key: "Enter", action: "Open / Fold" },
        ],
      },
    ],
  },
  {
    title: "Search",
    groups: [
      {
        binds: [
          { key: "n", action: "Next match" },
          { key: "N", action: "Prev match" },
          { key: "/", action: "Search repos" },
        ],
      },
    ],
  },
  {
    title: "Misc",
    groups: [
      {
        binds: [
          { key: "?", action: "Help" },
          { key: "Esc", action: "Close" },
        ],
      },
    ],
  },
];

export function KeybindHelp({ onClose }: KeybindHelpProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const el = containerRef.current;
      if (!el) return;
      if (e.metaKey || e.altKey) return;

      const lineStep = 28;
      switch (e.key) {
        case "j":
          if (e.ctrlKey || e.shiftKey) return;
          el.scrollBy({ top: lineStep });
          break;
        case "k":
          if (e.ctrlKey || e.shiftKey) return;
          el.scrollBy({ top: -lineStep });
          break;
        case "g":
          if (e.ctrlKey || e.shiftKey) return;
          el.scrollTo({ top: 0 });
          break;
        case "G":
          if (e.ctrlKey) return;
          el.scrollTo({ top: el.scrollHeight });
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div
      className="absolute inset-0 z-10 flex flex-col bg-[var(--panel-bg)]"
      onClick={onClose}
      tabIndex={-1}
    >
      <div
        ref={containerRef}
        className="flex-1 overflow-y-auto px-4 py-4 flex items-start justify-center"
      >
        <div className="w-full flex flex-col gap-3" onClick={(e) => e.stopPropagation()}>
          {SECTIONS.map((section) => (
            <div key={section.title} className="flex flex-col gap-1">
              <div className="text-[9px] uppercase tracking-wider font-semibold text-[var(--text-muted)] pl-1">
                {section.title}
              </div>
              <div className="flex flex-col gap-1.5">
                {section.groups.map((group, gi) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: subgroup index is stable per section
                  <div key={gi} className="flex flex-col gap-0.5">
                    {group.title && (
                      <div className="text-[9px] tracking-wide text-[var(--text-muted)] opacity-60 pl-1">
                        {group.title}
                      </div>
                    )}
                    <div className="grid grid-cols-2 gap-x-3 gap-y-1">
                      {group.binds.map(({ key, action }) => (
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
          ))}
        </div>
      </div>
      <div
        className="border-t border-[var(--border-subtle)] px-3 py-1.5 flex items-center gap-3 text-[9px] text-[var(--text-muted)] bg-[var(--panel-bg)]"
        onClick={(e) => e.stopPropagation()}
      >
        <span className="flex items-center gap-1">
          <kbd className="font-mono text-[var(--text-primary)]">j/k</kbd>
          scroll
        </span>
        <span className="flex items-center gap-1">
          <kbd className="font-mono text-[var(--text-primary)]">g/G</kbd>
          top/bottom
        </span>
      </div>
    </div>
  );
}
