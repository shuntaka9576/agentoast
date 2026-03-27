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
    /// errorOccurred event includes an error object
    error: Option<CopilotError>,
}

#[derive(Deserialize)]
struct CopilotError {
    message: Option<String>,
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
        "agentStop" | "subagentStop" => ("Stop", "green", String::new()),
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
