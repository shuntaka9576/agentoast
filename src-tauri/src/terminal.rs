use std::path::PathBuf;
use std::process::Command;

fn find_tmux() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/tmux", // Homebrew (Apple Silicon)
        "/usr/local/bin/tmux",    // Homebrew (Intel) / manual
        "/usr/bin/tmux",          // system
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

fn switch_tmux_pane(tmux_pane: &str) -> Result<(), String> {
    let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    // Switch the attached session to the one containing the target pane
    Command::new(&tmux_path)
        .args(["switch-client", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("tmux switch-client failed: {}", e))?;

    Command::new(&tmux_path)
        .args(["select-window", "-t", tmux_pane])
        .output()
        .map_err(|e| format!("tmux select-window failed: {}", e))?;

    Command::new(&tmux_path)
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

    Err(format!(
        "Terminal application not found: {}",
        bundle_id
    ))
}

pub fn focus_terminal(tmux_pane: &str, terminal_bundle_id: &str) -> Result<(), String> {
    // 1. Switch tmux pane if specified (failure is non-fatal)
    if !tmux_pane.is_empty() {
        if let Err(e) = switch_tmux_pane(tmux_pane) {
            log::debug!("tmux pane switch failed (non-fatal): {}", e);
        }
    }

    // 2. Activate terminal app
    activate_terminal(terminal_bundle_id)
}
