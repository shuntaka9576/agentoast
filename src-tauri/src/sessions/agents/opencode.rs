use agentoast_shared::{db, models::AgentStatus};

use super::capture_pane;

/// OpenCode mode patterns: (substring after ▣ prefix, label for frontend)
const OPENCODE_MODE_PATTERNS: &[(&str, &str)] = &[("Plan", "plan"), ("Build", "build")];

struct OpencodePaneContentInfo {
    is_running: bool, // "esc interrupt" / "esc again to interrupt" in status bar
    has_selection_dialog: bool, // "↑↓ select  enter submit  esc dismiss"
    has_permission_dialog: bool, // "Permission Required" + Allow/Deny buttons
    agent_modes: Vec<String>, // "plan", "build"
}

pub(super) fn detect_opencode_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
) -> (AgentStatus, Option<String>, Vec<String>) {
    let info = check_opencode_pane_content(pane_id);

    log::debug!(
        "detect_opencode_status({}): running={} permission={} selection={}",
        pane_id,
        info.is_running,
        info.has_permission_dialog,
        info.has_selection_dialog
    );

    let (status, waiting_reason) = if info.is_running {
        (AgentStatus::Running, None)
    } else if info.has_permission_dialog || info.has_selection_dialog {
        (AgentStatus::Waiting, Some("respond".to_string()))
    } else {
        // No running signal — check DB for pending notifications
        if let Some(conn) = db_conn {
            if let Ok(Some(_)) = db::get_latest_notification_by_pane(conn, pane_id) {
                (AgentStatus::Waiting, None)
            } else {
                (AgentStatus::Idle, None)
            }
        } else {
            (AgentStatus::Idle, None)
        }
    };

    (status, waiting_reason, info.agent_modes)
}

fn check_opencode_pane_content(pane_id: &str) -> OpencodePaneContentInfo {
    let default = OpencodePaneContentInfo {
        is_running: false,
        has_selection_dialog: false,
        has_permission_dialog: false,
        agent_modes: Vec::new(),
    };

    let content = match capture_pane(pane_id) {
        Some(c) => c,
        None => {
            log::debug!(
                "check_opencode_pane_content({}): capture-pane failed",
                pane_id
            );
            return default;
        }
    };

    let all_lines: Vec<&str> = content.lines().collect();

    // Get last 30 non-empty lines for scanning
    let last_lines: Vec<&str> = all_lines
        .iter()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(30)
        .copied()
        .collect();

    log::debug!(
        "check_opencode_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut is_running = false;
    let mut has_selection_dialog = false;
    let mut has_permission_dialog = false;
    let mut agent_modes: Vec<String> = Vec::new();

    for line in &last_lines {
        let trimmed = line.trim();

        // Running detection: "esc interrupt" or "esc again to interrupt"
        if !is_running && is_opencode_running_line(trimmed) {
            log::debug!(
                "check_opencode_pane_content({}): running detected: {:?}",
                pane_id,
                trimmed
            );
            is_running = true;
        }

        // Selection dialog: "↑↓ select  enter submit  esc dismiss"
        if !has_selection_dialog && is_opencode_selection_dialog_line(trimmed) {
            log::debug!(
                "check_opencode_pane_content({}): selection dialog detected: {:?}",
                pane_id,
                trimmed
            );
            has_selection_dialog = true;
        }

        // Permission dialog: "Permission Required" or "Allow (a)" + "Deny (d)"
        if !has_permission_dialog && is_opencode_permission_dialog_line(trimmed) {
            log::debug!(
                "check_opencode_pane_content({}): permission dialog detected: {:?}",
                pane_id,
                trimmed
            );
            has_permission_dialog = true;
        }

        // Mode detection from task indicator lines: "▣  Plan · big-pickle"
        if trimmed.starts_with('\u{25A3}') {
            let task_text = trimmed.trim_start_matches('\u{25A3}').trim();
            for &(pattern, label) in OPENCODE_MODE_PATTERNS {
                if !agent_modes.iter().any(|m| m == label) && task_text.starts_with(pattern) {
                    log::debug!(
                        "check_opencode_pane_content({}): mode '{}' detected: {:?}",
                        pane_id,
                        label,
                        trimmed
                    );
                    agent_modes.push(label.to_string());
                }
            }
        }
    }

    OpencodePaneContentInfo {
        is_running,
        has_selection_dialog,
        has_permission_dialog,
        agent_modes,
    }
}

/// Check if a line indicates OpenCode is actively running.
/// Matches "esc interrupt" or "esc again to interrupt" in the bottom status bar.
/// e.g., "⬝⬝■■■■■■  esc interrupt  ctrl+t variants ..."
/// e.g., "⬝⬝⬝⬝⬝⬝⬝⬝  esc again to interrupt  ctrl+t variants ..."
fn is_opencode_running_line(line: &str) -> bool {
    line.contains("esc interrupt") || line.contains("esc again to interrupt")
}

/// Check if a line is the OpenCode selection dialog footer.
/// Matches "↑↓ select  enter submit  esc dismiss" which appears at the bottom
/// of question/plan selection dialogs.
fn is_opencode_selection_dialog_line(line: &str) -> bool {
    let stripped = strip_opencode_border(line);
    stripped.contains("select")
        && stripped.contains("enter submit")
        && stripped.contains("esc dismiss")
}

/// Check if a line indicates an OpenCode permission dialog.
/// Permission dialogs show "Permission Required" or "Allow (a)" + "Deny (d)" buttons.
fn is_opencode_permission_dialog_line(line: &str) -> bool {
    let stripped = strip_opencode_border(line);
    stripped.contains("Permission Required")
        || (stripped.contains("Allow (a)") && stripped.contains("Deny (d)"))
}

/// Strip leading ┃ (U+2503 BOX DRAWINGS HEAVY VERTICAL) border and whitespace.
/// OpenCode uses ┃ as a left border for chat messages and dialog content.
fn strip_opencode_border(line: &str) -> &str {
    line.trim().trim_start_matches('\u{2503}').trim_start()
}
