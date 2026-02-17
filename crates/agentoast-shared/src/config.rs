use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use toml_edit::DocumentMut;

/// XDG_DATA_HOME / agentoast を返す。
/// macOS の dirs クレートは ~/Library/Application Support を返すため、
/// XDG 準拠で ~/.local/share を直接構築する。
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

/// SQLite DB ファイルのパスを返す。
pub fn db_path() -> PathBuf {
    data_dir().join("notifications.db")
}

/// XDG_CONFIG_HOME / agentoast を返す。
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
    #[serde(default = "default_group_limit")]
    pub group_limit: usize,
    #[serde(default)]
    pub muted: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            group_limit: default_group_limit(),
            muted: false,
        }
    }
}

fn default_group_limit() -> usize {
    3
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

/// config.toml のパスを返す。
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// config.toml を読み込む。ファイルが存在しない場合やパースエラーの場合はデフォルト値を返す。
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

/// config.toml の [panel] muted を更新する。既存のコメントやフォーマットを保持する。
pub fn save_panel_muted(muted: bool) -> io::Result<()> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().unwrap_or_default();
    doc["panel"]["muted"] = toml_edit::value(muted);
    std::fs::write(&path, doc.to_string())
}

/// デフォルトの config.toml テンプレート。
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
# Maximum number of notifications per group (default: 3, 0 = unlimited)
# group_limit = 3

# Mute all notifications (default: false)
# muted = false

# Global keyboard shortcut
[shortcut]
# Shortcut to toggle the notification panel (default: ctrl+alt+n)
# Format: modifier+key (modifiers: ctrl, shift, alt/option, super/cmd)
# Set to "" to disable
# toggle_panel = "ctrl+alt+n"

"#
}

/// config.toml が存在しなければデフォルトテンプレートで作成する。パスを返す。
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

/// 使用するエディタを決定する。
/// 優先順位: config.toml の editor → $EDITOR 環境変数 → vim
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
