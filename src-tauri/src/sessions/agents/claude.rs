use std::time::Instant;

use agentoast_shared::{db, models::AgentStatus};

use super::{is_numbered_option, AgentDetectionResult};
use crate::sessions::hysteresis::InputRegion;

/// How long after the body-hash last changed the pane is still treated as
/// Running while sitting `at_prompt`. Sized for a 2 s polling cadence so a
/// single missed spinner frame doesn't blink the status to Idle.
pub(crate) const CHANGE_TTL: std::time::Duration = std::time::Duration::from_millis(3000);

struct ClaudePaneContentInfo {
    has_spinner: bool, // Spinner chars + "…" / "esc to interrupt" (real-time, reliable)
    has_status_running: bool, // Status bar "(running)" suffix (may be stale)
    at_prompt: bool,
    has_question_dialog: bool, // "Enter to select" navigation hint (AskUserQuestion dialog)
    has_plan_approval: bool,   // ❯ N. selection cursor + 2+ numbered options (plan approval etc.)
    shell_count: Option<u32>, // Background shell task count from "· N shell" (or "· N bash") in mode line
    local_agent_count: Option<u32>, // Background local agent count from "· N local agent(s)" in mode line
    monitor_count: Option<u32>,     // Background monitor count from "· N monitor(s)" in mode line
    fork_count: Option<u32>, // Background fork count from "◯ <name>  <desc>  <duration>" picker rows
    agent_modes: Vec<String>,
    team_role: Option<String>, // "lead" or "teammate" (Agent Teams feature)
    team_name: Option<String>, // "@agent-alpha" for teammates
}

pub(super) fn detect_claude_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    content: Option<&str>,
    last_changed_at: Option<Instant>,
) -> AgentDetectionResult {
    let info = check_claude_pane_content(pane_id, content);

    let hash_age_ms = last_changed_at.map(|t| t.elapsed().as_millis());
    log::debug!(
        "detect_claude_status({}): spinner={} status_running={} question_dialog={} plan_approval={} prompt={} hash_age_ms={:?}",
        pane_id,
        info.has_spinner,
        info.has_status_running,
        info.has_question_dialog,
        info.has_plan_approval,
        info.at_prompt,
        hash_age_ms,
    );

    // Background work indicators that should override at_prompt Idle.
    // The fork picker rows ("◯ <name>") in particular prevent is_prompt_line
    // from finding the prompt, so this signal must also override the
    // "no prompt detected" fallback below.
    let has_background_work = info.shell_count.is_some_and(|c| c > 0)
        || info.local_agent_count.is_some_and(|c| c > 0)
        || info.monitor_count.is_some_and(|c| c > 0)
        || info.fork_count.is_some_and(|c| c > 0);

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
    } else if has_background_work {
        // Background work in progress — Running regardless of prompt state.
        (AgentStatus::Running, None)
    } else if hash_assist_says_running(last_changed_at) {
        // Body content is still mutating off-screen — almost always means
        // the agent is mid-stream and the spinner was simply absent from
        // this capture. Sits above at_prompt because pure text streaming
        // hides the input box (is_prompt_line returns false), which would
        // otherwise drop us straight to the Idle fallback below. Sits
        // BELOW the question / plan-approval checks so explicit Waiting
        // dialogs still win during a recent hash change.
        (AgentStatus::Running, None)
    } else if info.at_prompt {
        let has_recent_notif = db_conn.as_ref().is_some_and(|conn| {
            matches!(
                db::get_latest_notification_by_pane(conn, pane_id),
                Ok(Some(_))
            )
        });
        if has_recent_notif {
            (AgentStatus::Waiting, None)
        } else {
            (AgentStatus::Idle, None)
        }
    } else if info.has_status_running {
        // Status bar shows "(running)" — agent reports active work even
        // though no prompt / spinner is visible (e.g. tool execution).
        (AgentStatus::Running, None)
    } else {
        // No signal at all — TUI not drawn yet (startup splash banner,
        // post-`/clear` redraw, hang). Default to Idle to avoid false
        // Running indications for transient states.
        (AgentStatus::Idle, None)
    };

    AgentDetectionResult {
        status,
        waiting_reason,
        agent_modes: info.agent_modes,
        team_role: info.team_role,
        team_name: info.team_name,
    }
}

