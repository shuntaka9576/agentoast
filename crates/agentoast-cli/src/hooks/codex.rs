use agentoast_shared::{config, models::IconType};
use serde::Deserialize;

use super::{
    collect_git_metadata, emit_result, insert_notification, HookContext, HookResult,
    NotificationPayload,
};

#[derive(Deserialize)]
struct CodexHookData {
    #[serde(rename = "type")]
    event_type: String,
    cwd: Option<String>,
    #[serde(rename = "last-assistant-message")]
    last_assistant_message: Option<String>,
}

const BODY_MAX_LEN: usize = 200;

fn truncate_body(msg: &str) -> String {
    if msg.len() <= BODY_MAX_LEN {
        return msg.to_string();
    }
    let truncate_at = msg
        .char_indices()
        .take_while(|(i, _)| *i <= BODY_MAX_LEN)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut truncated = msg[..truncate_at].to_string();
    truncated.push_str("...");
    truncated
}

pub fn run(json_arg: &str) -> Result<(), String> {
    let data: CodexHookData =
        serde_json::from_str(json_arg).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let hook_config = config::load_config().notification.agents.codex;

    if !hook_config.events.iter().any(|e| e == &data.event_type) {
        return Ok(());
    }

    let badge = "Stop";
    let badge_color = "green";
    let body = if hook_config.include_body {
        data.last_assistant_message
            .as_deref()
            .map(truncate_body)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let force_focus = hook_config
        .focus_events
        .iter()
        .any(|e| e == &data.event_type);

    let (repo_name, metadata) = collect_git_metadata(data.cwd.as_deref());
    let ctx = HookContext::from_env();

    insert_notification(
        &ctx,
        &NotificationPayload {
            badge,
            body: &body,
            badge_color,
            icon: &IconType::Codex,
            metadata: &metadata,
            repo_name: &repo_name,
            force_focus,
        },
    )
}

pub fn handle(json: &str) {
    let result = match run(json) {
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
