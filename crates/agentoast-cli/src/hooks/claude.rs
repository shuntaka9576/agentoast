use std::io::Read;

use agentoast_shared::{config, models::IconType};
use serde::Deserialize;

use super::{
    collect_git_metadata, emit_result, insert_notification, HookContext, HookResult,
    NotificationPayload,
};

#[derive(Deserialize)]
struct ClaudeHookData {
    hook_event_name: String,
    cwd: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
}

pub fn run() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    let data: ClaudeHookData =
        serde_json::from_str(&input).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let event_key = data
        .notification_type
        .as_deref()
        .unwrap_or(&data.hook_event_name);

    let hook_config = config::load_config().notification.agents.claude_code;

    if !hook_config.events.iter().any(|e| e == event_key) {
        return Ok(());
    }

    let is_stop = data.hook_event_name == "Stop";
    let badge = if is_stop { "Stop" } else { "Notification" };
    let badge_color = if is_stop { "green" } else { "blue" };
    let body = data.message.as_deref().unwrap_or("");
    let force_focus = hook_config.focus_events.iter().any(|e| e == event_key);

    let (repo_name, metadata) = collect_git_metadata(data.cwd.as_deref());
    let ctx = HookContext::from_env();

    insert_notification(
        &ctx,
        &NotificationPayload {
            badge,
            body,
            badge_color,
            icon: &IconType::ClaudeCode,
            metadata: &metadata,
            repo_name: &repo_name,
            force_focus,
        },
    )
}

pub fn handle() {
    let result = match run() {
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
