use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use agentoast_shared::config::{self, AppConfig};

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

fn get_config() -> &'static AppConfig {
    CONFIG.get_or_init(config::load_config)
}

const KNOWN_TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.github.wez.wezterm",
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "com.apple.Terminal",
    "org.alacritty",
    "net.kovidgoyal.kitty",
];

pub(crate) fn find_tmux() -> Option<PathBuf> {
    // config.toml override (highest priority)
    if let Some(ref path) = get_config().system.tmux {
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

pub(crate) fn find_git() -> Option<PathBuf> {
    // config.toml override (highest priority)
    if let Some(ref path) = get_config().system.git {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
        log::warn!(
            "config.toml system.git={} not found, falling back to auto-detection",
            path
        );
    }

    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/usr/bin/git"),          // system (Xcode CLT)
        PathBuf::from("/opt/homebrew/bin/git"), // Homebrew (Apple Silicon)
        PathBuf::from("/usr/local/bin/git"),    // Homebrew (Intel) / manual
    ];

    // Nix Home Manager
    if let Ok(user) = std::env::var("USER") {
        candidates.push(PathBuf::from(format!(
            "/etc/profiles/per-user/{}/bin/git",
            user
        )));
    }
    // Nix single-user profile
    candidates.push(PathBuf::from("/nix/var/nix/profiles/default/bin/git"));

    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        return Some(found.clone());
    }

    // PATH-based fallback
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join("git");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Resolve a pane id (%NN) to a concrete `session:window.pane` target by
/// asking tmux. Returns `None` when the pane no longer exists, which lets the
/// caller decide to fall back to the raw pane id or report failure upstream.
fn resolve_pane_target(tmux_path: &std::path::Path, pane_id: &str) -> Option<(String, String)> {
    let output = Command::new(tmux_path)
        .env_remove("TMPDIR")
        .args([
            "display-message",
            "-t",
            pane_id,
            "-p",
            "#{session_name}\t#{session_name}:#{window_index}.#{pane_index}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!(
            "switch_tmux_pane: display-message failed for pane={} exit={:?} stderr={}",
            pane_id,
            output.status.code(),
            stderr.trim()
        );
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    let mut parts = line.splitn(2, '\t');
    let session = parts.next()?.trim().to_string();
    let target = parts.next()?.trim().to_string();
    if session.is_empty() || target.is_empty() {
        return None;
    }
    Some((session, target))
}

fn run_tmux_subcmd(tmux_path: &std::path::Path, subcmd: &str, target: &str) -> Result<(), String> {
    let output = Command::new(tmux_path)
        .env_remove("TMPDIR")
        .args([subcmd, "-t", target])
        .output()
        .map_err(|e| format!("tmux {} failed to spawn: {}", subcmd, e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!(
            "tmux {} -t {} exit={:?} stderr={}",
            subcmd,
            target,
            output.status.code(),
            stderr.trim()
        );
        log::warn!("switch_tmux_pane: {}", msg);
        return Err(msg);
    }
    Ok(())
}

pub(crate) fn switch_tmux_pane(tmux_pane: &str) -> Result<(), String> {
    let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    // Resolve pane-id to session+window+pane so switch-client gets a valid
    // target-session (its -t flag expects a session, not a pane id).
    let (session_target, full_target) = match resolve_pane_target(&tmux_path, tmux_pane) {
        Some(pair) => pair,
        None => {
            // Pane likely vanished; fall back to the raw id so at least
            // select-pane has a chance to succeed. The three sub-calls
            // below will surface real errors via their exit status.
            (tmux_pane.to_string(), tmux_pane.to_string())
        }
    };

    run_tmux_subcmd(&tmux_path, "switch-client", &session_target)?;
    run_tmux_subcmd(&tmux_path, "select-window", &full_target)?;
    run_tmux_subcmd(&tmux_path, "select-pane", &full_target)?;

    log::info!(
        "switch_tmux_pane ok pane={} target={}",
        tmux_pane,
        full_target
    );
    Ok(())
}

pub(crate) fn activate_terminal(bundle_id: &str) -> Result<(), String> {
    if bundle_id.is_empty() {
        return Err("No terminal bundle ID specified".to_string());
    }

    use objc2_app_kit::{NSApplicationActivationOptions, NSWorkspace};
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    let target_ns = NSString::from_str(bundle_id);

    for app in &apps {
        if let Some(bid) = app.bundleIdentifier() {
            if bid.isEqualToString(&target_ns) {
                let activated =
                    app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
                if activated {
                    return Ok(());
                }
            }
        }
    }

    Err(format!("Terminal application not found: {}", bundle_id))
}

/// Check if a terminal with the given bundle ID is currently the active (focused) application.
fn is_terminal_focused(bundle_id: &str) -> bool {
    if bundle_id.is_empty() {
        log::debug!("is_terminal_focused: bundle_id is empty, returning false");
        return false;
    }

    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    let target_ns = NSString::from_str(bundle_id);

    for app in &apps {
        if let Some(bid) = app.bundleIdentifier() {
            if bid.isEqualToString(&target_ns) {
                let active = app.isActive();
                log::debug!(
                    "is_terminal_focused: bundle_id={} active={}",
                    bundle_id,
                    active
                );
                return active;
            }
        }
    }

    log::debug!(
        "is_terminal_focused: bundle_id={} not found in running apps",
        bundle_id
    );
    false
}

/// Check if the given tmux pane is the active visible pane.
/// Returns true when pane_active=1, window_active=1, session_attached=1.
fn is_tmux_pane_active(tmux_pane: &str) -> bool {
    if tmux_pane.is_empty() {
        log::debug!("is_tmux_pane_active: tmux_pane is empty, returning false");
        return false;
    }

    let tmux_path = match find_tmux() {
        Some(p) => p,
        None => {
            log::debug!("is_tmux_pane_active: tmux not found, returning false");
            return false;
        }
    };

    let output = match Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args([
            "display-message",
            "-t",
            tmux_pane,
            "-p",
            "#{pane_active} #{window_active} #{session_attached}",
        ])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::debug!(
                "is_tmux_pane_active: tmux command failed for pane={}: {}",
                tmux_pane,
                e
            );
            return false;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::debug!(
            "is_tmux_pane_active: tmux exited with error for pane={}: {}",
            tmux_pane,
            stderr.trim()
        );
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();
    let active = result == "1 1 1";
    log::debug!(
        "is_tmux_pane_active: pane={} result='{}' active={}",
        tmux_pane,
        result,
        active
    );

    active
}

/// Check if the notification's originating terminal pane is currently visible to the user.
/// Short-circuits: only checks tmux if the terminal app is focused first.
pub fn is_pane_visible_to_user(terminal_bundle_id: &str, tmux_pane: &str) -> bool {
    if terminal_bundle_id.is_empty() || tmux_pane.is_empty() {
        log::debug!(
            "is_pane_visible_to_user: skipped (bundle_id='{}', pane='{}')",
            terminal_bundle_id,
            tmux_pane
        );
        return false;
    }

    let terminal_focused = is_terminal_focused(terminal_bundle_id);
    if !terminal_focused {
        log::debug!(
            "is_pane_visible_to_user: terminal not focused (bundle_id='{}'), returning false",
            terminal_bundle_id
        );
        return false;
    }

    let pane_active = is_tmux_pane_active(tmux_pane);
    log::debug!(
        "is_pane_visible_to_user: terminal focused, pane_active={} (pane='{}')",
        pane_active,
        tmux_pane
    );
    pane_active
}

pub(crate) fn activate_any_terminal() -> Result<(), String> {
    for &bid in KNOWN_TERMINAL_BUNDLE_IDS {
        if activate_terminal(bid).is_ok() {
            return Ok(());
        }
    }
    Err("No known terminal application found".to_string())
}

pub fn focus_terminal(tmux_pane: &str, terminal_bundle_id: &str) -> Result<(), String> {
    // 1. Switch tmux pane if specified (failure is non-fatal)
    if !tmux_pane.is_empty() {
        if let Err(e) = switch_tmux_pane(tmux_pane) {
            log::debug!("tmux pane switch failed (non-fatal): {}", e);
        }
    }

    // 2. Activate terminal app (try all known terminals if bundle ID is empty)
    if terminal_bundle_id.is_empty() {
        activate_any_terminal()
    } else {
        activate_terminal(terminal_bundle_id)
    }
}
