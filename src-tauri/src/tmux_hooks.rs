//! Installs tmux global hooks that call `agentoast dismiss` so notifications
//! clear when the user navigates to a pane via tmux directly. Runs lazily on
//! the first successful session refresh — the tmux server may not be up when
//! agentoast launches, so we retry until a hook registration attempt succeeds.
//!
//! Self-healing: if the user moves or replaces `Agentoast.app`, an old hook
//! pointing at the previous binary path stays registered with the tmux server
//! and produces `exit 127` on every pane switch. On install we detect such
//! stale entries (path differs from the current binary) and remove them via
//! `set-hook -gu '<event>[<idx>]'`. The hook payload also wraps the call in
//! `[ -x "<bin>" ] && ... || true` so a stale path or a transient dismiss
//! failure (SQLite WAL contention with the running app) silently no-ops
//! instead of producing `'... dismiss -t ...' returned N` lines in the tmux
//! status bar. dismiss is best-effort, so swallowing exit codes here is safe.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::terminal::find_tmux;

static INSTALLED: AtomicBool = AtomicBool::new(false);

const HOOK_MARKER: &str = "agentoast dismiss";

const HOOK_EVENTS: &[&str] = &[
    "after-select-pane",
    "after-select-window",
    "client-attached",
];

pub fn install() {
    if INSTALLED.load(Ordering::Relaxed) {
        return;
    }

    let Some(tmux) = find_tmux() else {
        return;
    };
    let Ok(agentoast_bin) = std::env::current_exe() else {
        log::debug!("tmux_hooks: current_exe unavailable, skipping install");
        return;
    };

    let bin_str = agentoast_bin.to_string_lossy().to_string();
    if bin_str.contains('\'') || bin_str.contains('"') {
        // Path can't be safely shell-quoted; mark as installed to stop
        // retrying indefinitely. User can symlink to a cleaner path.
        log::warn!(
            "tmux_hooks: agentoast path contains quote chars, skipping install: {}",
            bin_str
        );
        INSTALLED.store(true, Ordering::Relaxed);
        return;
    }

    let show = match Command::new(&tmux).args(["show-hooks", "-g"]).output() {
        Ok(o) if o.status.success() => o,
        _ => {
            // tmux server not running yet / socket unreachable. Leave
            // INSTALLED unset so the next refresh attempt retries.
            return;
        }
    };
    let existing = String::from_utf8_lossy(&show.stdout);

    // First pass: collect stale entries (matching marker but pointing at a
    // different binary path) and detect events that already have a correct
    // hook so we can skip the re-add.
    let mut stale: Vec<(String, u32)> = Vec::new(); // (event, idx) to unset
    let mut already_correct: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();

    for line in existing.lines() {
        if !line.contains(HOOK_MARKER) {
            continue;
        }
        let Some(parsed) = parse_hook_line(line, HOOK_EVENTS) else {
            continue;
        };
        if parsed.bin == bin_str && parsed.has_or_true {
            already_correct.insert(parsed.event);
        } else {
            log::info!(
                "tmux_hooks: stale {}[{}] -> {} (current: {}, has_or_true: {})",
                parsed.event,
                parsed.idx,
                parsed.bin,
                bin_str,
                parsed.has_or_true,
            );
            stale.push((parsed.event.to_string(), parsed.idx));
        }
    }

    // Remove stale entries highest-index-first per event so the indices of
    // the remaining hooks don't shift under us.
    stale.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    for (event, idx) in &stale {
        let target = format!("{}[{}]", event, idx);
        match Command::new(&tmux)
            .args(["set-hook", "-gu", &target])
            .output()
        {
            Ok(o) if o.status.success() => {
                log::info!("tmux_hooks: removed stale {}", target);
            }
            Ok(o) => {
                log::warn!(
                    "tmux_hooks: unset {} failed: {}",
                    target,
                    String::from_utf8_lossy(&o.stderr).trim()
                );
            }
            Err(e) => {
                log::warn!("tmux_hooks: unset {} spawn failed: {}", target, e);
            }
        }
    }

    // Self-healing payload: if the bin disappears (uninstall, .app moved),
    // the `[ -x ... ]` test fails and the `|| true` tail keeps the overall
    // exit at 0 so tmux doesn't print `... returned 1` in the status bar.
    // Same suppression covers transient dismiss failures (e.g. SQLITE_BUSY
    // when the app's watcher holds a reader).
    let payload = format!(
        "run-shell -b '[ -x \"{0}\" ] && \"{0}\" dismiss -t \"#{{pane_id}}\" || true'",
        bin_str
    );

    let mut all_ok = true;
    for event in HOOK_EVENTS {
        if already_correct.contains(event) {
            continue;
        }
        match Command::new(&tmux)
            .args(["set-hook", "-ag", event, &payload])
            .output()
        {
            Ok(o) if o.status.success() => {
                log::info!("tmux_hooks: installed {} hook", event);
            }
            Ok(o) => {
                log::warn!(
                    "tmux_hooks: set-hook {} failed: {}",
                    event,
                    String::from_utf8_lossy(&o.stderr).trim()
                );
                all_ok = false;
            }
            Err(e) => {
                log::warn!("tmux_hooks: set-hook {} spawn failed: {}", event, e);
                all_ok = false;
            }
        }
    }

    if all_ok {
        INSTALLED.store(true, Ordering::Relaxed);
    }
}

