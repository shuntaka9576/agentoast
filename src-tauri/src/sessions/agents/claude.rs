use agentoast_shared::{db, models::AgentStatus};

use super::{is_numbered_option, AgentDetectionResult};

struct ClaudePaneContentInfo {
    has_spinner: bool, // Spinner chars + "…" / "esc to interrupt" (real-time, reliable)
    has_status_running: bool, // Status bar "(running)" suffix (may be stale)
    at_prompt: bool,
    has_question_dialog: bool, // "Enter to select" navigation hint (AskUserQuestion dialog)
    has_plan_approval: bool,   // ❯ N. selection cursor + 2+ numbered options (plan approval etc.)
    bash_count: Option<u32>,   // Background bash task count from "· N bash" in mode line
    agent_modes: Vec<String>,
    team_role: Option<String>, // "lead" or "teammate" (Agent Teams feature)
    team_name: Option<String>, // "@agent-alpha" for teammates
}

pub(super) fn detect_claude_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    content: Option<&str>,
) -> AgentDetectionResult {
    let info = check_claude_pane_content(pane_id, content);

    log::debug!(
        "detect_claude_status({}): spinner={} status_running={} question_dialog={} plan_approval={} prompt={}",
        pane_id,
        info.has_spinner,
        info.has_status_running,
        info.has_question_dialog,
        info.has_plan_approval,
        info.at_prompt
    );

    // Spinners are real-time signals and take highest priority.
    // Status bar "(running)" may be stale (e.g., plan mode waiting with old
    // status bar text), so it does NOT override at_prompt.
    let (status, waiting_reason) = if info.has_spinner {
        (AgentStatus::Running, None)
    } else if info.has_question_dialog {
        // Question dialog ("Enter to select" detected) — always Waiting.
        // Checked before at_prompt because question dialog option description text
        // (indented continuation lines) causes is_prompt_line() to return false.
        (AgentStatus::Waiting, Some("respond".to_string()))
    } else if info.has_plan_approval && !info.at_prompt {
        // Plan approval dialog without "Enter to select" (e.g., context clearing).
        // Detected via ❯ N. selection cursor + 2+ numbered options.
        // Only valid when no prompt is detected — if at_prompt is true, the user
        // has already completed the selection and the dialog text is just stale
        // content still visible on screen.
        (AgentStatus::Waiting, Some("respond".to_string()))
    } else if info.at_prompt {
        // Background bash tasks mean work is still in progress — treat as Running
        if info.bash_count.is_some_and(|c| c > 0) {
            (AgentStatus::Running, None)
        } else if let Some(conn) = db_conn {
            if let Ok(Some(_)) = db::get_latest_notification_by_pane(conn, pane_id) {
                (AgentStatus::Waiting, None)
            } else {
                (AgentStatus::Idle, None)
            }
        } else {
            (AgentStatus::Idle, None)
        }
    } else {
        // has_status_running or no signal — default to Running
        (AgentStatus::Running, None)
    };

    AgentDetectionResult {
        status,
        waiting_reason,
        agent_modes: info.agent_modes,
        team_role: info.team_role,
        team_name: info.team_name,
    }
}

/// Claude Code spinner characters that appear at the start of running lines.
const SPINNER_CHARS: &[char] = &['✢', '✽', '✶', '✳', '✻', '·'];

/// Mode detection patterns: (substring to match, label for frontend)
const MODE_PATTERNS: &[(&str, &str)] = &[
    ("plan mode on", "plan"),
    ("bypass permissions on", "bypass"),
    ("accept edits on", "accept"),
];

