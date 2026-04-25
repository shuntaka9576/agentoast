import { Minus, Plus } from "lucide-react";

interface NumberStepperProps {
  value: number;
  onChange: (value: number) => void;
  id?: string;
  min?: number;
  max?: number;
  step?: number;
  /** Width of the value field in px */
  width?: number;
}

export function NumberStepper({
  value,
  onChange,
  id,
  min,
  max,
  step = 1,
  width = 76,
}: NumberStepperProps) {
  const clamp = (n: number): number => {
    if (typeof min === "number" && n < min) return min;
    if (typeof max === "number" && n > max) return max;
    return n;
  };

  const dec = () => onChange(clamp(value - step));
  const inc = () => onChange(clamp(value + step));

  return (
    <div className="flex h-7 items-stretch rounded-md border border-[var(--border-primary)] bg-[var(--panel-bg)] focus-within:border-[var(--accent)]">
      <input
        id={id}
        type="number"
        className="no-spinner w-full bg-transparent px-2 text-right text-xs text-[var(--text-primary)] outline-none"
        style={{ width }}
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => {
          const next = Number(e.target.value);
          if (!Number.isNaN(next)) onChange(next);
        }}
        onBlur={(e) => {
          const v = Number(e.target.value);
          if (Number.isNaN(v)) {
            if (typeof min === "number") onChange(min);
            return;
          }
          const c = clamp(v);
          if (c !== v) onChange(c);
        }}
      />
      <div className="flex flex-col border-l border-[var(--border-subtle)]">
        <button
          type="button"
          tabIndex={-1}
          onClick={inc}
          className="flex h-1/2 w-5 items-center justify-center text-[var(--text-tertiary)] hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)]"
          aria-label="Increment"
        >
          <Plus size={9} strokeWidth={2.5} />
        </button>
        <button
          type="button"
          tabIndex={-1}
          onClick={dec}
          className="flex h-1/2 w-5 items-center justify-center border-t border-[var(--border-subtle)] text-[var(--text-tertiary)] hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)]"
          aria-label="Decrement"
        >
          <Minus size={9} strokeWidth={2.5} />
        </button>
      </div>
    </div>
  );
}