struct ParsedHook {
    event: &'static str,
    idx: u32,
    bin: String,
    has_or_true: bool,
}

/// Parse a single `tmux show-hooks -g` line that looks like an `agentoast
/// dismiss` invocation. Returns `None` if the line doesn't target one of the
/// given events or can't be parsed.
///
/// Handles three payload shapes (oldest first):
///   v1: `run-shell -b "<bin> dismiss -t ..."`
///   v2: `run-shell -b "[ -x \"<bin>\" ] && \"<bin>\" dismiss -t ..."`
///   v3: `run-shell -b "[ -x \"<bin>\" ] && \"<bin>\" dismiss -t ... || true"`
///
/// `has_or_true` is true only for v3. v1/v2 are flagged so the caller can
/// re-install with the current payload format.
fn parse_hook_line(line: &str, events: &[&'static str]) -> Option<ParsedHook> {
    let event = events
        .iter()
        .copied()
        .find(|e| line.starts_with(&format!("{}[", e)))?;
    let after_event = &line[event.len() + 1..]; // skip `<event>[`
    let bracket_end = after_event.find(']')?;
    let idx: u32 = after_event[..bracket_end].parse().ok()?;

    // Bin path lives just before " dismiss". In v2/v3 it is the second
    // occurrence of \" ... \"; in v1 the path is bare between `-b "` and
    // ` dismiss`.
    let dismiss_idx = line.find(" dismiss")?;
    let prefix = &line[..dismiss_idx];
    let bin = if let Some(trimmed) = prefix.strip_suffix("\\\"") {
        let start = trimmed.rfind("\\\"")?;
        trimmed[start + 2..].to_string()
    } else {
        let payload_start = prefix.rfind("-b \"")?;
        prefix[payload_start + "-b \"".len()..].to_string()
    };

    // v3 marker: trailing ` || true` inside the run-shell payload. tmux
    // doesn't escape `|` or whitespace so a substring check is sufficient.
    let has_or_true = line[dismiss_idx..].contains("|| true");

    Some(ParsedHook {
        event,
        idx,
        bin,
        has_or_true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTS: &[&str] = &[
        "after-select-pane",
        "after-select-window",
        "client-attached",
    ];

    #[test]
    fn parses_v1_bare_bin_payload() {
        let line = "after-select-pane[0] run-shell -b \"/old/path/agentoast dismiss -t \\\"#{pane_id}\\\"\"";
        let p = parse_hook_line(line, EVENTS).expect("parse");
        assert_eq!(p.event, "after-select-pane");
        assert_eq!(p.idx, 0);
        assert_eq!(p.bin, "/old/path/agentoast");
        assert!(!p.has_or_true);
    }

    #[test]
    fn parses_v2_existence_check_without_or_true() {
        let line = "client-attached[2] run-shell -b \"[ -x \\\"/new/path/agentoast\\\" ] && \\\"/new/path/agentoast\\\" dismiss -t \\\"#{pane_id}\\\"\"";
        let p = parse_hook_line(line, EVENTS).expect("parse");
        assert_eq!(p.event, "client-attached");
        assert_eq!(p.idx, 2);
        assert_eq!(p.bin, "/new/path/agentoast");
        assert!(!p.has_or_true);
    }

    #[test]
    fn parses_v3_existence_check_with_or_true() {
        let line = "after-select-window[1] run-shell -b \"[ -x \\\"/p/agentoast\\\" ] && \\\"/p/agentoast\\\" dismiss -t \\\"#{pane_id}\\\" || true\"";
        let p = parse_hook_line(line, EVENTS).expect("parse");
        assert_eq!(p.event, "after-select-window");
        assert_eq!(p.idx, 1);
        assert_eq!(p.bin, "/p/agentoast");
        assert!(p.has_or_true);
    }

    #[test]
    fn ignores_unrelated_events() {
        let line = "after-bind-key[0] run-shell -b \"/x/agentoast dismiss -t \\\"#{pane_id}\\\"\"";
        assert!(parse_hook_line(line, EVENTS).is_none());
    }
}
