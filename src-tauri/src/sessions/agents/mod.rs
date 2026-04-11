use std::process::Command;

use agentoast_shared::{db, models::AgentStatus};

use crate::terminal::find_tmux;

mod claude;
mod codex;
mod copilot;
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
pub(crate) fn capture_pane(pane_id: &str) -> Option<String> {
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

/// Lightweight running check: capture a pane and check for universal running signals.
/// Does NOT require agent_type or process tree — uses patterns common to all agents.
pub(crate) fn is_pane_agent_running(pane_id: &str) -> bool {
    let content = match capture_pane(pane_id) {
        Some(c) => c,
        None => return false,
    };

    content
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(30)
        .any(|line| {
            let trimmed = line.trim();
            is_universal_running_line(trimmed)
        })
}

/// Running signals common across Claude Code, Codex, OpenCode, and Copilot CLI.
fn is_universal_running_line(line: &str) -> bool {
    // Claude Code: spinner chars (✢✽✶✳✻·) + "esc to interrupt" or "…"
    if let Some(c) = line.chars().next() {
        if ['✢', '✽', '✶', '✳', '✻', '·'].contains(&c)
            && (line.contains("esc to interrupt") || line.contains('…'))
        {
            return true;
        }
    }
    // Copilot CLI: spinner chars (◎○◉●) + "Esc to cancel"
    if let Some(c) = line.chars().next() {
        if ['◎', '○', '◉', '●'].contains(&c) && line.contains("Esc to cancel") {
            return true;
        }
    }
    // Claude Code: "· esc to interrupt" in status line
    if line.contains("\u{00B7} esc to interrupt") {
        return true;
    }
    // Claude Code: status bar "(running)" suffix
    if line.ends_with("(running)") {
        return true;
    }
    // Codex: "esc to interrupt" (covered by spinner check above)
    // OpenCode: "esc interrupt" (without "to")
    if line.contains("esc interrupt") {
        return true;
    }
    false
}

#[allow(dead_code)]
pub(super) fn detect_agent_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    agent_type: &str,
) -> AgentDetectionResult {
    let content = capture_pane(pane_id);
    detect_agent_status_with_content(db_conn, pane_id, agent_type, content.as_deref())
}

/// Detect agent status using pre-captured pane content.
/// This avoids redundant capture-pane calls when content is already available
/// (e.g., captured in parallel by the caller).
pub(super) fn detect_agent_status_with_content(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    agent_type: &str,
    content: Option<&str>,
) -> AgentDetectionResult {
    match agent_type {
        "claude-code" => claude::detect_claude_status(db_conn, pane_id, content),
        "codex" => codex::detect_codex_status(db_conn, pane_id, content),
        "copilot-cli" => copilot::detect_copilot_status(db_conn, pane_id, content),
        "opencode" => opencode::detect_opencode_status(db_conn, pane_id, content),
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
