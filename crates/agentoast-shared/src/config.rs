use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use toml_edit::DocumentMut;

/// Return XDG_DATA_HOME/agentoast.
/// The `dirs` crate returns ~/Library/Application Support on macOS,
/// so we construct ~/.local/share directly for XDG compliance.
pub fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("agentoast")
    } else {
        let home = std::env::var("HOME").expect("HOME not set");
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("agentoast")
    }
}

/// Return the path to the SQLite DB file.
pub fn db_path() -> PathBuf {
    data_dir().join("notifications.db")
}

/// Marker file written when onboarding has been completed.
pub fn onboarded_marker_path() -> PathBuf {
    data_dir().join(".onboarded")
}

/// Whether the user has completed the onboarding flow.
pub fn is_onboarded() -> bool {
    onboarded_marker_path().exists()
}

/// Persist the "onboarding complete" flag by creating the marker file.
pub fn mark_onboarded() -> io::Result<()> {
    let path = onboarded_marker_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, b"")
}

/// Return XDG_CONFIG_HOME/agentoast.
pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("agentoast")
    } else {
        let home = std::env::var("HOME").expect("HOME not set");
        PathBuf::from(home).join(".config").join("agentoast")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    pub editor: Option<String>,
    #[serde(default)]
    pub toast: ToastConfig,
    #[serde(default)]
    pub notification: NotificationConfig,
    #[serde(default)]
    pub keybinding: KeybindingConfig,
    #[serde(default)]
    pub update: UpdateConfig,
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub apps: AppsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppsConfig {
    #[serde(default)]
    pub allowed_apps: Vec<AllowedApp>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AllowedApp {
    #[serde(rename = "bundleId", alias = "bundle_id")]
    pub bundle_id: String,
    #[serde(rename = "displayName", alias = "display_name")]
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ToastPosition {
    /// The canonical kebab-case string used in config.toml and IPC payloads.
    /// Must stay in lockstep with the `rename_all = "kebab-case"` serde attr
    /// above — keep them together so changes are obvious.
    pub fn as_str(self) -> &'static str {
        match self {
            ToastPosition::TopLeft => "top-left",
            ToastPosition::TopRight => "top-right",
            ToastPosition::BottomLeft => "bottom-left",
            ToastPosition::BottomRight => "bottom-right",
        }
    }
}

/// Which screens a toast appears on. `Active` follows the cursor's screen
/// (historical behavior); `All` mirrors the toast onto every attached screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastDisplay {
    #[default]
    Active,
    All,
}

impl ToastDisplay {
    /// The canonical kebab-case string used in config.toml and IPC payloads.
    /// Must stay in lockstep with the `rename_all = "kebab-case"` serde attr
    /// above — keep them together so changes are obvious.
    pub fn as_str(self) -> &'static str {
        match self {
            ToastDisplay::Active => "active",
            ToastDisplay::All => "all",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToastConfig {
    #[serde(default = "default_toast_duration")]
    pub duration_ms: u64,
    #[serde(default)]
    pub persistent: bool,
    #[serde(default = "default_toast_positions")]
    pub positions: Vec<ToastPosition>,
    #[serde(default)]
    pub display: ToastDisplay,
}

impl Default for ToastConfig {
    fn default() -> Self {
        Self {
            duration_ms: default_toast_duration(),
            persistent: false,
            positions: default_toast_positions(),
            display: ToastDisplay::default(),
        }
    }
}

fn default_toast_positions() -> Vec<ToastPosition> {
    vec![ToastPosition::TopRight]
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_filter_notified_only")]
    pub filter_notified_only: bool,
    #[serde(default = "default_show_non_agent_panes")]
    pub show_non_agent_panes: bool,
    #[serde(default)]
    pub agents: AgentsConfig,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            muted: false,
            filter_notified_only: default_filter_notified_only(),
            show_non_agent_panes: default_show_non_agent_panes(),
            agents: AgentsConfig::default(),
        }
    }
}

fn default_filter_notified_only() -> bool {
    false
}

fn default_show_non_agent_panes() -> bool {
    false
}

fn default_toast_duration() -> u64 {
    4000
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeybindingConfig {
    #[serde(default = "default_toggle_panel")]
    pub toggle_panel: String,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            toggle_panel: default_toggle_panel(),
        }
    }
}

