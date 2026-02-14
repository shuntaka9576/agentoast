use serde::Deserialize;
use std::io;
use std::path::PathBuf;

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
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_group_limit")]
    pub group_limit: usize,
}

fn default_group_limit() -> usize {
    3
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            group_limit: default_group_limit(),
        }
    }
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

/// デフォルトの config.toml テンプレート。
fn default_config_template() -> &'static str {
    r#"# agentoast configuration

# Editor to open when running `agentoast config`
# Falls back to $EDITOR environment variable, then vim
# editor = "vim"

[display]
# Maximum number of notifications per group in the main panel
# group_limit = 3
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
