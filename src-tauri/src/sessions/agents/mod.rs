use std::process::Command;

use agentoast_shared::{db, models::AgentStatus};

use crate::terminal::find_tmux;

mod claude;
mod codex;
mod opencode;

pub(super) struct AgentDetectionResult {
    pub status: AgentStatus,
    pub waiting_reason: Option<String>,
    pub agent_modes: Vec<String>,
    pub team_role: Option<String>, // "lead" or "teammate"
    pub team_name: Option<String>, // "@agent-alpha" (teammate only)
}

/// Capture tmux pane content as plain text.
/// Returns None if tmux is not found or the capture command fails.
pub(super) fn capture_pane(pane_id: &str) -> Option<String> {
    let tmux_path = find_tmux()?;
    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["capture-pane", "-t", pane_id, "-p"])
        .output()
        .ok()?;
    if !output.status.success() {
        log::debug!(
            "capture_pane({}): exit={} stderr={}",
            pane_id,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Check if a line is a numbered option (e.g., "1. Yes", "2. No").
/// Shared between Claude Code and Codex detection.
pub(super) fn is_numbered_option(line: &str) -> bool {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => chars.as_str().starts_with(". "),
        _ => false,
    }
}

pub(super) fn detect_agent_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    agent_type: &str,
) -> AgentDetectionResult {
    match agent_type {
        "claude-code" => claude::detect_claude_status(db_conn, pane_id),
        "codex" => codex::detect_codex_status(db_conn, pane_id),
        "opencode" => opencode::detect_opencode_status(db_conn, pane_id),
        _ => {
            log::debug!(
                "detect_agent_status({}): unknown agent_type='{}', defaulting to Running",
                pane_id,
                agent_type
            );
            AgentDetectionResult {
                status: AgentStatus::Running,
                waiting_reason: None,
                agent_modes: Vec::new(),
                team_role: None,
                team_name: None,
            }
        }
    }
}