fn default_toggle_panel() -> String {
    "super+ctrl+n".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentsConfig {
    #[serde(default)]
    pub claude_code: ClaudeCodeHookConfig,
    #[serde(default)]
    pub codex: CodexHookConfig,
    #[serde(default)]
    pub copilot_cli: CopilotCliHookConfig,
    #[serde(default)]
    pub opencode: OpenCodeHookConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeCodeHookConfig {
    #[serde(default = "default_claude_code_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub focus_events: Vec<String>,
    #[serde(default = "default_true")]
    pub include_body: bool,
}

impl Default for ClaudeCodeHookConfig {
    fn default() -> Self {
        Self {
            events: default_claude_code_events(),
            focus_events: Vec::new(),
            include_body: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexHookConfig {
    #[serde(default = "default_codex_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub focus_events: Vec<String>,
    #[serde(default = "default_true")]
    pub include_body: bool,
}

impl Default for CodexHookConfig {
    fn default() -> Self {
        Self {
            events: default_codex_events(),
            focus_events: Vec::new(),
            include_body: true,
        }
    }
}

fn default_codex_events() -> Vec<String> {
    vec!["agent-turn-complete".to_string()]
}

#[derive(Debug, Clone, Deserialize)]
pub struct CopilotCliHookConfig {
    #[serde(default = "default_copilot_cli_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub focus_events: Vec<String>,
    #[serde(default = "default_true")]
    pub include_body: bool,
}

impl Default for CopilotCliHookConfig {
    fn default() -> Self {
        Self {
            events: default_copilot_cli_events(),
            focus_events: Vec::new(),
            include_body: true,
        }
    }
}

fn default_copilot_cli_events() -> Vec<String> {
    vec!["agentStop".to_string()]
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenCodeHookConfig {
    #[serde(default = "default_opencode_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub focus_events: Vec<String>,
}

impl Default for OpenCodeHookConfig {
    fn default() -> Self {
        Self {
            events: default_opencode_events(),
            focus_events: Vec::new(),
        }
    }
}

fn default_opencode_events() -> Vec<String> {
    vec![
        "session.status".to_string(),
        "session.error".to_string(),
        "permission.asked".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SystemConfig {
    pub tmux: Option<String>,
    // `git` used to live here as a binary-path override. Repo/branch info is
    // now read straight from `.git` metadata (git_info.rs), so the key is
    // gone; configs that still contain `git = "..."` parse fine because
    // unknown fields are ignored (no deny_unknown_fields).
}

fn default_claude_code_events() -> Vec<String> {
    vec![
        "Stop".to_string(),
        "permission_prompt".to_string(),
        "auth_success".to_string(),
        "elicitation_dialog".to_string(),
    ]
}

/// Return the path to config.toml.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Load config.toml. Return defaults if the file is missing or fails to parse.
pub fn load_config() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            log::warn!("Failed to parse config.toml: {}, using defaults", e);
            AppConfig::default()
        }),
        Err(_) => AppConfig::default(),
    }
}

/// Update [notification] muted in config.toml, preserving existing comments and formatting.
pub fn save_notification_muted(muted: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["notification"]["muted"] = toml_edit::value(muted);
    std::fs::write(&path, doc.to_string())
}

/// Update [notification] filter_notified_only in config.toml, preserving existing comments and formatting.
pub fn save_notification_filter_notified_only(value: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["notification"]["filter_notified_only"] = toml_edit::value(value);
    std::fs::write(&path, doc.to_string())
}

/// Update [notification] show_non_agent_panes in config.toml, preserving existing comments and formatting.
pub fn save_notification_show_non_agent_panes(value: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["notification"]["show_non_agent_panes"] = toml_edit::value(value);
    std::fs::write(&path, doc.to_string())
}

/// Update [toast] duration_ms in config.toml, preserving existing comments and formatting.
pub fn save_toast_duration_ms(value: u64) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["toast"]["duration_ms"] = toml_edit::value(value as i64);
    std::fs::write(&path, doc.to_string())
}

/// Update [toast] persistent in config.toml, preserving existing comments and formatting.
pub fn save_toast_persistent(value: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["toast"]["persistent"] = toml_edit::value(value);
    std::fs::write(&path, doc.to_string())
}

/// Update [toast] positions in config.toml, preserving existing comments and formatting.
pub fn save_toast_positions(values: &[ToastPosition]) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    let mut arr = toml_edit::Array::new();
    for v in values {
        arr.push(v.as_str());
    }
    doc["toast"]["positions"] = toml_edit::value(arr);
    std::fs::write(&path, doc.to_string())
}

/// Update [toast] display in config.toml, preserving existing comments and formatting.
pub fn save_toast_display(value: ToastDisplay) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["toast"]["display"] = toml_edit::value(value.as_str());
    std::fs::write(&path, doc.to_string())
}

/// Update [keybinding] toggle_panel in config.toml, preserving existing comments and formatting.
pub fn save_keybinding_toggle_panel(value: &str) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["keybinding"]["toggle_panel"] = toml_edit::value(value);
    std::fs::write(&path, doc.to_string())
}

