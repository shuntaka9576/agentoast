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
/// Reads the tail of the file to avoid loading the entire file into memory.
///
/// events.jsonl writes are asynchronous — the latest assistant.message may not
/// be flushed when agentStop fires. When the file tail ends with
/// `assistant.turn_start` (turn in progress, message not yet written),
/// retries up to 5 times with 100ms intervals.
fn get_last_assistant_message(path: &str) -> Option<String> {
    const MAX_RETRIES: u32 = 5;
    const RETRY_INTERVAL_MS: u64 = 100;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS));
        }
        let tail = read_tail(path)?;
        let last_type = find_last_event_type(&tail);

        // If last entry is assistant.turn_start, the message hasn't been written yet — retry
        if last_type.as_deref() == Some("assistant.turn_start") && attempt < MAX_RETRIES {
            continue;
        }

        return find_last_assistant_content(&tail);
    }
    None
}

/// Read the tail of a file (last 8KB).
fn read_tail(path: &str) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len == 0 {
        return None;
    }
    const TAIL_SIZE: u64 = 8 * 1024;
    let start = file_len.saturating_sub(TAIL_SIZE);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    Some(buf)
}

/// Find the `type` field of the last parseable entry in the tail.
fn find_last_event_type(tail: &str) -> Option<String> {
    for line in tail.lines().rev() {
        if line.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) {
            return Some(entry.entry_type);
        }
    }
    None
}

/// Find the content of the last `assistant.message` entry in the tail.
fn find_last_assistant_content(tail: &str) -> Option<String> {
    for line in tail.lines().rev() {
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
