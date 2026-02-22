use serde::Deserialize;
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToastConfig {
    #[serde(default = "default_toast_duration")]
    pub duration_ms: u64,
    #[serde(default)]
    pub persistent: bool,
}

impl Default for ToastConfig {
    fn default() -> Self {
        Self {
            duration_ms: default_toast_duration(),
            persistent: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_filter_notified_only")]
    pub filter_notified_only: bool,
    #[serde(default)]
    pub agents: AgentsConfig,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            muted: false,
            filter_notified_only: default_filter_notified_only(),
            agents: AgentsConfig::default(),
        }
    }
}

fn default_filter_notified_only() -> bool {
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
    pub claude: ClaudeHookConfig,
    #[serde(default)]
    pub codex: CodexHookConfig,
    #[serde(default)]
    pub opencode: OpenCodeHookConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClaudeHookConfig {
    #[serde(default = "default_claude_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub focus_events: Vec<String>,
}

impl Default for ClaudeHookConfig {
    fn default() -> Self {
        Self {
            events: default_claude_events(),
            focus_events: Vec::new(),
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

fn default_claude_events() -> Vec<String> {
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

# Notification settings
[notification]
# Mute all notifications (default: false)
# muted = false

# Show only groups with notifications (default: false)
# filter_notified_only = false

# Claude Code agent settings
[notification.agents.claude]
# Events that trigger notifications
# Available: Stop, permission_prompt, idle_prompt, auth_success, elicitation_dialog
# idle_prompt is excluded by default (noisy); add it back if you want idle notifications
# events = ["Stop", "permission_prompt", "auth_success", "elicitation_dialog"]

# Events that auto-focus the terminal (default: none)
# These events set force_focus=true, causing silent terminal focus without toast (when not muted)
# focus_events = []

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

# Keyboard shortcuts
[keybinding]
# Shortcut to toggle the notification panel (default: super+ctrl+n)
# Format: modifier+key (modifiers: ctrl, shift, alt/option, super/cmd)
# Set to "" to disable
# toggle_panel = "super+ctrl+n"

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
    fn default_claude_hook_config() {
        let config = ClaudeHookConfig::default();
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
[notification.agents.claude]
events = ["Stop"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.notification.agents.claude.events, vec!["Stop"]);
        assert!(config.notification.agents.claude.focus_events.is_empty());
    }

    #[test]
    fn parse_focus_events() {
        let toml_str = r#"
[notification.agents.claude]
focus_events = ["Stop", "permission_prompt"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.notification.agents.claude.focus_events,
            vec!["Stop", "permission_prompt"]
        );
        // Events should still have defaults (4 without idle_prompt)
        assert_eq!(config.notification.agents.claude.events.len(), 4);
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
    fn parse_empty_agents_section() {
        let toml_str = r#"
[toast]
duration_ms = 5000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        // notification.agents.claude should use defaults (4 without idle_prompt)
        assert_eq!(config.notification.agents.claude.events.len(), 4);
        assert!(config.notification.agents.claude.focus_events.is_empty());
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