/// Update [update] enabled in config.toml, preserving existing comments and formatting.
#[allow(dead_code)] // Settings UI hides this toggle; kept for programmatic callers.
pub fn save_update_enabled(value: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["update"]["enabled"] = toml_edit::value(value);
    std::fs::write(&path, doc.to_string())
}

/// Update [apps] allowed_apps in config.toml, preserving existing comments and formatting.
pub fn save_apps_allowed_apps(apps: &[AllowedApp]) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();

    let mut array = toml_edit::Array::new();
    for app in apps {
        let mut entry = toml_edit::InlineTable::new();
        entry.insert("bundle_id", toml_edit::Value::from(app.bundle_id.clone()));
        entry.insert(
            "display_name",
            toml_edit::Value::from(app.display_name.clone()),
        );
        array.push(toml_edit::Value::from(entry));
    }

    if !doc.contains_table("apps") {
        doc["apps"] = toml_edit::table();
    }
    doc["apps"]["allowed_apps"] = toml_edit::value(array);
    std::fs::write(&path, doc.to_string())
}

/// Update top-level `editor` in config.toml, preserving existing comments and formatting.
/// Empty string removes the key so that `$EDITOR` / `vim` fallback applies.
pub fn save_editor(value: &str) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    if value.is_empty() {
        doc.remove("editor");
    } else {
        doc["editor"] = toml_edit::value(value);
    }
    std::fs::write(&path, doc.to_string())
}

/// Default config.toml template.
fn default_config_template() -> &'static str {
    r#"# agentoast configuration

# Editor to open when running `agentoast config`
# Falls back to $EDITOR environment variable, then vim
# editor = "vim"

# Toast popup notification
[toast]
# Display duration in milliseconds (default: 4000)
# duration_ms = 4000

# Keep toast visible until clicked (default: false)
# persistent = false

# Where toast notifications appear on the active screen.
# Multiple positions show the same toast in each corner simultaneously.
# Valid values: "top-left", "top-right", "bottom-left", "bottom-right"
# positions = ["top-right"]

# Which screens show toasts (default: "active")
# "active" = only the screen the cursor is on, "all" = every attached screen
# display = "active"

# Notification settings
[notification]
# Mute all notifications (default: false)
# muted = false

# Show only groups with notifications (default: false)
# filter_notified_only = false

# Show tmux panes without an AI coding agent (default: false)
# show_non_agent_panes = false

# Claude Code agent settings
[notification.agents.claude_code]
# Events that trigger notifications
# Available: Stop, permission_prompt, idle_prompt, auth_success, elicitation_dialog, TeammateIdle, TaskCompleted
# idle_prompt, TeammateIdle, TaskCompleted are excluded by default
# events = ["Stop", "permission_prompt", "auth_success", "elicitation_dialog"]

# Events that auto-focus the terminal (default: none)
# These events set force_focus=true, causing silent terminal focus without toast (when not muted)
# focus_events = []