fn check_claude_pane_content(pane_id: &str, content: Option<&str>) -> ClaudePaneContentInfo {
    let default = ClaudePaneContentInfo {
        has_spinner: false,
        has_status_running: false,
        at_prompt: false,
        has_question_dialog: false,
        has_plan_approval: false,
        bash_count: None,
        agent_modes: Vec::new(),
        team_role: None,
        team_name: None,
    };

    let content = match content {
        Some(c) => c,
        None => {
            log::debug!(
                "check_claude_pane_content({}): no content available",
                pane_id
            );
            return default;
        }
    };

    let all_lines: Vec<&str> = content.lines().collect();

    // Get last 30 non-empty, non-separator lines for scanning
    let last_lines: Vec<&str> = all_lines
        .iter()
        .rev()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !is_separator_line(trimmed)
        })
        .take(30)
        .copied()
        .collect();

    log::debug!(
        "check_claude_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut has_spinner = false;
    let mut has_status_running = false;
    let mut has_question_dialog = false;
    let mut has_selection_cursor = false;
    let mut numbered_option_count: usize = 0;
    let mut agent_modes: Vec<String> = Vec::new();
    let mut bash_count: Option<u32> = None; // set in status_area scan below

    for line in &last_lines {
        let trimmed = line.trim();

        // Running detection: spinner char + "esc to interrupt" or "…"
        if !has_spinner && is_claude_running_line(trimmed) {
            log::debug!(
                "check_claude_pane_content({}): running detected (spinner): {:?}",
                pane_id,
                trimmed
            );
            has_spinner = true;
        }

        // Status bar "(running)" suffix — may be stale
        // e.g., "⏵⏵ bypass permissions on · for dir in auth admin; do… (running)"
        if !has_status_running && trimmed.ends_with("(running)") {
            log::debug!(
                "check_claude_pane_content({}): status bar running detected: {:?}",
                pane_id,
                trimmed
            );
            has_status_running = true;
        }

        // Question dialog detection: "Enter to select · ↑/↓ to navigate · Esc to cancel"
        if !has_question_dialog && trimmed.starts_with("Enter to select") {
            log::debug!(
                "check_claude_pane_content({}): question dialog detected: {:?}",
                pane_id,
                trimmed
            );
            has_question_dialog = true;
        }

        // Count numbered options and selection cursors for selection dialog detection.
        // Selection cursor (❯ N. ) is the key discriminator — without it, numbered
        // lines are just markdown content, not a selection dialog.
        if is_selection_cursor(trimmed) {
            has_selection_cursor = true;
            numbered_option_count += 1;
        } else if is_numbered_option(trimmed) {
            numbered_option_count += 1;
        }

        // Agent mode detection: plan, bypass, accept
        for &(pattern, label) in MODE_PATTERNS {
            if !agent_modes.iter().any(|m| m == label) && trimmed.contains(pattern) {
                log::debug!(
                    "check_claude_pane_content({}): mode '{}' detected: {:?}",
                    pane_id,
                    label,
                    trimmed
                );
                agent_modes.push(label.to_string());
            }
        }
    }

    // Scan the status bar area (bottom ~7 lines) for background bash count,
    // Agent Teams detection, etc. Limited to 7 lines to avoid false positives
    // from conversation text.
    let status_area = &last_lines[..last_lines.len().min(7)];
    let mut team_role: Option<String> = None;
    let mut team_name: Option<String> = None;

    for line in status_area {
        let trimmed = line.trim();

        // Background bash task detection: "· N bash" pattern anywhere in status area.
        // Can appear on the mode line ("⏵⏵ bypass permissions on · 1 bash")
        // or as a standalone status line ("1 bash · PR #1381").
        if bash_count.is_none() {
            bash_count = extract_bash_count(trimmed);
        }

        // Lead: mode line (⏵/⏸) containing "teammate"
        //   e.g., "⏸ plan mode on · 3 teammates"
        if team_role.is_none() {
            let is_mode_line = trimmed.starts_with('⏵') || trimmed.starts_with('⏸');
            if is_mode_line && trimmed.contains("teammate") {
                log::debug!(
                    "check_claude_pane_content({}): agent team lead detected (mode line): {:?}",
                    pane_id,
                    trimmed
                );
                team_role = Some("lead".to_string());
            }
        }

        // Lead: team listing starting with @ and containing "· ↓ to expand"
        //   e.g., "@main @agent-alpha @agent-beta @agent-gamma · ↓ to expand"
        if team_role.is_none()
            && trimmed.starts_with('@')
            && trimmed.contains("\u{00B7} \u{2193} to expand")
        {
            log::debug!(
                "check_claude_pane_content({}): agent team lead detected (team listing): {:?}",
                pane_id,
                trimmed
            );
            team_role = Some("lead".to_string());
        }

        // Teammate: separator "──── @agent-name ──"
        if team_role.is_none() && trimmed.starts_with('\u{2500}') {
            if let Some(name) = extract_team_agent_name(trimmed) {
                log::debug!(
                    "check_claude_pane_content({}): agent team teammate '{}' detected: {:?}",
                    pane_id,
                    name,
                    trimmed
                );
                team_role = Some("teammate".to_string());
                team_name = Some(name);
            }
        }
    }

    // Plan approval: requires ❯ N. selection cursor AND 2+ total numbered option lines.
    // Without the selection cursor, numbered lines are just markdown content (e.g., "1. PR特定").
    let has_plan_approval = has_selection_cursor && numbered_option_count >= 2;
    if has_plan_approval {
        log::debug!(
            "check_claude_pane_content({}): plan approval detected ({} numbered options)",
            pane_id,
            numbered_option_count
        );
    }

    // Idle detection: walk from bottom, skip TUI footer, check if first
    // meaningful line is a prompt (❯, $, %, >)
    let at_prompt = is_prompt_line(&all_lines);
    if at_prompt {
        log::debug!(
            "check_claude_pane_content({}): prompt line detected",
            pane_id
        );
    }

    // Add background bash count to agent_modes if detected
    if let Some(count) = bash_count {
        if count > 0 {
            log::debug!(
                "check_claude_pane_content({}): {} background bash task(s) detected",
                pane_id,
                count
            );
            agent_modes.push(format!("{} bash", count));
        }
    }

    ClaudePaneContentInfo {
        has_spinner,
        has_status_running,
        at_prompt,
        has_question_dialog,
        has_plan_approval,
        bash_count,
        agent_modes,
        team_role,
        team_name,
    }
}

