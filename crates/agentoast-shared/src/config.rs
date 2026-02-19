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
    pub panel: PanelConfig,
    #[serde(default)]
    pub shortcut: ShortcutConfig,
    #[serde(default)]
    pub hook: HookConfig,
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
pub struct PanelConfig {
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_filter_notified_only")]
    pub filter_notified_only: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            muted: false,
            filter_notified_only: default_filter_notified_only(),
        }
    }
}

fn default_filter_notified_only() -> bool {
    true
}

fn default_toast_duration() -> u64 {
    4000
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShortcutConfig {
    #[serde(default = "default_toggle_panel")]
    pub toggle_panel: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            toggle_panel: default_toggle_panel(),
        }
    }
}

fn default_toggle_panel() -> String {
    "ctrl+alt+n".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub claude: ClaudeHookConfig,
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

/// Update [panel] muted in config.toml, preserving existing comments and formatting.
pub fn save_panel_muted(muted: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["panel"]["muted"] = toml_edit::value(muted);
    std::fs::write(&path, doc.to_string())
}

/// Update [panel] filter_notified_only in config.toml, preserving existing comments and formatting.
pub fn save_panel_filter_notified_only(value: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["panel"]["filter_notified_only"] = toml_edit::value(value);
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

# Menu bar notification panel
[panel]
# Mute all notifications (default: false)
# muted = false

# Show only groups with notifications (default: true)
# filter_notified_only = true

# Global keyboard shortcut
[shortcut]
# Shortcut to toggle the notification panel (default: ctrl+alt+n)
# Format: modifier+key (modifiers: ctrl, shift, alt/option, super/cmd)
# Set to "" to disable
# toggle_panel = "ctrl+alt+n"

# Claude Code hook settings
[hook.claude]
# Events that trigger notifications
# Available: Stop, permission_prompt, idle_prompt, auth_success, elicitation_dialog
# idle_prompt is excluded by default (noisy); add it back if you want idle notifications
# events = ["Stop", "permission_prompt", "auth_success", "elicitation_dialog"]

# Events that auto-focus the terminal (default: none)
# These events set force_focus=true, causing silent terminal focus without toast (when not muted)
# focus_events = []

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
[hook.claude]
events = ["Stop"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.hook.claude.events, vec!["Stop"]);
        assert!(config.hook.claude.focus_events.is_empty());
    }

    #[test]
    fn parse_focus_events() {
        let toml_str = r#"
[hook.claude]
focus_events = ["Stop", "permission_prompt"]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.hook.claude.focus_events,
            vec!["Stop", "permission_prompt"]
        );
        // Events should still have defaults (4 without idle_prompt)
        assert_eq!(config.hook.claude.events.len(), 4);
    }

    #[test]
    fn parse_empty_hook_section() {
        let toml_str = r#"
[toast]
duration_ms = 5000
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        // Hook.claude should use defaults (4 without idle_prompt)
        assert_eq!(config.hook.claude.events.len(), 4);
        assert!(config.hook.claude.focus_events.is_empty());
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
