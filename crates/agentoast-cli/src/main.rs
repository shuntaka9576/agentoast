mod hooks;

use agentoast_shared::models::IconType;
use agentoast_shared::{config, db};
use clap::{Parser, Subcommand};

use hooks::{get_git_info, parse_metadata};

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
    /// Handle Codex hook events (reads JSON from last CLI argument)
    Codex {
        /// JSON payload (passed as the last argument by Codex)
        json: String,
    },
    /// Handle OpenCode hook events (reads JSON from CLI argument)
    Opencode {
        /// JSON payload containing event type, properties, and directory
        json: String,
    },
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
                &db::NotificationInput {
                    badge: &badge,
                    body: &body,
                    badge_color: &badge_color,
                    icon: &icon_type,
                    metadata: &metadata,
                    repo: &repo,
                    tmux_pane: &tmux_pane,
                    terminal_bundle_id: &terminal_bundle_id,
                    force_focus: focus,
                },
            ) {
                Ok(id) => println!("Notification saved (id={})", id),
                Err(e) => {
                    eprintln!("Failed to insert notification: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Hook { agent } => match agent {
            HookAgent::Claude => hooks::claude::handle(),
            HookAgent::Codex { json } => hooks::codex::handle(&json),
            HookAgent::Opencode { json } => hooks::opencode::handle(&json),
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
