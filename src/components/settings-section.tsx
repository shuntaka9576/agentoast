import { Children, type ReactNode } from "react";

interface SettingsSectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsSection({ title, description, children }: SettingsSectionProps) {
  const rows = Children.toArray(children);

  return (
    <section className="mb-5">
      <header className="mb-2 px-1">
        <h2 className="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-tertiary)]">
          {title}
        </h2>
        {description && (
          <p className="mt-1 text-[11px] text-[var(--text-tertiary)]">{description}</p>
        )}
      </header>
      <div className="rounded-lg bg-[var(--surface-card)] shadow-[var(--shadow-card)]">
        {rows.map((row, idx) => (
          <div key={idx} className={idx > 0 ? "border-t border-[var(--border-subtle)]" : ""}>
            {row}
          </div>
        ))}
      </div>
    </section>
  );
}

interface SettingsRowProps {
  label: string;
  hint?: string;
  htmlFor?: string;
  children: ReactNode;
}

export function SettingsRow({ label, hint, htmlFor, children }: SettingsRowProps) {
  return (
    <div className="flex items-center gap-4 px-3.5 py-2.5 transition-colors hover:bg-[var(--row-hover)]">
      <div className="flex min-w-0 flex-1 flex-col">
        <label htmlFor={htmlFor} className="text-[12px] font-medium text-[var(--text-primary)]">
          {label}
        </label>
        {hint && (
          <span className="mt-0.5 text-[11px] leading-snug text-[var(--text-tertiary)]">
            {hint}
          </span>
        )}
      </div>
      <div className="flex shrink-0 items-center">{children}</div>
    </div>
  );
}
