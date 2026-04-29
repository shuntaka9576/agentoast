interface KeybindHelpProps {
  onClose: () => void;
}

interface Keybind {
  key: string;
  action: string;
  experimental?: boolean;
}

const KEYBINDS: readonly Keybind[] = [
  { key: "j", action: "Next" },
  { key: "k", action: "Previous" },
  { key: "Enter", action: "Open / Fold" },
  { key: "a", action: "Apps view" },
  { key: "e", action: "Easy motion jump", experimental: true },
  { key: "d", action: "Delete notif" },
  { key: "D", action: "Delete all notifs" },
  { key: "C", action: "Collapse all" },
  { key: "E", action: "Expand all" },
  { key: "F", action: "Filter action needed" },
  { key: "T", action: "Toggle non-agent panes" },
  { key: "Esc", action: "Close" },
  { key: "?", action: "Help" },
];

export function KeybindHelp({ onClose }: KeybindHelpProps) {
  return (
    <div
      className="absolute inset-0 z-10 flex items-center justify-center"
      style={{ backgroundColor: "var(--panel-bg)" }}
      onClick={onClose}
      tabIndex={-1}
    >
      <div className="flex flex-col gap-2" onClick={(e) => e.stopPropagation()}>
        {KEYBINDS.map(({ key, action, experimental }) => (
          <div key={key} className="flex items-center gap-3">
            <span
              className="w-12 text-center text-[11px] font-mono py-0.5 px-1.5 rounded"
              style={{
                backgroundColor: "var(--hover-bg-strong)",
                color: "var(--text-primary)",
                border: "1px solid var(--border-subtle)",
              }}
            >
              {key}
            </span>
            <span className="text-[11px]" style={{ color: "var(--text-secondary)" }}>
              {action}
            </span>
            {experimental && (
              <span className="text-[9px] font-medium uppercase tracking-wide px-1 py-px rounded bg-amber-500/20 text-amber-400 border border-amber-500/40">
                experimental
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