/// Extract agent name from an Agent Teams teammate separator line.
/// "──────── @agent-alpha ──" → Some("@agent-alpha")
/// Lines start with ─ (U+2500) and contain " @name " pattern.
fn extract_team_agent_name(line: &str) -> Option<String> {
    let at_pos = line.find(" @")?;
    let rest = &line[at_pos + 1..]; // "@agent-alpha ──"
    let end = rest.find(' ').unwrap_or(rest.len());
    let name = &rest[..end];
    if name.starts_with('@') && name.len() > 1 {
        Some(name.to_string())
    } else {
        None
    }
}

/// Extract background bash task count from a status bar line.
/// Pattern 1 (mode line suffix): "⏵⏵ bypass permissions on · 1 bash" → Some(1)
/// Pattern 2 (standalone line):  "1 bash · PR #1381" → Some(1)
fn extract_bash_count(line: &str) -> Option<u32> {
    let trimmed = line.trim();

    // Pattern 1: "· N bash" suffix (· = U+00B7 MIDDLE DOT)
    let marker = "\u{00B7} ";
    if let Some(pos) = trimmed.rfind(marker) {
        let after = trimmed[pos + marker.len()..].trim();
        let mut parts = after.split_whitespace();
        if let Some(count) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
            if parts.next() == Some("bash") {
                return Some(count);
            }
        }
    }

    // Pattern 2: "N bash" at line start (e.g., "1 bash · PR #1381")
    let mut parts = trimmed.split_whitespace();
    if let Some(count) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
        if parts.next() == Some("bash") {
            return Some(count);
        }
    }

    None
}

