//! tmux helpers shared between the CLI and the GUI.
//!
//! `find_tmux` resolves the tmux binary with a single lookup order used by
//! both the CLI and the Tauri app: the `[system] tmux` config override first,
//! then well-known install paths, then a `$PATH` scan. The override is passed
//! in by the caller so this module stays independent of config loading — the
//! GUI caches its `AppConfig` (hot path) while the CLI reads it once per run.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve the tmux binary path.
///
/// Priority: `tmux_override` (from `config.toml` `[system] tmux`) → well-known
/// paths (Homebrew / system / Nix) → `$PATH` scan. Returns `None` when not found.
pub fn find_tmux(tmux_override: Option<&str>) -> Option<PathBuf> {
    // config.toml override (highest priority)
    if let Some(path) = tmux_override {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
        log::warn!(
            "config.toml system.tmux={} not found, falling back to auto-detection",
            path
        );
    }

    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/opt/homebrew/bin/tmux"), // Homebrew (Apple Silicon)
        PathBuf::from("/usr/local/bin/tmux"),    // Homebrew (Intel) / manual
        PathBuf::from("/usr/bin/tmux"),          // system
    ];

    // Nix Home Manager: /etc/profiles/per-user/<username>/bin/tmux
    if let Ok(user) = std::env::var("USER") {
        candidates.push(PathBuf::from(format!(
            "/etc/profiles/per-user/{}/bin/tmux",
            user
        )));
    }
    // Nix single-user profile
    candidates.push(PathBuf::from("/nix/var/nix/profiles/default/bin/tmux"));

    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        return Some(found.clone());
    }

    // PATH-based fallback (mise, asdf, custom installs, etc.)
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join("tmux");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Resolve a tmux pane's pid (e.g. for `%72`), or `None` if it doesn't exist.
///
/// This doubles as an existence check: `display-message` exits 0 even for a
/// bogus target (tmux evaluates the format against the current client and
/// prints an empty `#{pane_pid}`), but an empty / unparseable pid yields `None`
/// — so a real pane is the only way to get `Some`.
pub fn pane_pid(tmux: &Path, pane: &str) -> Option<u32> {
    let out = Command::new(tmux)
        .args(["display-message", "-t", pane, "-p", "#{pane_pid}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Inject `body` into the target pane via a bracketed-paste sequence, then
/// optionally submit it with Enter.
///
/// We deliberately avoid `tmux send-keys -l <body>` followed by a separate
/// `send-keys Enter`: on some agent TUIs (notably Codex's crossterm-based
/// input loop) the trailing Enter arrives within the body burst and is
/// absorbed as paste content, so the message lands in the input box but
/// never submits. The failure is intermittent — it depends on tmux server
/// scheduling and the receiver's paste-detection heuristics.
///
/// Instead we stage the body in a uniquely-named tmux paste buffer and
/// dispatch it with `paste-buffer -p`, which wraps the bytes in bracketed-
/// paste markers (`ESC [200~ … ESC [201~`). Receivers that opt into
/// bracketed paste mode (every modern agent TUI does) see an explicit
/// paste-end marker before our subsequent Enter, so the Enter is
/// unambiguously a key press and not part of the paste — eliminating the
/// race at the byte-stream level regardless of how the TUI schedules reads.
/// `-d` deletes the buffer after pasting so we don't leak names.
pub fn send_keys(tmux: &Path, pane: &str, body: &str, enter: bool) -> Result<(), String> {
    let buf_name = format!("agentoast-send-{}", std::process::id());

    let out = Command::new(tmux)
        .args(["set-buffer", "-b", &buf_name, "--", body])
        .output()
        .map_err(|e| format!("failed to run tmux set-buffer: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    let out = Command::new(tmux)
        .args(["paste-buffer", "-t", pane, "-b", &buf_name, "-p", "-d"])
        .output()
        .map_err(|e| format!("failed to run tmux paste-buffer: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    if enter {
        let out = Command::new(tmux)
            .args(["send-keys", "-t", pane, "Enter"])
            .output()
            .map_err(|e| format!("failed to run tmux send-keys Enter: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
    }

    Ok(())
}
