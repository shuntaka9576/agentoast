pub mod claude;
pub mod codex;
pub mod opencode;

use std::collections::HashMap;
use std::path::Path;

use agentoast_shared::{config, db, models::IconType};
use serde::Serialize;

pub struct GitInfo {
    pub repo_name: String,
    pub branch_name: String,
}

pub fn get_git_info(cwd: &Path) -> GitInfo {
    let mut repo_name = String::new();
    let mut branch_name = String::new();

    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    let is_git_repo = git_check
        .as_ref()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false);

    if is_git_repo {
        if let Ok(output) = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Some(caps) = url.rsplit('/').next().or_else(|| url.rsplit(':').next()) {
                    repo_name = caps.trim_end_matches(".git").to_string();
                }
            }
        }

        if repo_name.is_empty() {
            repo_name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
        }

        if let Ok(output) = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            if output.status.success() {
                branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
    } else {
        repo_name = cwd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    GitInfo {
        repo_name,
        branch_name,
    }
}

#[derive(Serialize)]
pub struct HookResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn parse_metadata(meta_args: &[String]) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    for entry in meta_args {
        if let Some((key, value)) = entry.split_once('=') {
            metadata.insert(key.to_string(), value.to_string());
        } else {
            eprintln!(
                "Warning: ignoring invalid metadata entry '{}' (expected KEY=VALUE)",
                entry
            );
        }
    }
    metadata
}

/// Runtime context resolved from environment variables
pub struct HookContext {
    pub tmux_pane: String,
    pub terminal_bundle_id: String,
}

impl HookContext {
    pub fn from_env() -> Self {
        HookContext {
            tmux_pane: std::env::var("TMUX_PANE").unwrap_or_default(),
            terminal_bundle_id: std::env::var("__CFBundleIdentifier").unwrap_or_default(),
        }
    }
}

/// Resolves git info from the given working directory and returns (repo_name, metadata)
pub fn collect_git_metadata(cwd_opt: Option<&str>) -> (String, HashMap<String, String>) {
    let mut metadata = HashMap::new();
    let repo_name = if let Some(cwd_str) = cwd_opt {
        let git_info = get_git_info(Path::new(cwd_str));
        if !git_info.branch_name.is_empty() {
            metadata.insert("branch".to_string(), git_info.branch_name);
        }
        git_info.repo_name
    } else {
        String::new()
    };
    (repo_name, metadata)
}

/// Serializes a HookResult as JSON and writes it to stdout
pub fn emit_result(result: HookResult) {
    println!(
        "{}",
        serde_json::to_string(&result).unwrap_or_else(|_| r#"{"success":false}"#.to_string())
    );
}

/// Parameters for inserting a hook notification
pub struct NotificationPayload<'a> {
    pub badge: &'a str,
    pub body: &'a str,
    pub badge_color: &'a str,
    pub icon: &'a IconType,
    pub metadata: &'a HashMap<String, String>,
    pub repo_name: &'a str,
    pub force_focus: bool,
}

/// Opens a DB connection and inserts a notification
pub fn insert_notification(ctx: &HookContext, p: &NotificationPayload) -> Result<(), String> {
    let db_path = config::db_path();
    let conn = db::open_reader(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;
    db::insert_notification(
        &conn,
        &db::NotificationInput {
            badge: p.badge,
            body: p.body,
            badge_color: p.badge_color,
            icon: p.icon,
            metadata: p.metadata,
            repo: p.repo_name,
            tmux_pane: &ctx.tmux_pane,
            terminal_bundle_id: &ctx.terminal_bundle_id,
            force_focus: p.force_focus,
        },
    )
    .map(|_| ())
    .map_err(|e| format!("Failed to insert notification: {}", e))
}
