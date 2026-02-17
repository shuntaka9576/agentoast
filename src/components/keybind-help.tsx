interface KeybindHelpProps {
  onClose: () => void;
}

const KEYBINDS = [
  { key: "j", action: "Next" },
  { key: "k", action: "Previous" },
  { key: "Enter", action: "Open / Fold" },
  { key: "d", action: "Delete" },
  { key: "D", action: "Delete group" },
  { key: "Esc", action: "Close" },
  { key: "?", action: "Help" },
] as const;

export function KeybindHelp({ onClose }: KeybindHelpProps) {
  return (
    <div
      className="absolute inset-0 z-10 flex items-center justify-center"
      style={{ backgroundColor: "var(--panel-bg)" }}
      onClick={onClose}
      tabIndex={-1}
    >
      <div className="flex flex-col gap-2" onClick={(e) => e.stopPropagation()}>
        {KEYBINDS.map(({ key, action }) => (
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
            <span
              className="text-[11px]"
              style={{ color: "var(--text-secondary)" }}
            >
              {action}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
