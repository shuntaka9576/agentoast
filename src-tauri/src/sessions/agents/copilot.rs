use agentoast_shared::{db, models::AgentStatus};

use super::{capture_pane, is_numbered_option, AgentDetectionResult};

/// Copilot CLI spinner characters that cycle during "Thinking" state.
const COPILOT_SPINNER_CHARS: &[char] = &[
    '◎', // U+25CE BULLSEYE
    '○', // U+25CB WHITE CIRCLE
    '◉', // U+25C9 FISHEYE
    '●', // U+25CF BLACK CIRCLE
];

/// Mode detection patterns: (substring to match in status bar, label for frontend)
/// Status bar examples:
///   Idle:    "v1.0.12 available · run /update · plan · shift+tab switch mode"
///   Running: "plan · shift+tab switch mode"  (update notice disappears, plan at line start)
///   Idle:    "v1.0.12 available · run /update · autopilot · shift+tab switch mode"
///   Running: "autopilot · shift+tab switch mode"
/// Pattern uses "plan \u{00B7}" / "autopilot \u{00B7}" to match both positions.
/// This is safe because "plan" and "autopilot" don't appear elsewhere in the status bar.
const COPILOT_MODE_PATTERNS: &[(&str, &str)] = &[
    ("plan \u{00B7}", "plan"),
    ("autopilot \u{00B7}", "autopilot"),
];

struct CopilotPaneContentInfo {
    has_spinner: bool,          // Spinner chars (◎○◉●) + "Esc to cancel"
    has_selection_dialog: bool, // ❯ N. selection cursor + 2+ numbered options
    has_tool_approval: bool,    // "Do you want to run this command?"
    at_prompt: bool,            // ❯ prompt character
    agent_modes: Vec<String>,   // "plan", "autopilot"
}

