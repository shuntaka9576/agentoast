import { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, ChevronDown, ChevronUp, X } from "lucide-react";

interface ShortcutRecorderProps {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  /** System-reserved shortcuts (macOS symbolic hotkeys). Used to dim examples
   * that would conflict and to warn when a captured shortcut matches. */
  reservedShortcuts?: string[];
}

const MODIFIER_SYMBOLS: Record<string, string> = {
  super: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
};

const MODIFIER_ORDER = ["super", "ctrl", "alt", "shift"] as const;
const MODIFIER_SET: ReadonlySet<string> = new Set(MODIFIER_ORDER);

/**
 * Normalize a shortcut combo so comparisons are robust against casing and
 * modifier-order drift (e.g. "Ctrl+Super+Space" vs "super+ctrl+Space").
 * Modifiers are sorted into a canonical order; the trailing key is kept as the
 * last token but lowercased so it collates with whatever form the backend
 * emits.
 */
function canonicalize(combo: string): string {
  if (!combo) return "";
  const parts = combo.split("+").map((p) => p.toLowerCase());
  const mods = MODIFIER_ORDER.filter((m) => parts.includes(m));
  const keys = parts.filter((p) => !MODIFIER_SET.has(p));
  return [...mods, ...keys].join("+");
}

/**
 * Static candidate pool shown in the "Examples" dropdown. These are ordered
 * most-conventional first and are filtered at render time against the
 * currently-selected value and macOS system-reserved hotkeys.
 */
const SHORTCUT_EXAMPLES: string[] = [
  "super+ctrl+n",
  "super+ctrl+a",
  "super+ctrl+j",
  "super+ctrl+k",
  "super+ctrl+Space",
  "super+shift+n",
  "super+shift+a",
  "ctrl+alt+n",
  "ctrl+alt+Space",
  "super+alt+n",
  "super+shift+alt+n",
];

function displayTokens(value: string): string[] {
  if (!value) return [];
  return value.split("+").map((p) => MODIFIER_SYMBOLS[p] ?? p.toUpperCase());
}

// Map a KeyboardEvent.code to the token Tauri's global-shortcut parser accepts.
function codeToToken(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `Num${code.slice(6)}`;
  if (/^F\d{1,2}$/.test(code)) return code;
  if (code.startsWith("Arrow")) return code.slice(5);
  switch (code) {
    case "Space":
    case "Enter":
    case "Tab":
    case "Backspace":
    case "Delete":
    case "Home":
    case "End":
    case "PageUp":
    case "PageDown":
    case "Insert":
      return code;
    case "Minus":
      return "-";
    case "Equal":
      return "=";
    case "BracketLeft":
      return "[";
    case "BracketRight":
      return "]";
    case "Backslash":
      return "\\";
    case "Semicolon":
      return ";";
    case "Quote":
      return "'";
    case "Comma":
      return ",";
    case "Period":
      return ".";
    case "Slash":
      return "/";
    case "Backquote":
      return "`";
    default:
      return null;
  }
}

interface LiveModifiers {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
}

const EMPTY_MODS: LiveModifiers = {
  meta: false,
  ctrl: false,
  alt: false,
  shift: false,
};

function liveModifierSymbols(mods: LiveModifiers): string[] {
  const out: string[] = [];
  if (mods.meta) out.push("⌘");
  if (mods.ctrl) out.push("⌃");
  if (mods.alt) out.push("⌥");
  if (mods.shift) out.push("⇧");
  return out;
}

function ExampleToken({ token }: { token: string }) {
  return (
    <kbd className="flex h-6 min-w-[24px] items-center justify-center rounded-md border border-[var(--border-primary)] bg-[var(--hover-bg)] px-1.5 font-mono text-[10px] text-[var(--text-primary)]">
      {token}
    </kbd>
  );
}