fn hash_assist_says_running(last_changed_at: Option<Instant>) -> bool {
    last_changed_at.is_some_and(|t| t.elapsed() < CHANGE_TTL)
}

/// Locate the input box region in a Claude Code pane capture.
///
/// Real Claude layouts place the `│ ... │` input box ABOVE the mode line,
/// usage stats footer, and shortcut hints — i.e., the box is not the
/// bottom-most non-empty content. Walk up from the bottom skipping empty,
/// separator, and footer-noise lines, then expect a `│`-prefixed block and
/// return its contiguous span. `None` means we did not find a box (startup
/// splash, modal dialog covering the box, plain shell after agent exit,
/// etc.) — the hysteresis layer disables hash assist for that cycle rather
/// than risk hashing the user's keystrokes.
pub(super) fn locate_input_region(lines: &[&str]) -> Option<InputRegion> {
    let mut end = lines.len();
    loop {
        if end == 0 {
            return None;
        }
        let idx = end - 1;
        let line = lines[idx];
        let trimmed = line.trim();
        if trimmed.is_empty() || is_separator_line(trimmed) || is_footer_noise(line) {
            end -= 1;
            continue;
        }
        if is_box_border_line(line) {
            break;
        }
        // A non-box, non-footer line below where the input box should be —
        // not a Claude TUI we recognize. Bail.
        return None;
    }
    let last_box = end - 1;
    let mut start = last_box;
    while start > 0 && is_box_border_line(lines[start - 1]) {
        start -= 1;
    }
    Some(InputRegion {
        start_line: start,
        end_line: last_box,
    })
}

fn is_box_border_line(line: &str) -> bool {
    line.trim_start().starts_with('\u{2502}')
}

/// Lines immediately above the input region eligible for footer normalization.
/// Anything further up is conversation body and is hashed verbatim — that way
/// a code block in the chat that mentions e.g. "ctrl+c" never gets stripped.
const FOOTER_SCAN_LINES: usize = 10;

/// Collect the lines that should feed Claude's body hash. The cutoff is the
/// start of the input region when we can locate the `│ ... │` box; when we
/// cannot — and pure text streaming is the canonical case, because the
/// response scrolls the input box off-screen — we hash the entire capture.
/// Returns `None` only when there's nothing to hash (zero non-blank lines)
/// or when the input region is reported as starting at line 0, which means
/// the box ate the whole screen and there is no body left.
///
/// **Safety of the full-content fallback:** Claude Code holds the keyboard
/// while streaming (the TUI does not accept input characters until the
/// response completes), so the user can never be typing into the input box
/// when the box is off-screen. That means hashing the full capture in the
/// streaming case cannot accidentally fold user keystrokes into the body
/// hash — the only thing that can change is generated output, which is
/// exactly the signal hash assist is looking for.
pub(super) fn collect_hashable_body(content: &str) -> Option<Vec<&str>> {
    let lines: Vec<&str> = content.lines().collect();
    let cutoff = match locate_input_region(&lines) {
        Some(region) if region.start_line == 0 => return None,
        Some(region) => region.start_line,
        None => lines.len(),
    };
    if cutoff == 0 {
        return None;
    }
    let scan_start = cutoff.saturating_sub(FOOTER_SCAN_LINES);
    let mut out: Vec<&str> = Vec::with_capacity(cutoff);
    for (idx, line) in lines.iter().enumerate().take(cutoff) {
        if idx >= scan_start && is_footer_noise(line) {
            continue;
        }
        out.push(line);
    }
    Some(out)
}

