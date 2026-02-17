use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IconType {
    Agentoast,
    ClaudeCode,
    Codex,
    OpenCode,
}

impl IconType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IconType::Agentoast => "agentoast",
            IconType::ClaudeCode => "claude-code",
            IconType::Codex => "codex",
            IconType::OpenCode => "opencode",
        }
    }
}

impl FromStr for IconType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "agentoast" => Ok(IconType::Agentoast),
            "claude-code" => Ok(IconType::ClaudeCode),
            "codex" => Ok(IconType::Codex),
            "opencode" => Ok(IconType::OpenCode),
            _ => Err(format!("Invalid icon: {}", s)),
        }
    }
}

impl std::fmt::Display for IconType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub color: String,
    pub icon: String,
    pub group_name: String,
    pub metadata: HashMap<String, String>,
    pub tmux_pane: String,
    pub terminal_bundle_id: String,
    pub force_focus: bool,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationGroup {
    pub group_name: String,
    pub notifications: Vec<Notification>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmuxPane {
    pub pane_id: String,
    pub pane_pid: u32,
    pub session_name: String,
    pub window_name: String,
    pub current_path: String,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TmuxPaneGroup {
    pub repo_name: String,
    pub current_path: String,
    pub panes: Vec<TmuxPane>,
}
