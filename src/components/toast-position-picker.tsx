import { Check } from "lucide-react";
import type { ToastPosition } from "@/lib/settings-types";
import { cn } from "@/lib/utils";

interface ToastPositionPickerProps {
  value: ToastPosition[];
  onChange: (next: ToastPosition[]) => void;
}

const OPTIONS: { id: ToastPosition; label: string }[] = [
  { id: "top-left", label: "Top Left" },
  { id: "top-right", label: "Top Right" },
  { id: "bottom-left", label: "Bottom Left" },
  { id: "bottom-right", label: "Bottom Right" },
];

export function ToastPositionPicker({ value, onChange }: ToastPositionPickerProps) {
  const toggle = (pos: ToastPosition) => {
    if (value.includes(pos)) {
      onChange(value.filter((p) => p !== pos));
      return;
    }
    // Preserve a canonical display order so the saved array doesn't shift
    // based on click sequence — purely cosmetic for config.toml diffs.
    const next = OPTIONS.map((o) => o.id).filter((id) => value.includes(id) || id === pos);
    onChange(next);
  };

  return (
    <div className="px-3.5 py-2.5">
      <div className="grid grid-cols-4 gap-2">
        {OPTIONS.map((opt) => {
          const checked = value.includes(opt.id);
          return (
            <button
              key={opt.id}
              type="button"
              role="checkbox"
              aria-checked={checked}
              aria-label={opt.label}
              onClick={() => toggle(opt.id)}
              className="flex flex-col items-stretch gap-1.5 rounded-md p-1 text-left transition-colors hover:bg-[var(--row-hover)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
            >
              <PreviewTile position={opt.id} selected={checked} />
              <div className="flex items-center gap-1.5">
                <CheckboxBox checked={checked} />
                <span className="text-[11px] font-medium text-[var(--text-primary)]">
                  {opt.label}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

interface PreviewTileProps {
  position: ToastPosition;
  selected: boolean;
}

function PreviewTile({ position, selected }: PreviewTileProps) {
  const corner = cornerStyles(position);
  return (
    <div
      className={cn(
        "relative w-full overflow-hidden rounded border",
        // 16:9 aspect; Tailwind's aspect-video does exactly this without
        // pulling in extra layout math.
        "aspect-video",
        selected
          ? "border-[var(--accent)] ring-1 ring-[var(--accent)]"
          : "border-[var(--border-subtle)]",
      )}
      style={{ backgroundColor: "#ffffff" }}
    >
      <span
        className="absolute rounded-[2px]"
        style={{
          backgroundColor: "var(--accent)",
          width: "30%",
          height: "16%",
          ...corner,
        }}
      />
    </div>
  );
}

function cornerStyles(position: ToastPosition): React.CSSProperties {
  switch (position) {
    case "top-left":
      return { top: "10%", left: "8%" };
    case "top-right":
      return { top: "10%", right: "8%" };
    case "bottom-left":
      return { bottom: "10%", left: "8%" };
    case "bottom-right":
      return { bottom: "10%", right: "8%" };
  }
}

interface CheckboxBoxProps {
  checked: boolean;
}

function CheckboxBox({ checked }: CheckboxBoxProps) {
  return (
    <span
      aria-hidden
      className={cn(
        "flex h-3 w-3 shrink-0 items-center justify-center rounded-[3px] border transition-colors",
        checked
          ? "border-[var(--accent)] bg-[var(--accent)] text-white"
          : "border-[var(--text-tertiary)] bg-transparent",
      )}
    >
      {checked && <Check size={9} strokeWidth={3} />}
    </span>
  );
}