export function ShortcutRecorder({
  value,
  onChange,
  id,
  reservedShortcuts,
}: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [liveMods, setLiveMods] = useState<LiveModifiers>(EMPTY_MODS);
  const [showExamples, setShowExamples] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState(false);

  const reservedSet = useMemo(
    () => new Set((reservedShortcuts ?? []).map(canonicalize)),
    [reservedShortcuts],
  );

  const canonicalValue = useMemo(() => canonicalize(value), [value]);

  const examples = useMemo(
    () =>
      SHORTCUT_EXAMPLES.filter((s) => {
        const c = canonicalize(s);
        return c !== canonicalValue && !reservedSet.has(c);
      }),
    [canonicalValue, reservedSet],
  );

  const conflictsWithReserved = value !== "" && reservedSet.has(canonicalValue);

  useEffect(() => {
    if (!recording) return;

    const updateMods = (e: KeyboardEvent) => {
      setLiveMods({
        meta: e.metaKey,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
      });
    };

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      updateMods(e);

      if (e.key === "Escape") {
        setRecording(false);
        return;
      }

      if (["Control", "Shift", "Alt", "Meta", "OS"].includes(e.key)) return;

      const token = codeToToken(e.code);
      if (!token) return;

      const parts: string[] = [];
      if (e.metaKey) parts.push("super");
      if (e.ctrlKey) parts.push("ctrl");
      if (e.altKey) parts.push("alt");
      if (e.shiftKey) parts.push("shift");
      parts.push(token);

      if (parts.length < 2) return;

      onChange(parts.join("+"));
      setRecording(false);
    };

    const onKeyUp = (e: KeyboardEvent) => updateMods(e);

    const onClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setRecording(false);
      }
    };

    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("mousedown", onClickOutside, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      window.removeEventListener("mousedown", onClickOutside, true);
    };
  }, [recording, onChange]);

  useEffect(() => {
    if (!recording) {
      setLiveMods(EMPTY_MODS);
      setShowExamples(false);
    }
  }, [recording]);

  const applyExample = (combo: string) => {
    onChange(combo);
    setRecording(false);
  };

  const tokens = displayTokens(value);
  const liveTokens = liveModifierSymbols(liveMods);

  return (
    <div className="flex flex-col items-end gap-1">
      <div
        ref={containerRef}
        className="relative"
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        <button
          id={id}
          type="button"
          onClick={() => setRecording((v) => !v)}
          className={
            "flex h-7 min-w-[110px] items-center justify-center gap-1.5 rounded-full border px-3 text-xs transition-colors " +
            (recording
              ? "border-[var(--accent)] bg-[var(--panel-bg)] text-[var(--accent)]"
              : value
                ? "border-[var(--border-subtle)] bg-[var(--panel-bg)] text-[var(--text-primary)] hover:bg-[var(--hover-bg)]"
                : "border-[var(--border-subtle)] bg-[var(--panel-bg)] text-[var(--text-tertiary)] hover:bg-[var(--hover-bg)]")
          }
        >
          {tokens.length > 0 ? (
            tokens.map((t, i) => (
              <span key={i} className="font-mono text-[11px]">
                {t}
              </span>
            ))
          ) : (
            <span>Not set</span>
          )}
        </button>

        {value && !recording && hovered && (
          <button
            type="button"
            onClick={() => onChange("")}
            className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-[var(--border-primary)] bg-[var(--panel-bg)] text-[var(--text-tertiary)] shadow-sm hover:text-[var(--delete-hover-text)]"
            title="Clear shortcut"
          >
            <X size={10} />
          </button>
        )}

        {recording && (
          <div
            className="absolute right-0 top-full z-50 mt-2 flex min-w-[240px] flex-col items-center gap-2 rounded-xl border border-[var(--border-primary)] bg-[var(--surface-elevated)] px-4 py-3 shadow-lg"
            role="dialog"
            aria-label="Shortcut recorder"
          >
            <div className="flex min-h-[28px] items-center gap-2">
              {liveTokens.length === 0 ? (
                <span className="text-[11px] text-[var(--text-tertiary)]">Hold modifier keys…</span>
              ) : (
                liveTokens.map((t, i) => (
                  <kbd
                    key={i}
                    className="flex h-7 min-w-[28px] items-center justify-center rounded-md border border-[var(--border-primary)] bg-[var(--hover-bg)] font-mono text-xs text-[var(--text-primary)]"
                  >
                    {t}
                  </kbd>
                ))
              )}
            </div>
            <div className="text-xs font-medium text-[var(--accent)]">Recording…</div>
            <div className="text-[10px] text-[var(--text-tertiary)]">Esc to cancel</div>

            {examples.length > 0 && (
              <>
                <div className="my-1 h-px w-full bg-[var(--border-subtle)]" />
                <button
                  type="button"
                  onClick={(e) => {
                    // stop outside-click handler on the same tick
                    e.stopPropagation();
                    setShowExamples((v) => !v);
                  }}
                  className="flex h-6 items-center gap-1 rounded-full border border-[var(--border-primary)] px-3 text-[11px] text-[var(--text-secondary)] hover:bg-[var(--hover-bg)]"
                >
                  {showExamples ? "Hide examples" : "Show examples"}
                  {showExamples ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                </button>

                {showExamples && (
                  <div className="mt-1 flex max-h-[200px] w-full flex-col gap-1 overflow-y-auto pr-1">
                    {examples.map((combo) => {
                      const parts = displayTokens(combo);
                      return (
                        <button
                          key={combo}
                          type="button"
                          onMouseDown={(e) => {
                            // Prevent outside-click from firing first
                            e.preventDefault();
                            e.stopPropagation();
                            applyExample(combo);
                          }}
                          className="flex w-full items-center justify-center gap-1.5 rounded-md border border-transparent px-2 py-1 hover:border-[var(--border-subtle)] hover:bg-[var(--row-hover)]"
                        >
                          {parts.map((p, i) => (
                            <ExampleToken key={i} token={p} />
                          ))}
                        </button>
                      );
                    })}
                  </div>
                )}
              </>
            )}
          </div>
        )}
      </div>

      {conflictsWithReserved && !recording && (
        <div className="flex items-center gap-1 text-[10px] text-[var(--delete-hover-text)]">
          <AlertTriangle size={10} />
          <span>Conflicts with a macOS system shortcut</span>
        </div>
      )}
    </div>
  );
}