# Include last-assistant-message as notification body (default: true, truncated to 200 chars)
# include_body = true

# Codex agent settings
[notification.agents.codex]
# Events that trigger notifications
# Available: agent-turn-complete
# events = ["agent-turn-complete"]

# Events that auto-focus the terminal (default: none)
# focus_events = []

# Include last-assistant-message as notification body (default: true, truncated to 200 chars)
# include_body = true

# OpenCode agent settings
[notification.agents.opencode]
# Events that trigger notifications
# Available: session.status (idle only), session.error, permission.asked
# events = ["session.status", "session.error", "permission.asked"]

# Events that auto-focus the terminal (default: none)
# focus_events = []

# Copilot CLI agent settings
[notification.agents.copilot_cli]
# Events that trigger notifications
# Available: agentStop, subagentStop, errorOccurred
# events = ["agentStop"]

# Events that auto-focus the terminal (default: none)
# focus_events = []

# Include error message as notification body (default: true, truncated to 200 chars)
# include_body = true

# Keyboard shortcuts
[keybinding]
# Shortcut to toggle the notification panel (default: super+ctrl+n)
# Format: modifier+key (modifiers: ctrl, shift, alt/option, super/cmd)
# Set to "" to disable
# toggle_panel = "super+ctrl+n"

# System settings
# Override the auto-detected tmux binary path (useful when auto-detection fails)
# [system]
# tmux = "/custom/path/to/tmux"

# Apps tab — surface frequently-used applications inside the main panel
# Add apps from Settings → Apps. Each entry pins the app to the Apps tab so a
# single click brings it to the front. Defaults to empty (Apps tab is empty).
# [apps]
# allowed_apps = [
#   { bundle_id = "com.google.Chrome",          display_name = "Google Chrome" },
#   { bundle_id = "com.tinyspeck.slackmacgap",  display_name = "Slack" },
# ]

"#
}