pub(super) fn detect_copilot_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
) -> AgentDetectionResult {
    let info = check_copilot_pane_content(pane_id);

    log::debug!(
        "detect_copilot_status({}): spinner={} selection={} tool_approval={} prompt={}",
        pane_id,
        info.has_spinner,
        info.has_selection_dialog,
        info.has_tool_approval,
        info.at_prompt
    );

    let (status, waiting_reason) = if info.has_spinner {
        (AgentStatus::Running, None)
    } else if info.has_selection_dialog && info.has_tool_approval {
        // Tool execution approval dialog: "Do you want to run this command?" + selection cursor
        (AgentStatus::Waiting, Some("respond".to_string()))
    } else if info.has_selection_dialog {
        // Question/elicitation dialog: selection cursor without tool approval text
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

    // Copilot CLI does not support Agent Teams
    AgentDetectionResult {
        status,
        waiting_reason,
        agent_modes: info.agent_modes,
        team_role: None,
        team_name: None,
    }
}

fn check_copilot_pane_content(pane_id: &str) -> CopilotPaneContentInfo {
    let default = CopilotPaneContentInfo {
        has_spinner: false,
        has_selection_dialog: false,
        has_tool_approval: false,
        at_prompt: false,
        agent_modes: Vec::new(),
    };

    let content = match capture_pane(pane_id) {
        Some(c) => c,
        None => {
            log::debug!(
                "check_copilot_pane_content({}): capture-pane failed",
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
        "check_copilot_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut has_spinner = false;
    let mut has_tool_approval = false;
    let mut has_selection_cursor = false;
    let mut numbered_option_count: usize = 0;
    let mut agent_modes: Vec<String> = Vec::new();

    for line in &last_lines {
        let trimmed = line.trim();

        // Running detection: spinner char (◎○◉●) + "Esc to cancel"
        if !has_spinner && is_copilot_running_line(trimmed) {
            log::debug!(
                "check_copilot_pane_content({}): running detected: {:?}",
                pane_id,
                trimmed
            );
            has_spinner = true;
        }

        // Tool approval detection: "Do you want to run this command?"
        // Strip box border (│) before checking, as this text appears inside a dialog box.
        let stripped = strip_box_border(trimmed);
        if !has_tool_approval && stripped.contains("Do you want to run this command?") {
            log::debug!(
                "check_copilot_pane_content({}): tool approval detected: {:?}",
                pane_id,
                trimmed
            );
            has_tool_approval = true;
        }

        // Selection cursor (❯ N.) and numbered options — check after stripping box border
        if is_selection_cursor(stripped) {
            has_selection_cursor = true;
            numbered_option_count += 1;
        } else if is_numbered_option(stripped) {
            numbered_option_count += 1;
        }

        // Mode detection from status bar (last ~7 lines)
        for &(pattern, label) in COPILOT_MODE_PATTERNS {
            if !agent_modes.iter().any(|m| m == label) && trimmed.contains(pattern) {
                log::debug!(
                    "check_copilot_pane_content({}): mode '{}' detected: {:?}",
                    pane_id,
                    label,
                    trimmed
                );
                agent_modes.push(label.to_string());
            }
        }
    }

    // Selection dialog: requires ❯ N. selection cursor AND 2+ total numbered options
    let has_selection_dialog = has_selection_cursor && numbered_option_count >= 2;
    if has_selection_dialog {
        log::debug!(
            "check_copilot_pane_content({}): selection dialog detected ({} options)",
            pane_id,
            numbered_option_count
        );
    }

    // Idle detection: walk from bottom, skip TUI footer, check for ❯ prompt
    let at_prompt = is_copilot_prompt_line(&all_lines);
    if at_prompt {
        log::debug!(
            "check_copilot_pane_content({}): prompt line detected",
            pane_id
        );
    }

    CopilotPaneContentInfo {
        has_spinner,
        has_selection_dialog,
        has_tool_approval,
        at_prompt,
        agent_modes,
    }
}

/// Check if a line indicates Copilot CLI is actively running.
/// Matches spinner characters (◎○◉●) followed by "Esc to cancel".
/// e.g., "◎ Thinking (Esc to cancel · 668 B)"
fn is_copilot_running_line(line: &str) -> bool {
    if let Some(c) = line.chars().next() {
        if COPILOT_SPINNER_CHARS.contains(&c) && line.contains("Esc to cancel") {
            return true;
        }
    }
    false
}

/// Check if the last meaningful line is a prompt, skipping TUI footer lines.
/// Copilot CLI uses ❯ as the prompt character, same as Claude Code's starship prompt.
/// The TUI footer consists of separators and a status bar line.
fn is_copilot_prompt_line(lines: &[&str]) -> bool {
    const MAX_UNKNOWN_LINES: usize = 3;
    let mut unknown_count = 0;

    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_separator_line(trimmed) {
            continue;
        }
        // Status bar: contains "shift+tab switch mode" (always present)
        if trimmed.contains("shift+tab switch mode") {
            continue;
        }
        // Status bar: contains "Remaining reqs.:" (always present)
        if trimmed.contains("Remaining reqs.:") {
            continue;
        }
        // Selection dialog footer lines
        if trimmed.contains("to select") && trimmed.contains("Esc to cancel") {
            continue;
        }
        if trimmed.contains("to navigate") && trimmed.contains("Esc to cancel") {
            continue;
        }
        // Numbered options inside dialogs (e.g., "  2. No, and tell Copilot ...")
        let stripped = strip_box_border(trimmed);
        if is_numbered_option(stripped) {
            continue;
        }
        // Selection cursor inside dialog (❯ N. ...) — skip, not a prompt
        if is_selection_cursor(stripped) {
            continue;
        }
        // Box border lines (╭╮╰╯) — part of dialog
        if is_box_corner_line(trimmed) {
            continue;
        }
        // Prompt: ❯ character
        if stripped.starts_with('❯') {
            return true;
        }
        unknown_count += 1;
        if unknown_count >= MAX_UNKNOWN_LINES {
            return false;
        }
    }
    false
}

/// Check if a line consists entirely of box-drawing characters (U+2500..U+257F).
fn is_separator_line(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| ('\u{2500}'..='\u{257F}').contains(&c))
}

/// Strip leading/trailing box drawing vertical bar (│ U+2502) and whitespace.
fn strip_box_border(line: &str) -> &str {
    line.trim_start_matches('│')
        .trim_start()
        .trim_end_matches('│')
        .trim_end()
}

/// Check if a ❯-prefixed line is a selection cursor on a numbered option.
/// "❯ 1. Yes" → true (selection cursor)
/// "❯ Type @ to mention files" → false (prompt with placeholder text)
fn is_selection_cursor(line: &str) -> bool {
    let trimmed = line.trim();
    let rest = trimmed.trim_start_matches('❯').trim_start();
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

/// Check if a line starts with a box corner character (╭╮╰╯).
/// These are part of Copilot CLI's dialog box borders.
fn is_box_corner_line(line: &str) -> bool {
    let trimmed = line.trim();
    if let Some(c) = trimmed.chars().next() {
        matches!(c, '╭' | '╮' | '╰' | '╯')
    } else {
        false
    }
}
