use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use agentoast_shared::config;
use agentoast_shared::db;
use agentoast_shared::models::IconType;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "agentoast", about = "Agentoast - CLI notification tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a notification
    Send {
        /// Badge text displayed on notification card
        #[arg(short = 'B', long, default_value = "")]
        badge: String,

        /// Notification body text
        #[arg(short = 'b', long, default_value = "")]
        body: String,

        /// Badge color: green, blue, red, gray
        #[arg(short = 'c', long, default_value = "gray")]
        badge_color: String,

        /// Icon preset: claude-code, codex, or agentoast
        #[arg(short = 'i', long, default_value = "agentoast")]
        icon: String,

        /// Repository name for grouping notifications (auto-detected from git if omitted)
        #[arg(short = 'r', long)]
        repo: Option<String>,

        /// tmux pane ID (e.g. %5)
        #[arg(short = 't', long, default_value = "")]
        tmux_pane: String,

        /// Terminal bundle ID for focus-on-click (e.g. com.github.wez.wezterm).
        /// Auto-detected from __CFBundleIdentifier if not specified.
        #[arg(long)]
        bundle_id: Option<String>,

        /// Focus terminal automatically when notification is sent
        #[arg(short = 'f', long)]
        focus: bool,

        /// Metadata key=value pairs (can be specified multiple times)
        #[arg(short = 'm', long = "meta", value_name = "KEY=VALUE")]
        meta: Vec<String>,
    },

    /// Handle hook events from AI coding agents
    Hook {
        #[command(subcommand)]
        agent: HookAgent,
    },

    /// List recent notifications (debug)
    List {
        /// Max number of notifications to show
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },

    /// Open config file in editor
    Config,
}

#[derive(Subcommand)]
enum HookAgent {
    /// Handle Claude Code hook events (reads JSON from stdin)
    Claude,
}

