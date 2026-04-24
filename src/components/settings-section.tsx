import type { ReactNode } from "react";

interface SettingsSectionProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsSection({ title, description, children }: SettingsSectionProps) {
  return (
    <section className="border-b border-[var(--border-subtle)] px-5 py-4">
      <header className="mb-3">
        <h2 className="text-sm font-semibold text-[var(--text-primary)]">{title}</h2>
        {description && <p className="mt-0.5 text-xs text-[var(--text-tertiary)]">{description}</p>}
      </header>
      <div className="flex flex-col gap-3">{children}</div>
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
    <div className="flex items-start justify-between gap-4">
      <div className="flex min-w-0 flex-col">
        <label htmlFor={htmlFor} className="text-xs font-medium text-[var(--text-secondary)]">
          {label}
        </label>
        {hint && <span className="mt-0.5 text-[11px] text-[var(--text-tertiary)]">{hint}</span>}
      </div>
      <div className="flex-shrink-0">{children}</div>
    </div>
  );
}