/// Check if a line indicates Claude Code is actively running.
/// Matches spinner characters followed by "esc to interrupt" or "…" (ellipsis).
fn is_claude_running_line(line: &str) -> bool {
    if let Some(c) = line.chars().next() {
        if SPINNER_CHARS.contains(&c) {
            // Spinner char + "esc to interrupt"
            // e.g., "✻ Thinking… (esc to interrupt · 30s · ...)"
            if line.contains("esc to interrupt") {
                return true;
            }
            // Spinner char + "…" (active progress indicator)
            // e.g., "✶ Galloping…", "✻ Thinking…", "✢ Compacting…"
            if line.contains('…') {
                return true;
            }
        }
    }
    // "esc to interrupt" in status line suffix
    // e.g., "4 files +20 -0 · esc to interrupt"
    if line.contains("· esc to interrupt") {
        return true;
    }
    false
}

/// Check if the last meaningful line is a prompt, skipping TUI footer lines.
/// Walks from bottom to top, skipping empty lines, separators, status bar,
/// and help text. Up to MAX_UNKNOWN_LINES non-prompt lines are tolerated
/// (e.g. user-configured statusline) before giving up.
fn is_prompt_line(lines: &[&str]) -> bool {
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
        // Mode indicator: ⏵⏵ bypass permissions, ⏸ plan mode
        if trimmed.starts_with('⏵') || trimmed.starts_with('⏸') {
            continue;
        }
        // ctrl shortcut hints (e.g., "ctrl+b ctrl+b (twice) to run in background",
        // "ctrl-g to edit in Nvim")
        if trimmed.contains("ctrl+") || trimmed.contains("ctrl-") {
            continue;
        }
        // Context auto-compact warning (e.g., "Context left until auto-compact: 8%")
        if trimmed.contains("Context left until auto-compact") {
            continue;
        }
        // Skip Claude Code TUI footer lines
        if trimmed.contains("for shortcuts")
            || trimmed.contains("shift+tab to cycle")
            || is_file_changes_line(trimmed)
        {
            continue;
        }
        // Claude Code elicitation numbered options (e.g., "  2. Yes, and bypass permissions")
        // Skip these so we can reach the ❯-prefixed selected option line underneath.
        if is_numbered_option(trimmed) {
            continue;
        }
        // Claude Code elicitation navigation hint
        // e.g., "Enter to select · ↑/↓ to navigate · Esc to cancel"
        if trimmed.starts_with("Enter to select") {
            continue;
        }
        // Meaningful line: strip box border (│ ... │) then check prompt
        let check = strip_box_border(trimmed);
        // ❯ followed by "N. " is a selection cursor (plan approval etc.), not a prompt
        if check.starts_with('❯') && is_selection_cursor(check) {
            continue;
        }
        if check.starts_with('❯')         // starship / Claude Code prompt
            || check.ends_with("$ ")       // bash
            || check == "$"
            || check.ends_with("% ")       // zsh
            || check == "%"
            || check == ">"                // REPL prompt
            || check.starts_with("> ")
        {
            return true;
        }
        // Non-prompt meaningful line (e.g. statusline). Tolerate up to
        // MAX_UNKNOWN_LINES before concluding agent is not at a prompt.
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
/// Used to detect prompts inside Claude Code's bordered input box.
fn strip_box_border(line: &str) -> &str {
    line.trim_start_matches('│')
        .trim_start()
        .trim_end_matches('│')
        .trim_end()
}

/// Check if a ❯-prefixed line is a selection cursor on a numbered option.
/// "❯ 1. Yes, clear context" → true (selection cursor in plan approval dialog)
/// "❯ ls -la" → false (user typing at prompt)
/// "❯" → false (empty prompt)
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

/// Check if a line shows file changes (e.g., "4 files +42 -0").
fn is_file_changes_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        && trimmed.contains("file")
        && (trimmed.contains('+') || trimmed.contains('-'))
}