/// Claude Code mode-line run / pause markers. Used in three places
/// (`is_claude_tui_footer_line`, the team-role detector, `is_prompt_line`)
/// so the literal escapes don't drift across sites.
const MODE_PLAY: char = '\u{23F5}'; // ⏵
const MODE_PAUSE: char = '\u{23F8}'; // ⏸

/// Recognize a line as a built-in Claude Code TUI footer fragment shared
/// between two callers: `is_prompt_line` (which walks past these to look
/// for the real prompt) and `is_footer_noise` (which strips them from the
/// hashable body). Keeping the shared facts in one helper prevents the two
/// callers from silently diverging when Claude's footer changes.
fn is_claude_tui_footer_line(trimmed: &str) -> bool {
    if trimmed.starts_with(MODE_PLAY) || trimmed.starts_with(MODE_PAUSE) {
        return true;
    }
    if trimmed.contains("Context left until auto-compact") {
        return true;
    }
    if trimmed.contains("for shortcuts") {
        return true;
    }
    false
}

/// Recognize a line as periodic-update footer noise from Claude Code's
/// built-in TUI. Intentionally narrow — anything that could be a custom
/// statusline (user-configured model badges, rate-limit clusters, time
/// indicators, etc.) is NOT matched here, so this code stays portable
/// across statusline customizations. The cost of a false negative is a
/// brief false Running blip; the cost of a false positive is missing a
/// real diff and reverting to Idle while the agent is streaming, which is
/// the bug we're fixing.
fn is_footer_noise(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if is_claude_tui_footer_line(trimmed) {
        return true;
    }
    // Claude's ctrl-hint footer rows have the shape "ctrl+X (...) to <verb>"
    // (or "ctrl-X to <verb>"). Requiring both the modifier and the trailing
    // " to " keeps a bare "ctrl+c" inside a conversation code block from
    // being stripped, even when it ends up inside the footer scan window.
    // (is_prompt_line uses the broader, unguarded ctrl+ check below — safe
    // there because that detector only sees the absolute bottom of the
    // screen, where conversation text never lands.)
    if (trimmed.contains("ctrl+") || trimmed.contains("ctrl-")) && trimmed.contains(" to ") {
        return true;
    }
    false
}

/// Claude Code spinner characters that appear at the start of running lines.
const SPINNER_CHARS: &[char] = &['✢', '✽', '✶', '✳', '✻', '·'];

/// Mode detection patterns: (substring to match, label for frontend)
const MODE_PATTERNS: &[(&str, &str)] = &[
    ("plan mode on", "plan"),
    ("bypass permissions on", "bypass"),
    ("accept edits on", "accept"),
    ("auto mode on", "auto"),
];

