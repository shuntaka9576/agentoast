use std::io::Read;

use agentoast_shared::{config, models::IconType};
use serde::Deserialize;

use super::{
    collect_git_metadata, emit_result, insert_notification, truncate_body, HookContext, HookResult,
    NotificationPayload,
};

#[derive(Deserialize)]
struct ClaudeHookData {
    hook_event_name: String,
    cwd: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
    last_assistant_message: Option<String>,
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

    let force_focus = hook_config.focus_events.iter().any(|e| e == event_key);

    let (badge, badge_color, body) = match data.hook_event_name.as_str() {
        "Stop" => {
            let body = if hook_config.include_body {
                data.last_assistant_message
                    .as_deref()
                    .map(truncate_body)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            ("Stop", "green", body)
        }
        _ => {
            let body = data.message.unwrap_or_default();
            ("Notification", "blue", body)
        }
    };

    let (repo_name, metadata) = collect_git_metadata(data.cwd.as_deref());
    let ctx = HookContext::from_env();

    insert_notification(
        &ctx,
        &NotificationPayload {
            badge,
            body: &body,
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