/// Create config.toml with the default template if it does not exist. Return its path.
pub fn ensure_config_file() -> io::Result<PathBuf> {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, default_config_template())?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_system_git_key_is_ignored() {
        // `[system] git` was removed when repo info became file-read based;
        // configs written by older versions must keep parsing.
        let toml_str = r#"
[system]
tmux = "/opt/homebrew/bin/tmux"
git = "/opt/homebrew/bin/git"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.system.tmux.as_deref(),
            Some("/opt/homebrew/bin/tmux")
        );
    }

    #[test]
    fn default_toast_positions_is_top_right() {
        let config = ToastConfig::default();
        assert_eq!(config.positions, vec![ToastPosition::TopRight]);
    }

    #[test]
    fn parse_toast_positions_kebab_case() {
        let toml_str = r#"
[toast]
positions = ["top-left", "bottom-right"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.toast.positions,
            vec![ToastPosition::TopLeft, ToastPosition::BottomRight]
        );
    }

    #[test]
    fn missing_positions_falls_back_to_default() {
        let toml_str = r#"
[toast]
duration_ms = 2000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.toast.positions, vec![ToastPosition::TopRight]);
    }

    #[test]
    fn default_toast_display_is_active() {
        let config = ToastConfig::default();
        assert_eq!(config.display, ToastDisplay::Active);
    }

    #[test]
    fn parse_toast_display_all() {
        let toml_str = r#"
[toast]
display = "all"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.toast.display, ToastDisplay::All);
    }

    #[test]
    fn missing_toast_display_falls_back_to_active() {
        let toml_str = r#"
[toast]
duration_ms = 2000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.toast.display, ToastDisplay::Active);
    }

    #[test]
    fn default_claude_code_hook_config() {
        let config = ClaudeCodeHookConfig::default();
        assert_eq!(
            config.events,
            vec![
                "Stop",
                "permission_prompt",
                "auth_success",
                "elicitation_dialog",
            ]
        );
        assert!(config.focus_events.is_empty());
    }

    #[test]
    fn parse_custom_events() {
        let toml_str = r#"
[notification.agents.claude_code]
events = ["Stop"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.notification.agents.claude_code.events, vec!["Stop"]);
        assert!(config
            .notification
            .agents
            .claude_code
            .focus_events
            .is_empty());
    }

    #[test]
    fn parse_focus_events() {
        let toml_str = r#"
[notification.agents.claude_code]
focus_events = ["Stop", "permission_prompt"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notification.agents.claude_code.focus_events,
            vec!["Stop", "permission_prompt"]
        );
        // Events should still have defaults (4 without idle_prompt)
        assert_eq!(config.notification.agents.claude_code.events.len(), 4);
    }

    #[test]
    fn default_codex_hook_config() {
        let config = CodexHookConfig::default();
        assert_eq!(config.events, vec!["agent-turn-complete"]);
        assert!(config.focus_events.is_empty());
        assert!(config.include_body);
    }

    #[test]
    fn parse_codex_events() {
        let toml_str = r#"
[notification.agents.codex]
events = ["agent-turn-complete"]
focus_events = ["agent-turn-complete"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notification.agents.codex.events,
            vec!["agent-turn-complete"]
        );
        assert_eq!(
            config.notification.agents.codex.focus_events,
            vec!["agent-turn-complete"]
        );
        assert!(config.notification.agents.codex.include_body);
    }

    #[test]
    fn parse_codex_include_body_false() {
        let toml_str = r#"
[notification.agents.codex]
include_body = false
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.notification.agents.codex.include_body);
        assert_eq!(
            config.notification.agents.codex.events,
            vec!["agent-turn-complete"]
        );
    }

    #[test]
    fn default_opencode_hook_config() {
        let config = OpenCodeHookConfig::default();
        assert_eq!(
            config.events,
            vec!["session.status", "session.error", "permission.asked"]
        );
        assert!(config.focus_events.is_empty());
    }

    #[test]
    fn parse_opencode_events() {
        let toml_str = r#"
[notification.agents.opencode]
events = ["session.status"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notification.agents.opencode.events,
            vec!["session.status"]
        );
        assert!(config.notification.agents.opencode.focus_events.is_empty());
    }

    #[test]
    fn parse_opencode_focus_events() {
        let toml_str = r#"
[notification.agents.opencode]
focus_events = ["permission.asked"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notification.agents.opencode.focus_events,
            vec!["permission.asked"]
        );
        assert_eq!(config.notification.agents.opencode.events.len(), 3);
    }

    #[test]
    fn default_apps_config_is_empty() {
        let config = AppsConfig::default();
        assert!(config.allowed_apps.is_empty());
    }

    #[test]
    fn parse_apps_allowed_apps() {
        let toml_str = r#"
[apps]
allowed_apps = [
  { bundle_id = "com.google.Chrome", display_name = "Google Chrome" },
  { bundle_id = "com.tinyspeck.slackmacgap", display_name = "Slack" },
]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.apps.allowed_apps.len(), 2);
        assert_eq!(config.apps.allowed_apps[0].bundle_id, "com.google.Chrome");
        assert_eq!(config.apps.allowed_apps[0].display_name, "Google Chrome");
        assert_eq!(
            config.apps.allowed_apps[1].bundle_id,
            "com.tinyspeck.slackmacgap"
        );
    }

    #[test]
    fn parse_empty_agents_section() {
        let toml_str = r#"
[toast]
duration_ms = 5000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        // notification.agents.claude_code should use defaults (4 without idle_prompt)
        assert_eq!(config.notification.agents.claude_code.events.len(), 4);
        assert!(config
            .notification
            .agents
            .claude_code
            .focus_events
            .is_empty());
        // notification.agents.opencode should use defaults (3 events)
        assert_eq!(config.notification.agents.opencode.events.len(), 3);
        assert!(config.notification.agents.opencode.focus_events.is_empty());
    }
}

/// Resolve the editor to use.
/// Priority: config.toml `editor` -> $EDITOR env var -> vim.
pub fn resolve_editor() -> String {
    let config = load_config();
    if let Some(ref editor) = config.editor {
        if !editor.is_empty() {
            return editor.clone();
        }
    }
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }
    "vim".to_string()
}
