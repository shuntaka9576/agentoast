use agentoast_shared::{config, models::IconType};
use serde::Deserialize;

use super::{
    collect_git_metadata, emit_result, insert_notification, HookContext, HookResult,
    NotificationPayload,
};

#[derive(Deserialize)]
struct OpenCodeHookData {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    properties: serde_json::Value,
    directory: Option<String>,
}

pub fn run(json_arg: &str) -> Result<(), String> {
    let data: OpenCodeHookData =
        serde_json::from_str(json_arg).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let hook_config = config::load_config().notification.agents.opencode;

    if !hook_config.events.iter().any(|e| e == &data.event_type) {
        return Ok(());
    }

    // For session.status, only notify on the idle sub-type
    if data.event_type == "session.status" {
        let is_idle = data
            .properties
            .get("status")
            .and_then(|s| s.get("type"))
            .and_then(|t| t.as_str())
            == Some("idle");
        if !is_idle {
            return Ok(());
        }
    }

    let (badge, badge_color) = match data.event_type.as_str() {
        "session.status" => ("Stop", "green"),
        "session.error" => ("Error", "red"),
        "permission.asked" => ("Permission", "blue"),
        _ => ("Notification", "gray"),
    };

    let force_focus = hook_config
        .focus_events
        .iter()
        .any(|e| e == &data.event_type);

    let (repo_name, metadata) = collect_git_metadata(data.directory.as_deref());
    let ctx = HookContext::from_env();

    insert_notification(
        &ctx,
        &NotificationPayload {
            badge,
            body: "",
            badge_color,
            icon: &IconType::OpenCode,
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
