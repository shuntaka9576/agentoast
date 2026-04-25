interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  id?: string;
  ariaLabel?: string;
  disabled?: boolean;
}

export function Toggle({ checked, onChange, id, ariaLabel, disabled = false }: ToggleProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={
        "relative inline-flex h-[20px] w-[34px] shrink-0 items-center rounded-full transition-colors duration-150 outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] focus-visible:ring-offset-1 focus-visible:ring-offset-[var(--panel-bg)] disabled:opacity-40 " +
        (checked
          ? "bg-[var(--accent)]"
          : "bg-[var(--switch-track)] hover:bg-[var(--hover-bg-strong)]")
      }
    >
      <span
        className={
          "inline-block h-[16px] w-[16px] transform rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.25)] transition-transform duration-150 ease-out " +
          (checked ? "translate-x-[16px]" : "translate-x-[2px]")
        }
      />
    </button>
  );
}
