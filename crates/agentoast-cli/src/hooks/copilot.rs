use std::io::Read;

use agentoast_shared::{config, models::IconType};
use serde::Deserialize;

use super::{
    collect_git_metadata, emit_result, insert_notification, truncate_body, HookContext, HookResult,
    NotificationPayload,
};

/// Copilot CLI hook data received via stdin.
/// All events include at least `timestamp` and `cwd`.
/// The event name is NOT in the JSON — it comes from the `--event` CLI argument.
#[derive(Deserialize)]
struct CopilotHookData {
    cwd: Option<String>,
    /// Path to the session transcript (events.jsonl) — present in agentStop/subagentStop
    #[serde(rename = "transcriptPath")]
    transcript_path: Option<String>,
    /// errorOccurred event includes an error object
    error: Option<CopilotError>,
}

#[derive(Deserialize)]
struct CopilotError {
    message: Option<String>,
}

/// A single line in events.jsonl
#[derive(Deserialize)]
struct TranscriptEntry {
    #[serde(rename = "type")]
    entry_type: String,
    data: Option<TranscriptData>,
}

#[derive(Deserialize)]
struct TranscriptData {
    content: Option<String>,
}

/// Read the last assistant message from the events.jsonl transcript file.
/// Scans from the end of the file, looking for the last `assistant.message` entry.
fn get_last_assistant_message(path: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines().rev() {
        if line.is_empty() {
            continue;
        }
        let entry: TranscriptEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.entry_type == "assistant.message" {
            return entry.data.and_then(|d| d.content);
        }
    }
    None
}

pub fn run(event_name: &str) -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    let data: CopilotHookData =
        serde_json::from_str(&input).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let hook_config = config::load_config().notification.agents.copilot_cli;

    if !hook_config.events.iter().any(|e| e == event_name) {
        return Ok(());
    }

    let force_focus = hook_config.focus_events.iter().any(|e| e == event_name);

    let (repo_name, metadata) = collect_git_metadata(data.cwd.as_deref());

    let (badge, badge_color, body) = match event_name {
        "agentStop" | "subagentStop" => {
            let body = if hook_config.include_body {
                data.transcript_path
                    .as_deref()
                    .and_then(get_last_assistant_message)
                    .map(|m| truncate_body(&m))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            ("Stop", "green", body)
        }
        "errorOccurred" => {
            let msg = if hook_config.include_body {
                data.error
                    .and_then(|e| e.message)
                    .map(|m| truncate_body(&m))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            ("Error", "red", msg)
        }
        _ => ("Notification", "blue", String::new()),
    };

    let ctx = HookContext::from_env();

    insert_notification(
        &ctx,
        &NotificationPayload {
            badge,
            body: &body,
            badge_color,
            icon: &IconType::CopilotCli,
            metadata: &metadata,
            repo_name: &repo_name,
            force_focus,
        },
    )
}

pub fn handle(event_name: &str) {
    let result = match run(event_name) {
        Ok(()) => HookResult {
            success: true,
            error: None,
        },
        Err(e) => HookResult {
            success: false,
            error: Some(e),
        },
    };
    emit_result(result);
}
