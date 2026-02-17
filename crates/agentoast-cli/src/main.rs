use std::collections::HashMap;

use agentoast_shared::config;
use agentoast_shared::db;
use agentoast_shared::models::IconType;
use clap::{Parser, Subcommand};

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
        /// Notification title (displayed as badge)
        #[arg(long, default_value = "")]
        title: String,

        /// Notification body text
        #[arg(long, default_value = "")]
        body: String,

        /// Badge color: green, blue, red, gray
        #[arg(long, default_value = "gray")]
        color: String,

        /// Icon preset: claude-code, codex, or agentoast
        #[arg(long, default_value = "agentoast")]
        icon: String,

        /// tmux pane ID (e.g. %5)
        #[arg(long, default_value = "")]
        tmux_pane: String,

        /// Terminal bundle ID for focus-on-click (e.g. com.github.wez.wezterm).
        /// Auto-detected from __CFBundleIdentifier if not specified.
        #[arg(long)]
        bundle_id: Option<String>,

        /// Focus terminal automatically when notification is sent
        #[arg(long)]
        focus: bool,

        /// Metadata key=value pairs (can be specified multiple times)
        #[arg(long = "meta", value_name = "KEY=VALUE")]
        meta: Vec<String>,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send {
            title,
            body,
            color,
            icon,
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

            let metadata = parse_metadata(&meta);

            let terminal_bundle_id = bundle_id
                .unwrap_or_else(|| std::env::var("__CFBundleIdentifier").unwrap_or_default());

            let db_path = config::db_path();
            let conn = db::open_reader(&db_path).unwrap_or_else(|e| {
                eprintln!("Failed to open database: {}", e);
                std::process::exit(1);
            });

            match db::insert_notification(
                &conn,
                &title,
                &body,
                &color,
                &icon_type,
                &metadata,
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
                            read_mark, n.id, n.title, n.icon, pane_str, n.body, meta_str
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
