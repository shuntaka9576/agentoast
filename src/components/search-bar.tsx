import { useEffect, useRef } from "react";

interface SearchBarProps {
  query: string;
  matchCount: number;
  matchPosition: number; // 1-indexed; 0 when matchCount === 0
  onChange: (q: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}

export function SearchBar({
  query,
  matchCount,
  matchPosition,
  onChange,
  onConfirm,
  onCancel,
}: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const indicator =
    query.trim() === "" ? "" : matchCount === 0 ? "no matches" : `${matchPosition}/${matchCount}`;

  return (
    <div className="flex items-center gap-1 px-3 py-1.5 border-t border-[var(--border-primary)] bg-[var(--panel-bg)] font-mono text-[11px] text-[var(--text-secondary)]">
      <span className="text-[var(--text-tertiary)]">/</span>
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.nativeEvent.isComposing) return;
          if (e.key === "Enter") {
            e.preventDefault();
            onConfirm();
          } else if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
        className="flex-1 bg-transparent outline-none border-none p-0 text-[var(--text-primary)]"
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
      />
      {indicator && <span className="ml-2 text-[var(--text-tertiary)]">{indicator}</span>}
    </div>
  );
}
