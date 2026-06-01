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

/// Check that a tmux pane with the given id (e.g. `%72`) currently exists.
pub fn pane_exists(tmux: &Path, pane: &str) -> bool {
    Command::new(tmux)
        .args(["display-message", "-t", pane, "-p", "#{pane_id}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Inject `body` into the target pane as literal keystrokes, then optionally
/// submit it with Enter.
///
/// `-l` sends the text literally (no key-name interpretation) and `--`
/// terminates option parsing, so a body starting with `-` is delivered safely.
pub fn send_keys(tmux: &Path, pane: &str, body: &str, enter: bool) -> Result<(), String> {
    let out = Command::new(tmux)
        .args(["send-keys", "-t", pane, "-l", "--", body])
        .output()
        .map_err(|e| format!("failed to run tmux send-keys: {e}"))?;
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