fn check_claude_pane_content(pane_id: &str, content: Option<&str>) -> ClaudePaneContentInfo {
    let default = ClaudePaneContentInfo {
        has_spinner: false,
        has_status_running: false,
        at_prompt: false,
        has_question_dialog: false,
        has_plan_approval: false,
        shell_count: None,
        local_agent_count: None,
        monitor_count: None,
        fork_count: None,
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

    log::trace!(
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
    let mut shell_count: Option<u32> = None; // set in status_area scan below
    let mut local_agent_count: Option<u32> = None; // set in status_area scan below
    let mut monitor_count: Option<u32> = None; // set in status_area scan below
    let mut fork_count: Option<u32> = None; // set in status_area scan below

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

    // Scan the status bar area (bottom ~7 lines) for background shell count,
    // Agent Teams detection, etc. Limited to 7 lines to avoid false positives
    // from conversation text.
    let status_area = &last_lines[..last_lines.len().min(7)];
    let mut team_role: Option<String> = None;
    let mut team_name: Option<String> = None;

    for line in status_area {
        let trimmed = line.trim();

        // Background shell task detection: "· N shell" (or legacy "· N bash") pattern.
        // Can appear on the mode line ("⏵⏵ bypass permissions on · 1 shell")
        // or as a standalone status line ("1 shell · PR #1381").
        if shell_count.is_none() {
            shell_count = extract_shell_count(trimmed);
        }

        // Background local agent detection: "· N local agent(s)" pattern.
        // Can appear on the mode line ("⏸ plan mode on · 1 local agent")
        // or as a standalone status line ("1 local agent · ...").
        if local_agent_count.is_none() {
            local_agent_count = extract_local_agent_count(trimmed);
        }

        // Background monitor detection: "· N monitor(s)" pattern.
        // Can appear on the mode line ("⏵⏵ auto mode on · 1 monitor")
        // or as a standalone status line ("1 monitor · ...").
        if monitor_count.is_none() {
            monitor_count = extract_monitor_count(trimmed);
        }

        // Background fork detection: subagent picker rows that look like
        //   "◯ Explore  <description>  <duration>"
        // Newer Claude Code surfaces dispatched subagents via this picker
        // beneath the status bar instead of (or alongside) the
        // "· N local agent" mode-line suffix; without this signal a parent
        // that is just waiting for a fork would be classified as Idle.
        if is_fork_picker_row(trimmed) {
            fork_count = Some(fork_count.unwrap_or(0) + 1);
        }

        // Lead: mode line (⏵/⏸) containing "teammate"
        //   e.g., "⏸ plan mode on · 3 teammates"
        if team_role.is_none() {
            let is_mode_line = trimmed.starts_with(MODE_PLAY) || trimmed.starts_with(MODE_PAUSE);
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

    // Add background shell count to agent_modes if detected
    if let Some(count) = shell_count {
        if count > 0 {
            log::debug!(
                "check_claude_pane_content({}): {} background shell task(s) detected",
                pane_id,
                count
            );
            agent_modes.push(format!("{} shell", count));
        }
    }

    // Add local agent count to agent_modes if detected
    if let Some(count) = local_agent_count {
        if count > 0 {
            log::debug!(
                "check_claude_pane_content({}): {} local agent(s) detected",
                pane_id,
                count
            );
            agent_modes.push(format!("{} local agent", count));
        }
    }

    // Add monitor count to agent_modes if detected
    if let Some(count) = monitor_count {
        if count > 0 {
            log::debug!(
                "check_claude_pane_content({}): {} monitor(s) detected",
                pane_id,
                count
            );
            agent_modes.push(format!("{} monitor", count));
        }
    }

    // Add fork count to agent_modes if detected
    if let Some(count) = fork_count {
        if count > 0 {
            log::debug!(
                "check_claude_pane_content({}): {} fork(s) detected",
                pane_id,
                count
            );
            agent_modes.push(format!("{} fork", count));
        }
    }

    ClaudePaneContentInfo {
        has_spinner,
        has_status_running,
        at_prompt,
        has_question_dialog,
        has_plan_approval,
        shell_count,
        local_agent_count,
        monitor_count,
        fork_count,
        agent_modes,
        team_role,
        team_name,
    }
}

/// Check if a line is a row in the subagent fork picker.
/// Rows look like "◯ Explore  <description>  <duration>" and appear in the
/// status-bar area when forks are dispatched. The leading "◯" (U+25EF LARGE
/// CIRCLE) is the discriminator — it marks an actively running fork (vs "⏺"
/// which marks the parent / completed entries).
fn is_fork_picker_row(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    if chars.next() != Some('\u{25EF}') {
        return false;
    }
    // Require a space + at least one more char so a bare "◯" doesn't count.
    matches!(chars.next(), Some(' ')) && chars.next().is_some()
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

const MIDDLE_DOT: &str = "\u{00B7}";
const MIDDLE_DOT_SEP: &str = "\u{00B7} ";

/// Extract background shell task count from a status bar line.
/// Matches both "shell"/"shells" (current) and "bash"/"bashes" (legacy) keywords.
/// Pattern 1 (mode line suffix): "⏵⏵ bypass permissions on · 1 shell" → Some(1)
///   Also tolerates trailing hints, e.g. "· 1 shell · ← for agents".
/// Pattern 2 (standalone line):  "2 shells · PR #1381" → Some(2)
fn extract_shell_count(line: &str) -> Option<u32> {
    extract_count(line, |chunk| parse_keyword_chunk(chunk, is_shell_keyword))
}

fn is_shell_keyword(token: Option<&str>) -> bool {
    matches!(token, Some("shell" | "shells" | "bash" | "bashes"))
}

/// Extract background local agent count from a status bar line.
/// Pattern 1 (mode line suffix): "⏸ plan mode on · 1 local agent" → Some(1)
/// Pattern 2 (standalone line):  "1 local agent · ..." → Some(1)
fn extract_local_agent_count(line: &str) -> Option<u32> {
    extract_count(line, parse_local_agent_chunk)
}

fn parse_local_agent_chunk(chunk: &str) -> Option<u32> {
    let mut parts = chunk.split_whitespace();
    let count = parts.next()?.parse::<u32>().ok()?;
    if parts.next() != Some("local") {
        return None;
    }
    if !matches!(parts.next(), Some("agent" | "agents")) {
        return None;
    }
    is_count_terminator(parts.next()).then_some(count)
}

/// Extract background monitor count from a status bar line.
/// Pattern 1 (mode line suffix): "⏵⏵ auto mode on · 1 monitor" → Some(1)
/// Pattern 2 (standalone line):  "1 monitor · ..." → Some(1)
fn extract_monitor_count(line: &str) -> Option<u32> {
    extract_count(line, |chunk| parse_keyword_chunk(chunk, is_monitor_keyword))
}

fn is_monitor_keyword(token: Option<&str>) -> bool {
    matches!(token, Some("monitor" | "monitors"))
}

/// Parse a chunk shaped as "N <keyword>" where <keyword> is matched by the
/// given predicate. The token after the keyword must be absent or a middle
/// dot — otherwise the line is conversation text (e.g. "· 7 bash commands").
fn parse_keyword_chunk<F>(chunk: &str, is_keyword: F) -> Option<u32>
where
    F: FnOnce(Option<&str>) -> bool,
{
    let mut parts = chunk.split_whitespace();
    let count = parts.next()?.parse::<u32>().ok()?;
    if !is_keyword(parts.next()) {
        return None;
    }
    is_count_terminator(parts.next()).then_some(count)
}

/// Either end-of-chunk or the next "· " separator — both signal that the
/// preceding "N <keyword>" form is complete and not part of a longer phrase.
fn is_count_terminator(token: Option<&str>) -> bool {
    token.is_none() || token == Some(MIDDLE_DOT)
}

/// Walk every "· " separator and try to parse the chunk after each one.
/// Falls back to parsing from the start of the line for the standalone-line
/// form ("2 shells · PR #1381"). Using every position rather than just the
/// last lets us survive trailing hints Claude Code appends to the mode line,
/// e.g. "⏵⏵ bypass permissions on · 1 shell · ← for agents".
fn extract_count<F>(line: &str, parse_chunk: F) -> Option<u32>
where
    F: Fn(&str) -> Option<u32>,
{
    let trimmed = line.trim();
    for (pos, _) in trimmed.match_indices(MIDDLE_DOT_SEP) {
        let after = &trimmed[pos + MIDDLE_DOT_SEP.len()..];
        if let Some(count) = parse_chunk(after) {
            return Some(count);
        }
    }
    parse_chunk(trimmed)
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
        // Built-in Claude TUI footer (mode line, auto-compact warning, "for
        // shortcuts" hint). Single shared predicate so this detector and
        // is_footer_noise can't drift.
        if is_claude_tui_footer_line(trimmed) {
            continue;
        }
        // ctrl shortcut hints (e.g., "ctrl+b ctrl+b (twice) to run in background",
        // "ctrl-g to edit in Nvim"). Unguarded match is safe here because the
        // detector only inspects the absolute bottom of the screen.
        if trimmed.contains("ctrl+") || trimmed.contains("ctrl-") {
            continue;
        }
        // Remaining footer odds and ends not in the shared predicate.
        if trimmed.contains("shift+tab to cycle") || is_file_changes_line(trimmed) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Parent is at the prompt with no spinner, while the subagent picker
    // ("⏺ main" + "◯ <name>  <desc>  <duration>" rows) shows a fork still
    // running. Without fork-row detection this falls through to Idle.
    const FORK_RUNNING_NO_PARENT_SPINNER: &str = "\
❯ /clear
  ⎿  (no content)

❯ Dispatch a task to Explore

⏺ Sure, delegating the claude.rs review to Explore.

────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
❯
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  opus4.7[1m] ◕ │ ctx ○ 4% │ 5h ○ 6%·2h │ 7d ◔ 25%·2d17h
  ⏸ plan mode on (shift+tab to cycle)

  ⏺ main                                                                               ↑/↓ to select · Enter to view
  ◯ Explore  inspect claude.rs                                                                                     9s
";

    #[test]
    fn detects_running_when_fork_is_active_without_parent_spinner() {
        let result = detect_claude_status(&None, "%4", Some(FORK_RUNNING_NO_PARENT_SPINNER), None);
        assert_eq!(
            result.status,
            AgentStatus::Running,
            "fork running should be Running, got modes={:?}",
            result.agent_modes
        );
    }

    #[test]
    fn fork_count_added_to_agent_modes() {
        let result = detect_claude_status(&None, "%4", Some(FORK_RUNNING_NO_PARENT_SPINNER), None);
        assert!(
            result.agent_modes.iter().any(|m| m.ends_with("fork")),
            "expected fork count badge in agent_modes, got: {:?}",
            result.agent_modes
        );
    }

    #[test]
    fn extract_shell_count_handles_trailing_hint() {
        // Claude Code appends "· ← for agents" after the count.
        assert_eq!(
            extract_shell_count(
                "⏵⏵ bypass permissions on \u{00B7} 1 shell \u{00B7} \u{2190} for agents"
            ),
            Some(1)
        );
    }

    #[test]
    fn extract_shell_count_mode_line_suffix() {
        assert_eq!(
            extract_shell_count("⏵⏵ bypass permissions on \u{00B7} 1 shell"),
            Some(1)
        );
    }

    #[test]
    fn extract_shell_count_alongside_local_agent() {
        assert_eq!(
            extract_shell_count("⏵⏵ bypass permissions on \u{00B7} 1 shell \u{00B7} 1 local agent"),
            Some(1)
        );
    }

    #[test]
    fn extract_shell_count_standalone_line() {
        assert_eq!(extract_shell_count("2 shells \u{00B7} PR #1381"), Some(2));
    }

    #[test]
    fn extract_shell_count_ignores_conversation_text() {
        assert_eq!(
            extract_shell_count("⏵⏵ bypass permissions on \u{00B7} 7 bash commands"),
            None
        );
    }

    #[test]
    fn extract_local_agent_count_handles_trailing_hint() {
        assert_eq!(
            extract_local_agent_count(
                "⏸ plan mode on \u{00B7} 1 local agent \u{00B7} \u{2190} for agents"
            ),
            Some(1)
        );
    }

    #[test]
    fn extract_monitor_count_handles_trailing_hint() {
        assert_eq!(
            extract_monitor_count(
                "⏵⏵ auto mode on \u{00B7} 1 monitor \u{00B7} \u{2190} for agents"
            ),
            Some(1)
        );
    }

    /// Pane sitting at an empty prompt with no spinner and no notification —
    /// matches the real Claude TUI layout where the built-in mode line
    /// renders BELOW the input box. Uses only built-in TUI patterns so the
    /// tests don't pin behavior to a specific statusline customization.
    const AT_PROMPT_NO_SPINNER: &str = "\
some history line
another history line
────────────────────────────────────────────────────────────────────────────────
│ ❯                                                                            │
────────────────────────────────────────────────────────────────────────────────
  ⏸ plan mode on (shift+tab to cycle)
";

    #[test]
    fn hash_assist_recent_change_promotes_to_running() {
        let result = detect_claude_status(
            &None,
            "%9",
            Some(AT_PROMPT_NO_SPINNER),
            Some(Instant::now()),
        );
        assert_eq!(
            result.status,
            AgentStatus::Running,
            "recent body change should keep pane Running, got {:?}",
            result.status
        );
    }

    #[test]
    fn hash_assist_stale_falls_through_to_idle() {
        let stale = Instant::now() - CHANGE_TTL - std::time::Duration::from_millis(500);
        let result = detect_claude_status(&None, "%9", Some(AT_PROMPT_NO_SPINNER), Some(stale));
        assert_eq!(
            result.status,
            AgentStatus::Idle,
            "stale body change should let Idle win, got {:?}",
            result.status
        );
    }

    #[test]
    fn hash_assist_none_preserves_legacy_idle() {
        // No hash assist available (first-seen pane / input not found) —
        // judgement must match the pre-change behavior exactly.
        let result = detect_claude_status(&None, "%9", Some(AT_PROMPT_NO_SPINNER), None);
        assert_eq!(result.status, AgentStatus::Idle);
    }

    /// Pure text streaming: spinner happens to be missing from this capture,
    /// and the bottom of the screen is mid-response text (no input box
    /// drawn), so `is_prompt_line` returns false. Without hash assist this
    /// falls through every status check and lands on the Idle fallback,
    /// which is exactly the user-visible bug fixed here.
    const STREAMING_NO_INPUT_BOX: &str = "\
some history line
⏺ Claude is streaming a response and the input box is hidden while text
  pours into the conversation history. Once the spinner glyph briefly
  drops out of view, none of the status-bar markers help either.
1234567890 streaming token output line continues here
1234567890 another generated line filling the bottom of the screen
";

    #[test]
    fn hash_assist_runs_even_when_at_prompt_is_false() {
        let result = detect_claude_status(
            &None,
            "%9",
            Some(STREAMING_NO_INPUT_BOX),
            Some(Instant::now()),
        );
        assert_eq!(
            result.status,
            AgentStatus::Running,
            "streaming content with recent body change must be Running, got {:?}",
            result.status
        );
    }

    #[test]
    fn hash_assist_stale_during_streaming_falls_to_idle_fallback() {
        // After CHANGE_TTL has elapsed and no spinner / prompt is visible,
        // the existing fallback (Idle) takes over. This is the natural
        // recovery path after interrupt with no further activity.
        let stale = Instant::now() - CHANGE_TTL - std::time::Duration::from_millis(500);
        let result = detect_claude_status(&None, "%9", Some(STREAMING_NO_INPUT_BOX), Some(stale));
        assert_eq!(result.status, AgentStatus::Idle);
    }

    #[test]
    fn locate_input_region_finds_box_border_block() {
        let content = "history\n────\n│ first  │\n│ second │";
        let lines: Vec<&str> = content.lines().collect();
        let region = locate_input_region(&lines).expect("box border expected");
        assert_eq!(region.start_line, 2);
        assert_eq!(region.end_line, 3);
    }

    #[test]
    fn locate_input_region_returns_none_without_box_border() {
        let content = "history\nmore history\nplain shell prompt $";
        let lines: Vec<&str> = content.lines().collect();
        assert!(locate_input_region(&lines).is_none());
    }

    #[test]
    fn collect_hashable_body_drops_input_and_footer_inside_scan_window() {
        let content = "history line A\nhistory line B\nhistory line C\nContext left until auto-compact: 8%\n⏵⏵ bypass permissions on · 1 shell\n│ user typed │\n│ second row │\n│             │";
        let body = collect_hashable_body(content).expect("box border present");
        let joined = body.join("\n");
        assert!(joined.contains("history line A"));
        assert!(joined.contains("history line C"));
        assert!(!joined.contains("Context left until auto-compact"));
        assert!(!joined.contains("bypass permissions"));
        assert!(!joined.contains("user typed"));
    }

    #[test]
    fn collect_hashable_body_keeps_footer_lookalikes_outside_scan_window() {
        // "ctrl+" mention at line 0 must survive — input box starts at line
        // 20, so the 10-line footer scan window only covers lines 10..20.
        // This guarantees a code block in the conversation that mentions
        // "ctrl+c" still moves the body hash and counts as activity.
        let mut lines: Vec<String> = vec!["Press ctrl+c to abort".into()];
        for _ in 0..19 {
            lines.push("history filler".into());
        }
        lines.push("│ input │".into());
        lines.push("│ input │".into());
        lines.push("│ input │".into());
        let content = lines.join("\n");
        let body = collect_hashable_body(&content).expect("box border present");
        assert!(
            body.iter().any(|l| l.contains("ctrl+c to abort")),
            "expected conversation body to survive footer scan: {:?}",
            body
        );
    }

    #[test]
    fn collect_hashable_body_keeps_bare_ctrl_mention_inside_scan_window() {
        // The footer-noise predicate requires both "ctrl+" AND " to " — a
        // bare mention without the trailing " to <verb>" hint must NOT be
        // stripped, even when it lands inside the 10-line scan window.
        let lines = [
            "history",
            "use ctrl+c",
            "history",
            "history",
            "history",
            "│ input │",
            "│ input │",
            "│ input │",
        ];
        let content = lines.join("\n");
        let body = collect_hashable_body(&content).expect("box border present");
        assert!(
            body.iter().any(|l| l.contains("use ctrl+c")),
            "bare ctrl+c mention must survive shape-gated footer scan: {:?}",
            body
        );
    }

    #[test]
    fn collect_hashable_body_falls_back_to_full_content_when_box_missing() {
        // Pure text streaming pushes the input box off-screen — there's no
        // `│ ... │` block to find. The detector must still hash whatever
        // text it can see, otherwise hash assist never fires during
        // long-form generation and the pane blinks back to Idle.
        let content = "history A\nhistory B\n⏺ streaming response text appears here\nmore streaming text on this line\nyet more streaming text";
        let body = collect_hashable_body(content).expect("streaming content must hash");
        assert!(body.iter().any(|l| l.contains("streaming response text")));
        assert!(body.iter().any(|l| l.contains("yet more streaming text")));
    }

    #[test]
    fn collect_hashable_body_strips_real_claude_ctrl_hint() {
        // Real Claude footer hint lines have the shape "ctrl+X (...) to <verb>";
        // those MUST still be stripped.
        let lines = [
            "history",
            "history",
            "history",
            "history",
            "ctrl+b to bookmark",
            "│ input │",
            "│ input │",
            "│ input │",
        ];
        let content = lines.join("\n");
        let body = collect_hashable_body(&content).expect("box border present");
        assert!(
            !body.iter().any(|l| l.contains("ctrl+b to bookmark")),
            "shape-matching ctrl hint must be stripped: {:?}",
            body
        );
    }

    #[test]
    fn locate_input_region_finds_box_with_footer_below() {
        // Real Claude layout: built-in mode line and shortcut hints render
        // BELOW the input box. The detector must walk past them before
        // landing on the box border. Uses ONLY Claude built-in TUI patterns
        // (⏸ mode line, ctrl+X to ... hint) — no custom statusline.
        let content = "history A\nhistory B\n────────\n│ ❯ first  │\n│ second    │\n────────\n  ⏸ plan mode on (shift+tab to cycle)\n  ctrl+b to bookmark\n";
        let lines: Vec<&str> = content.lines().collect();
        let region = locate_input_region(&lines).expect("input region expected");
        assert_eq!(region.start_line, 3);
        assert_eq!(region.end_line, 4);
    }
}
