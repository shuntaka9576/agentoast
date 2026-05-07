import { useCallback, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Pencil, Search, Trash2, X } from "lucide-react";
import { fuzzyScore } from "@/lib/fuzzy";
import type { AllowedApp, RunningApp } from "@/lib/types";

interface AppsSetupProps {
  allowedApps: AllowedApp[];
  onChange: (next: AllowedApp[]) => void;
}

type Status = { kind: "idle" } | { kind: "loading" } | { kind: "error"; message: string };

interface Candidate {
  bundleId: string;
  name: string;
  iconDataUrl?: string;
  running: boolean;
}

export function AppsSetup({ allowedApps, onChange }: AppsSetupProps) {
  const [running, setRunning] = useState<RunningApp[]>([]);
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const [query, setQuery] = useState("");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [editingBundleId, setEditingBundleId] = useState<string | null>(null);
  const [editingDraft, setEditingDraft] = useState("");
  const editInputRef = useRef<HTMLInputElement | null>(null);
  // `list_running_apps` enumerates every running macOS app and base64-encodes
  // each icon — ~2s on a busy desktop. Defer it until the user actually
  // interacts with the search input so cold start (where the settings webview
  // is pre-loaded but invisible) doesn't pay this cost.
  const fetchedRef = useRef(false);

  const ensureRunningLoaded = useCallback(() => {
    if (fetchedRef.current) return;
    fetchedRef.current = true;
    setStatus({ kind: "loading" });
    invoke<RunningApp[]>("list_running_apps")
      .then((apps) => {
        setRunning(apps);
        setStatus({ kind: "idle" });
      })
      .catch((err) => {
        fetchedRef.current = false;
        setStatus({ kind: "error", message: String(err) });
      });
  }, []);

  const allowedSet = useMemo(() => new Set(allowedApps.map((a) => a.bundleId)), [allowedApps]);

  // Map of bundleId → running RunningApp (for pinned-list icon lookup).
  const runningByBundle = useMemo(() => {
    const m = new Map<string, RunningApp>();
    for (const a of running) m.set(a.bundleId, a);
    return m;
  }, [running]);

  const candidates = useMemo<Candidate[]>(() => {
    // Only running apps go into search results — already-pinned apps are
    // hidden so the dropdown stays focused on net-new additions.
    const out: Candidate[] = [];
    for (const a of running) {
      if (allowedSet.has(a.bundleId)) continue;
      out.push({
        bundleId: a.bundleId,
        name: a.name,
        iconDataUrl: a.iconDataUrl,
        running: true,
      });
    }
    return out;
  }, [running, allowedSet]);

  const results = useMemo<Candidate[]>(() => {
    const q = query.trim();
    if (!q) return [];
    const scored: { c: Candidate; score: number }[] = [];
    for (const c of candidates) {
      const nameScore = fuzzyScore(q, c.name);
      const idScore = fuzzyScore(q, c.bundleId);
      const best =
        nameScore !== null && idScore !== null
          ? Math.min(nameScore, idScore)
          : (nameScore ?? idScore);
      if (best !== null) scored.push({ c, score: best });
    }
    scored.sort((a, b) => a.score - b.score);
    return scored.slice(0, 30).map((s) => s.c);
  }, [candidates, query]);

  const trimmedQuery = query.trim();
  // Show "Pin as bundle ID" fallback when the input looks like a bundle ID
  // (contains a dot) and isn't already in the allowlist or the running list.
  const showRawFallback =
    trimmedQuery.length > 0 &&
    trimmedQuery.includes(".") &&
    !allowedSet.has(trimmedQuery) &&
    !runningByBundle.has(trimmedQuery);

  const addApp = (bundleId: string, displayName: string) => {
    if (allowedSet.has(bundleId)) return;
    onChange([...allowedApps, { bundleId, displayName: displayName || bundleId }]);
    setQuery("");
    setDropdownOpen(false);
  };

  const removeApp = (bundleId: string) => {
    onChange(allowedApps.filter((a) => a.bundleId !== bundleId));
    if (editingBundleId === bundleId) {
      setEditingBundleId(null);
      setEditingDraft("");
    }
  };

  const beginEdit = (app: AllowedApp) => {
    setEditingBundleId(app.bundleId);
    setEditingDraft(app.displayName);
    // Defer focus until the input mounts.
    requestAnimationFrame(() => {
      editInputRef.current?.focus();
      editInputRef.current?.select();
    });
  };

  const cancelEdit = () => {
    setEditingBundleId(null);
    setEditingDraft("");
  };

  const commitEdit = () => {
    if (!editingBundleId) return;
    const draft = editingDraft.trim();
    onChange(
      allowedApps.map((a) =>
        a.bundleId === editingBundleId ? { ...a, displayName: draft || a.bundleId } : a,
      ),
    );
    setEditingBundleId(null);
    setEditingDraft("");
  };

  return (
    <div className="rounded-xl border border-[var(--border-subtle)] p-4">
      <div className="mb-3 flex flex-col gap-0.5">
        <span className="text-sm font-semibold text-[var(--text-primary)]">Pinned apps</span>
        <span className="text-[11px] text-[var(--text-tertiary)]">
          Search by app name or paste a bundle ID. Pinned apps appear in the panel’s Apps view
          (press{" "}
          <kbd className="rounded border border-[var(--border-subtle)] px-1 text-[10px]">a</kbd>).
        </span>
      </div>

      <div className="relative mb-3">
        <Search
          size={12}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-tertiary)]"
        />
        <input
          type="text"
          spellCheck={false}
          placeholder="Search apps or paste a bundle ID…"
          value={query}
          onChange={(e) => {
            ensureRunningLoaded();
            setQuery(e.target.value);
          }}
          onFocus={() => {
            ensureRunningLoaded();
            setDropdownOpen(true);
          }}
          onBlur={() => {
            // Delay so a mousedown on a dropdown item still registers.
            window.setTimeout(() => setDropdownOpen(false), 120);
          }}
          className="h-7 w-full rounded-md border border-[var(--border-primary)] bg-[var(--panel-bg)] pl-7 pr-7 text-xs text-[var(--text-primary)] outline-none focus:border-[var(--accent)]"
        />
        {query ? (
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setQuery("")}
            aria-label="Clear search"
            className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-[var(--text-tertiary)] hover:bg-[var(--hover-bg)] hover:text-[var(--text-primary)]"
          >
            <X size={11} />
          </button>
        ) : (
          status.kind === "loading" && (
            <span
              aria-label="Loading running apps"
              className="pointer-events-none absolute right-2 top-1/2 inline-block h-3 w-3 -translate-y-1/2 animate-spin rounded-full border border-[var(--border-primary)] border-t-[var(--accent)]"
            />
          )
        )}

        {dropdownOpen && trimmedQuery && (
          <div className="absolute left-0 right-0 top-full z-10 mt-1 max-h-[260px] overflow-y-auto rounded-md border border-[var(--border-primary)] bg-[var(--panel-bg)] shadow-lg">
            {status.kind === "loading" && results.length === 0 && (
              <div className="flex items-center gap-2 px-3 py-2 text-[11px] text-[var(--text-tertiary)]">
                <span className="inline-block h-2.5 w-2.5 animate-spin rounded-full border border-[var(--border-primary)] border-t-[var(--accent)]" />
                Loading running apps…
              </div>
            )}
            {status.kind !== "loading" && results.length === 0 && !showRawFallback && (
              <div className="px-3 py-2 text-[11px] text-[var(--text-tertiary)]">
                No matching apps.
              </div>
            )}
            {results.map((c) => (
              <button
                key={c.bundleId}
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => addApp(c.bundleId, c.name)}
                className="flex w-full items-center gap-2.5 px-2.5 py-1.5 text-left transition-colors hover:bg-[var(--hover-bg)]"
              >
                {c.iconDataUrl ? (
                  <img src={c.iconDataUrl} alt="" width={20} height={20} className="rounded" />
                ) : (
                  <div className="flex h-5 w-5 items-center justify-center rounded bg-[var(--hover-bg-strong)] text-[10px] text-[var(--text-tertiary)]">
                    {c.name.slice(0, 1).toUpperCase()}
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate text-xs text-[var(--text-primary)]">{c.name}</div>
                  <div className="truncate font-mono text-[10px] text-[var(--text-tertiary)]">
                    {c.bundleId}
                  </div>
                </div>
              </button>
            ))}
            {showRawFallback && (
              <button
                type="button"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => addApp(trimmedQuery, trimmedQuery)}
                className="flex w-full items-center gap-2.5 border-t border-[var(--border-subtle)] px-2.5 py-1.5 text-left transition-colors hover:bg-[var(--hover-bg)]"
              >
                <div className="flex h-5 w-5 items-center justify-center rounded bg-[var(--hover-bg-strong)] text-[10px] text-[var(--text-tertiary)]">
                  +
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-xs text-[var(--text-primary)]">
                    Pin "<span className="font-mono">{trimmedQuery}</span>" as bundle ID
                  </div>
                  <div className="text-[10px] text-[var(--text-tertiary)]">
                    The app does not need to be running.
                  </div>
                </div>
              </button>
            )}
          </div>
        )}
      </div>

      {status.kind === "loading" && (
        <p className="text-[11px] text-[var(--text-tertiary)]">Loading running apps…</p>
      )}
      {status.kind === "error" && (
        <p className="text-[11px] text-[var(--delete-hover-text)]">
          Failed to enumerate running apps: {status.message}
        </p>
      )}

      {allowedApps.length === 0 ? (
        <p className="text-[11px] text-[var(--text-tertiary)]">No apps pinned yet.</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {allowedApps.map((app) => {
            const ra = runningByBundle.get(app.bundleId);
            const isEditing = editingBundleId === app.bundleId;
            return (
              <li
                key={app.bundleId}
                className="flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-[var(--hover-bg)]"
              >
                {ra?.iconDataUrl ? (
                  <img src={ra.iconDataUrl} alt="" width={24} height={24} className="rounded" />
                ) : (
                  <div className="flex h-6 w-6 items-center justify-center rounded bg-[var(--hover-bg-strong)] text-[11px] text-[var(--text-tertiary)]">
                    {(app.displayName || app.bundleId).slice(0, 1).toUpperCase()}
                  </div>
                )}

                <div className="min-w-0 flex-1">
                  {isEditing ? (
                    <input
                      ref={editInputRef}
                      type="text"
                      spellCheck={false}
                      value={editingDraft}
                      onChange={(e) => setEditingDraft(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.preventDefault();
                          commitEdit();
                        } else if (e.key === "Escape") {
                          e.preventDefault();
                          cancelEdit();
                        }
                      }}
                      className="h-6 w-full rounded border border-[var(--accent)] bg-[var(--panel-bg)] px-1.5 text-xs text-[var(--text-primary)] outline-none"
                    />
                  ) : (
                    <div className="truncate text-xs text-[var(--text-primary)]">
                      {app.displayName || app.bundleId}
                    </div>
                  )}
                  <div className="truncate font-mono text-[10px] text-[var(--text-tertiary)]">
                    {app.bundleId}
                    {!ra && " · not running"}
                  </div>
                </div>

                <div className="flex items-center gap-0.5">
                  {isEditing ? (
                    <>
                      <button
                        type="button"
                        onClick={commitEdit}
                        aria-label="Save"
                        className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--hover-bg-strong)] hover:text-[var(--text-primary)]"
                      >
                        <Check size={12} />
                      </button>
                      <button
                        type="button"
                        onClick={cancelEdit}
                        aria-label="Cancel"
                        className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--hover-bg-strong)] hover:text-[var(--text-primary)]"
                      >
                        <X size={12} />
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        type="button"
                        onClick={() => beginEdit(app)}
                        aria-label={`Edit name for ${app.displayName || app.bundleId}`}
                        className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--hover-bg-strong)] hover:text-[var(--text-primary)]"
                      >
                        <Pencil size={12} />
                      </button>
                      <button
                        type="button"
                        onClick={() => removeApp(app.bundleId)}
                        aria-label={`Remove ${app.displayName || app.bundleId}`}
                        className="rounded p-1 text-[var(--text-tertiary)] hover:bg-[var(--hover-bg-strong)] hover:text-[var(--delete-hover-text)]"
                      >
                        <Trash2 size={12} />
                      </button>
                    </>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
