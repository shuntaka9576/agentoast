use agentoast_shared::{db, models::AgentStatus};

use super::{capture_pane, is_numbered_option};

struct CodexPaneContentInfo {
    is_running: bool,          // "(XXs • esc to interrupt)" pattern
    has_question_dialog: bool, // "enter to submit answer" in question dialog footer
    has_plan_approval: bool,   // "enter to confirm" in plan approval footer
    at_prompt: bool,           // › (U+203A) prompt character
}

pub(super) fn detect_codex_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
) -> (AgentStatus, Option<String>, Vec<String>) {
    let info = check_codex_pane_content(pane_id);

    log::debug!(
        "detect_codex_status({}): running={} question_dialog={} plan_approval={} prompt={}",
        pane_id,
        info.is_running,
        info.has_question_dialog,
        info.has_plan_approval,
        info.at_prompt
    );

    let (status, waiting_reason) = if info.is_running {
        (AgentStatus::Running, None)
    } else if info.has_question_dialog || info.has_plan_approval {
        // Question dialog ("enter to submit answer") or plan approval ("enter to confirm")
        // detected — always Waiting. Takes priority over at_prompt because the selection
        // cursor (› N.) can be misidentified as a prompt.
        (AgentStatus::Waiting, Some("respond".to_string()))
    } else if info.at_prompt {
        if let Some(conn) = db_conn {
            if let Ok(Some(_)) = db::get_latest_notification_by_pane(conn, pane_id) {
                (AgentStatus::Waiting, None)
            } else {
                (AgentStatus::Idle, None)
            }
        } else {
            (AgentStatus::Idle, None)
        }
    } else {
        // No clear signal — default to Running (conservative)
        (AgentStatus::Running, None)
    };

    // Codex has no mode indicators (plan/bypass/accept)
    (status, waiting_reason, Vec::new())
}

fn check_codex_pane_content(pane_id: &str) -> CodexPaneContentInfo {
    let default = CodexPaneContentInfo {
        is_running: false,
        has_question_dialog: false,
        has_plan_approval: false,
        at_prompt: false,
    };

    let content = match capture_pane(pane_id) {
        Some(c) => c,
        None => {
            log::debug!("check_codex_pane_content({}): capture-pane failed", pane_id);
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
        "check_codex_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut is_running = false;
    let mut has_question_dialog = false;
    let mut has_plan_approval = false;

    for line in &last_lines {
        let trimmed = line.trim();
        if !is_running && is_codex_running_line(trimmed) {
            log::debug!(
                "check_codex_pane_content({}): running detected: {:?}",
                pane_id,
                trimmed
            );
            is_running = true;
        }
        if !has_question_dialog && is_codex_question_dialog_line(trimmed) {
            log::debug!(
                "check_codex_pane_content({}): question dialog detected: {:?}",
                pane_id,
                trimmed
            );
            has_question_dialog = true;
        }
        if !has_plan_approval && is_codex_plan_approval_line(trimmed) {
            log::debug!(
                "check_codex_pane_content({}): plan approval detected: {:?}",
                pane_id,
                trimmed
            );
            has_plan_approval = true;
        }
    }

    let at_prompt = is_codex_prompt_line(&all_lines);
    if at_prompt {
        log::debug!(
            "check_codex_pane_content({}): prompt line detected",
            pane_id
        );
    }

    CodexPaneContentInfo {
        is_running,
        has_question_dialog,
        has_plan_approval,
        at_prompt,
    }
}

/// Check if a line indicates Codex is actively running.
/// Matches the pattern "(XXs • esc to interrupt)" where XX is a duration.
/// e.g., "• Working (48s • esc to interrupt) · 1 background terminal running"
fn is_codex_running_line(line: &str) -> bool {
    line.contains("s \u{2022} esc to interrupt") && line.contains('(')
}

/// Check if the last meaningful line is a Codex prompt (›), skipping footer lines
/// and question dialog elements.
fn is_codex_prompt_line(lines: &[&str]) -> bool {
    const MAX_UNKNOWN_LINES: usize = 3;
    let mut unknown_count = 0;

    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_codex_footer_line(trimmed) {
            continue;
        }
        // Skip question dialog footer (e.g., "tab to add notes | enter to submit answer | ...")
        if is_codex_question_dialog_line(trimmed) {
            continue;
        }
        // Skip plan approval footer (e.g., "Press enter to confirm or esc to go back")
        if is_codex_plan_approval_line(trimmed) {
            continue;
        }
        // Skip numbered options in question dialog (e.g., "2. 既存ユーザー ...")
        if is_numbered_option(trimmed) {
            continue;
        }
        // › (U+203A SINGLE RIGHT-POINTING ANGLE QUOTATION MARK) is the Codex prompt
        if trimmed.starts_with('\u{203A}') {
            // Skip selection cursor (› N. ...) in question dialog
            if is_codex_selection_cursor(trimmed) {
                continue;
            }
            return true;
        }
        unknown_count += 1;
        if unknown_count >= MAX_UNKNOWN_LINES {
            return false;
        }
    }
    false
}

/// Check if a line is a Codex TUI footer element that should be skipped.
fn is_codex_footer_line(line: &str) -> bool {
    line.contains("for shortcuts")
        || line.contains("context left")
        || line.contains("background terminal running")
        || line.contains("/ps to view")
        || line.contains("/clean to close")
}

/// Check if a line indicates a Codex question/elicitation dialog.
/// Matches the footer "tab to add notes | enter to submit answer | esc to interrupt"
/// which appears at the bottom of Codex's question dialogs.
fn is_codex_question_dialog_line(line: &str) -> bool {
    line.contains("enter to submit answer")
}

/// Check if a line indicates a Codex plan approval dialog.
/// Matches the footer "Press enter to confirm or esc to go back"
/// which appears at the bottom of Codex's plan approval screen.
fn is_codex_plan_approval_line(line: &str) -> bool {
    line.contains("enter to confirm")
}

/// Check if a ›-prefixed line is a selection cursor on a numbered option in Codex.
/// "› 1. New user (Recommended)" → true (selection cursor in question dialog)
/// "› ls -la" → false (user typing at prompt)
/// "›" → false (empty prompt)
fn is_codex_selection_cursor(line: &str) -> bool {
    let trimmed = line.trim();
    let rest = trimmed.trim_start_matches('\u{203A}').trim_start();
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => {
            let after_digits = chars
                .as_str()
                .trim_start_matches(|c: char| c.is_ascii_digit());
            after_digits.starts_with(". ")
        }
        _ => false,
    }
}