#[derive(Deserialize)]
struct ClaudeHookData {
    hook_event_name: String,
    cwd: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct HookResult {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

struct GitInfo {
    repo_name: String,
    branch_name: String,
}

fn parse_metadata(meta_args: &[String]) -> HashMap<String, String> {
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

fn get_git_info(cwd: &Path) -> GitInfo {
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
                // Extract repo name from URL like git@github.com:user/repo.git or https://...
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

fn run_claude_hook() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    let data: ClaudeHookData =
        serde_json::from_str(&input).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let event_key = data
        .notification_type
        .as_deref()
        .unwrap_or(&data.hook_event_name);

    let hook_config = config::load_config().hook.claude;

    if !hook_config.events.iter().any(|e| e == event_key) {
        return Ok(());
    }

    let is_stop = data.hook_event_name == "Stop";
    let badge = if is_stop { "Stop" } else { "Notification" };
    let badge_color = if is_stop { "green" } else { "blue" };
    let body = data.message.as_deref().unwrap_or("");
    let force_focus = hook_config.focus_events.iter().any(|e| e == event_key);

    let mut metadata = HashMap::new();

    let repo_name;
    if let Some(ref cwd_str) = data.cwd {
        let cwd = Path::new(cwd_str);
        let git_info = get_git_info(cwd);
        repo_name = git_info.repo_name;
        if !git_info.branch_name.is_empty() {
            metadata.insert("branch".to_string(), git_info.branch_name);
        }
    } else {
        repo_name = String::new();
    }

    let tmux_pane = std::env::var("TMUX_PANE").unwrap_or_default();
    let terminal_bundle_id = std::env::var("__CFBundleIdentifier").unwrap_or_default();

    let db_path = config::db_path();
    let conn = db::open_reader(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

    db::insert_notification(
        &conn,
        badge,
        body,
        badge_color,
        &IconType::ClaudeCode,
        &metadata,
        &repo_name,
        &tmux_pane,
        &terminal_bundle_id,
        force_focus,
    )
    .map_err(|e| format!("Failed to insert notification: {}", e))?;

    Ok(())
}

fn handle_claude_hook() {
    let result = match run_claude_hook() {
        Ok(()) => HookResult {
            success: true,
            error: None,
        },
        Err(e) => HookResult {
            success: false,
            error: Some(e),
        },
    };

    println!(
        "{}",
        serde_json::to_string(&result).unwrap_or_else(|_| r#"{"success":false}"#.to_string())
    );
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send {
            badge,
            body,
            badge_color,
            icon,
            repo,
            tmux_pane,
            bundle_id,
            focus,
            meta,
        } => {
            let icon_type: IconType = icon.parse().unwrap_or_else(|e: String| {
                eprintln!(
                    "{} Use 'agentoast', 'claude-code', 'codex', or 'opencode'.",
                    e
                );
                std::process::exit(1);
            });

            let mut metadata = parse_metadata(&meta);

            let repo = match repo {
                Some(r) => {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let git_info = get_git_info(&cwd);
                    if !git_info.branch_name.is_empty() {
                        metadata
                            .entry("branch".to_string())
                            .or_insert(git_info.branch_name);
                    }
                    r
                }
                None => {
                    let cwd = std::env::current_dir().unwrap_or_else(|e| {
                        eprintln!("Failed to get current directory: {}", e);
                        std::process::exit(1);
                    });
                    let git_info = get_git_info(&cwd);
                    if git_info.repo_name.is_empty() {
                        eprintln!("Could not detect repository name. Use --repo to specify it.");
                        std::process::exit(1);
                    }
                    if !git_info.branch_name.is_empty() {
                        metadata
                            .entry("branch".to_string())
                            .or_insert(git_info.branch_name);
                    }
                    git_info.repo_name
                }
            };

            let terminal_bundle_id = bundle_id
                .unwrap_or_else(|| std::env::var("__CFBundleIdentifier").unwrap_or_default());

            let db_path = config::db_path();
            let conn = db::open_reader(&db_path).unwrap_or_else(|e| {
                eprintln!("Failed to open database: {}", e);
                std::process::exit(1);
            });

            match db::insert_notification(
                &conn,
                &badge,
                &body,
                &badge_color,
                &icon_type,
                &metadata,
                &repo,
                &tmux_pane,
                &terminal_bundle_id,
                focus,
            ) {
                Ok(id) => println!("Notification saved (id={})", id),
                Err(e) => {
                    eprintln!("Failed to insert notification: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Hook { agent } => match agent {
            HookAgent::Claude => handle_claude_hook(),
        },
        Commands::Config => {
            let config_path = config::ensure_config_file().unwrap_or_else(|e| {
                eprintln!("Failed to create config file: {}", e);
                std::process::exit(1);
            });

            let editor = config::resolve_editor();

            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{} \"{}\"", editor, config_path.display()))
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("Failed to launch editor '{}': {}", editor, e);
                    std::process::exit(1);
                });

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::List { limit } => {
            let db_path = config::db_path();
            let conn = db::open_reader(&db_path).unwrap_or_else(|e| {
                eprintln!("Failed to open database: {}", e);
                std::process::exit(1);
            });

            match db::get_notifications(&conn, limit) {
                Ok(notifications) => {
                    if notifications.is_empty() {
                        println!("No notifications.");
                        return;
                    }
                    for n in &notifications {
                        let read_mark = if n.is_read { " " } else { "*" };
                        let meta_str = if n.metadata.is_empty() {
                            String::new()
                        } else {
                            let pairs: Vec<_> = n
                                .metadata
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect();
                            format!(" [{}]", pairs.join(", "))
                        };
                        let pane_str = if n.tmux_pane.is_empty() {
                            String::new()
                        } else {
                            format!(" (pane:{})", n.tmux_pane)
                        };
                        println!(
                            "{} [{}] {} [{}]{} {}{}",
                            read_mark, n.id, n.badge, n.icon, pane_str, n.body, meta_str
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to list notifications: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
