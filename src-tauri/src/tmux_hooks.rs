//! Installs tmux global hooks that call `agentoast dismiss` so notifications
//! clear when the user navigates to a pane via tmux directly. Runs lazily on
//! the first successful session refresh — the tmux server may not be up when
//! agentoast launches, so we retry until a hook registration attempt succeeds.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::terminal::find_tmux;

static INSTALLED: AtomicBool = AtomicBool::new(false);

// Marker matched against `tmux show-hooks -g` output to detect a prior install
// and avoid appending duplicate hook entries across app restarts.
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

    let show = match Command::new(&tmux).args(["show-hooks", "-g"]).output() {
        Ok(o) if o.status.success() => o,
        _ => {
            // tmux server not running yet / socket unreachable. Leave
            // INSTALLED unset so the next refresh attempt retries.
            return;
        }
    };
    let existing = String::from_utf8_lossy(&show.stdout);

    let bin_str = agentoast_bin.to_string_lossy();
    if bin_str.contains('\'') {
        // Path can't be safely single-quoted; mark as installed to stop
        // retrying indefinitely. User can symlink to a cleaner path.
        log::warn!(
            "tmux_hooks: agentoast path contains single quote, skipping install: {}",
            bin_str
        );
        INSTALLED.store(true, Ordering::Relaxed);
        return;
    }
    let payload = format!("run-shell -b '{} dismiss -t \"#{{pane_id}}\"'", bin_str);

    let mut all_ok = true;
    for event in HOOK_EVENTS {
        // Per-event idempotency: only skip if THIS specific event already has
        // an agentoast hook. A whole-output marker check breaks when one
        // event was wiped (e.g. `set-hook -gu`) but others remain.
        let prefix = format!("{}[", event);
        let already = existing
            .lines()
            .any(|line| line.starts_with(&prefix) && line.contains(HOOK_MARKER));
        if already {
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
