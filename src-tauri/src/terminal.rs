use std::path::PathBuf;
use std::process::Command;

const KNOWN_TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.github.wez.wezterm",
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "com.apple.Terminal",
    "org.alacritty",
    "net.kovidgoyal.kitty",
];

pub(crate) fn find_tmux() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/tmux", // Homebrew (Apple Silicon)
        "/usr/local/bin/tmux",    // Homebrew (Intel) / manual
        "/usr/bin/tmux",          // system
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

pub(crate) fn find_git() -> Option<PathBuf> {
    let candidates = [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

fn switch_tmux_pane(tmux_pane: &str) -> Result<(), String> {
    let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    // Switch the attached session to the one containing the target pane
    Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["switch-client", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("tmux switch-client failed: {}", e))?;

    Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["select-window", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("tmux select-window failed: {}", e))?;

    Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["select-pane", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("tmux select-pane failed: {}", e))?;

    Ok(())
}

fn activate_terminal(bundle_id: &str) -> Result<(), String> {
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
        log::info!("is_terminal_focused: bundle_id is empty, returning false");
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
                log::info!(
                    "is_terminal_focused: bundle_id={} active={}",
                    bundle_id,
                    active
                );
                return active;
            }
        }
    }

    log::info!(
        "is_terminal_focused: bundle_id={} not found in running apps",
        bundle_id
    );
    false
}

/// Check if the given tmux pane is the active visible pane.
/// Returns true when pane_active=1, window_active=1, session_attached=1.
fn is_tmux_pane_active(tmux_pane: &str) -> bool {
    if tmux_pane.is_empty() {
        log::info!("is_tmux_pane_active: tmux_pane is empty, returning false");
        return false;
    }

    let tmux_path = match find_tmux() {
        Some(p) => p,
        None => {
            log::info!("is_tmux_pane_active: tmux not found, returning false");
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
            log::info!(
                "is_tmux_pane_active: tmux command failed for pane={}: {}",
                tmux_pane,
                e
            );
            return false;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::info!(
            "is_tmux_pane_active: tmux exited with error for pane={}: {}",
            tmux_pane,
            stderr.trim()
        );
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();
    let active = result == "1 1 1";
    log::info!(
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
        log::info!(
            "is_pane_visible_to_user: skipped (bundle_id='{}', pane='{}')",
            terminal_bundle_id,
            tmux_pane
        );
        return false;
    }

    let terminal_focused = is_terminal_focused(terminal_bundle_id);
    if !terminal_focused {
        log::info!(
            "is_pane_visible_to_user: terminal not focused (bundle_id='{}'), returning false",
            terminal_bundle_id
        );
        return false;
    }

    let pane_active = is_tmux_pane_active(tmux_pane);
    log::info!(
        "is_pane_visible_to_user: terminal focused, pane_active={} (pane='{}')",
        pane_active,
        tmux_pane
    );
    pane_active
}

fn activate_any_terminal() -> Result<(), String> {
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
