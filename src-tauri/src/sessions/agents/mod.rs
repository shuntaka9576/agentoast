use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Differentiates marker lines across calls (and across processes via the
/// pid) so stale pane content scrolled from a previous capture can never be
/// mistaken for this call's markers.
static BATCH_SEQ: AtomicU64 = AtomicU64::new(0);

/// Keep each tmux invocation comfortably below ARG_MAX even with hundreds of
/// panes (~12 args / ~80 bytes per pane).
const BATCH_CHUNK: usize = 100;

/// Capture the content of many panes with a single `tmux` invocation by
/// chaining `display-message`/`capture-pane` commands with `;`.
///
/// tmux aborts a command chain at the first failing command (verified on
/// tmux 3.x: commands before the failure run, the rest are dropped), so each
/// capture is bracketed by BEGIN/END markers. A pane whose BEGIN appeared but
/// END did not is the one that vanished (→ None); panes whose BEGIN never
/// appeared were cut off by the abort and are retried in a follow-up
/// invocation without the dead pane. Steady state is exactly one spawn; each
/// vanished pane costs one extra.
pub(crate) fn capture_panes_batch(pane_ids: &[&str]) -> HashMap<String, Option<String>> {
    let mut result: HashMap<String, Option<String>> =
        pane_ids.iter().map(|id| (id.to_string(), None)).collect();
    if pane_ids.is_empty() {
        return result;
    }
    let Some(tmux_path) = find_tmux() else {
        return result;
    };

    let nonce = format!(
        "AGENTOAST_CAP_{}_{}",
        std::process::id(),
        BATCH_SEQ.fetch_add(1, Ordering::Relaxed)
    );

    for chunk in pane_ids.chunks(BATCH_CHUNK) {
        let mut remaining: Vec<&str> = chunk.to_vec();
        while !remaining.is_empty() {
            let mut cmd = Command::new(&tmux_path);
            cmd.env_remove("TMPDIR");
            for (i, id) in remaining.iter().enumerate() {
                if i > 0 {
                    cmd.arg(";");
                }
                // display-message expands the message as a tmux format, where
                // a bare `%` is consumed (verified: `%1` prints as `1`), so
                // escape the pane id's `%` as `%%` to keep markers literal.
                let escaped = id.replace('%', "%%");
                cmd.args([
                    "display-message",
                    "-p",
                    &format!("{}_BEGIN_{}", nonce, escaped),
                ]);
                cmd.arg(";");
                cmd.args(["capture-pane", "-p", "-t", id]);
                cmd.arg(";");
                cmd.args([
                    "display-message",
                    "-p",
                    &format!("{}_END_{}", nonce, escaped),
                ]);
            }
            // A non-zero exit only means the chain hit a dead pane; whatever
            // ran before the abort is still on stdout, so always parse it.
            let output = match cmd.output() {
                Ok(o) => o,
                Err(e) => {
                    log::warn!("capture_panes_batch: tmux spawn failed: {}", e);
                    return result;
                }
            };
            crate::sessions::note_spawn();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed = parse_batch_output(&stdout, &nonce);

            let mut progressed = false;
            remaining.retain(|id| {
                match parsed.get(*id) {
                    Some(PaneSection::Complete(content)) => {
                        result.insert(id.to_string(), Some(content.clone()));
                        progressed = true;
                        false
                    }
                    Some(PaneSection::Truncated) => {
                        // BEGIN without END: this is the pane the chain died
                        // on — it disappeared between list-panes and now.
                        log::debug!("capture_panes_batch: pane {} vanished, skipping", id);
                        progressed = true;
                        false
                    }
                    None => true, // cut off by the abort — retry next round
                }
            });
            if !progressed {
                // No marker came back at all (tmux server gone?). Bail rather
                // than loop.
                log::warn!(
                    "capture_panes_batch: no progress, {} pane(s) uncaptured (stderr: {})",
                    remaining.len(),
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                break;
            }
        }
    }
    result
}

enum PaneSection {
    /// BEGIN and END both seen; holds the lines in between.
    Complete(String),
    /// BEGIN seen but END missing — the chain aborted inside this capture.
    Truncated,
}

/// Split a batched invocation's stdout back into per-pane sections.
fn parse_batch_output(stdout: &str, nonce: &str) -> HashMap<String, PaneSection> {
    let begin_prefix = format!("{}_BEGIN_", nonce);
    let end_prefix = format!("{}_END_", nonce);

    let mut sections: HashMap<String, PaneSection> = HashMap::new();
    let mut current: Option<(String, Vec<&str>)> = None;

    for line in stdout.lines() {
        if let Some(id) = line.strip_prefix(&begin_prefix) {
            if let Some((prev_id, _)) = current.take() {
                sections.insert(prev_id, PaneSection::Truncated);
            }
            current = Some((id.to_string(), Vec::new()));
        } else if let Some(id) = line.strip_prefix(&end_prefix) {
            match current.take() {
                Some((open_id, lines)) if open_id == id => {
                    sections.insert(open_id, PaneSection::Complete(lines.join("\n")));
                }
                other => {
                    // Mismatched END (marker-looking pane content); restore state.
                    current = other;
                }
            }
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        }
    }
    if let Some((open_id, _)) = current {
        sections.insert(open_id, PaneSection::Truncated);
    }
    sections
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

#[cfg(test)]
mod batch_tests {
    use super::*;

    const NONCE: &str = "AGENTOAST_CAP_1_0";

    #[test]
    fn parses_complete_sections() {
        let stdout = "AGENTOAST_CAP_1_0_BEGIN_%1\nline a\nline b\nAGENTOAST_CAP_1_0_END_%1\nAGENTOAST_CAP_1_0_BEGIN_%2\n\nAGENTOAST_CAP_1_0_END_%2\n";
        let parsed = parse_batch_output(stdout, NONCE);
        assert!(
            matches!(parsed.get("%1"), Some(PaneSection::Complete(c)) if c == "line a\nline b")
        );
        assert!(matches!(parsed.get("%2"), Some(PaneSection::Complete(c)) if c.is_empty()));
    }

    #[test]
    fn aborted_chain_marks_truncated_and_omits_rest() {
        // Chain died while capturing %2: BEGIN printed, capture aborted the
        // chain, so neither END_%2 nor any %3 marker appears.
        let stdout = "AGENTOAST_CAP_1_0_BEGIN_%1\ncontent\nAGENTOAST_CAP_1_0_END_%1\nAGENTOAST_CAP_1_0_BEGIN_%2\n";
        let parsed = parse_batch_output(stdout, NONCE);
        assert!(matches!(parsed.get("%1"), Some(PaneSection::Complete(_))));
        assert!(matches!(parsed.get("%2"), Some(PaneSection::Truncated)));
        assert!(!parsed.contains_key("%3"));
    }

    #[test]
    fn mismatched_end_marker_is_treated_as_content() {
        // An END for a different pane id inside a section must not close it.
        let stdout = "AGENTOAST_CAP_1_0_BEGIN_%1\nAGENTOAST_CAP_1_0_END_%9\nreal line\nAGENTOAST_CAP_1_0_END_%1\n";
        let parsed = parse_batch_output(stdout, NONCE);
        match parsed.get("%1") {
            Some(PaneSection::Complete(c)) => assert!(c.contains("real line")),
            _ => panic!("expected complete section"),
        }
    }
}
