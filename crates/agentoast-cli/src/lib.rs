pub mod hooks;

use agentoast_shared::models::IconType;
use agentoast_shared::{agent_detect, config, db, tmux};
use clap::{Parser, Subcommand};

use hooks::{get_git_info, parse_metadata};

const APP_VERSION: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " version ",
    env!("CARGO_PKG_VERSION"),
    " (rev:",
    env!("GIT_HASH"),
    ")"
);

#[derive(Parser)]
#[command(
    name = "agentoast",
    about = "Agentoast - CLI notification tool",
    disable_version_flag = true
)]
struct Cli {
    /// Print version
    #[arg(long, short = 'v')]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
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

    /// Dismiss all notifications for a given tmux pane
    Dismiss {
        /// tmux pane ID (e.g. %5)
        #[arg(short = 't', long)]
        tmux_pane: String,
    },

    /// Inject a message into another tmux pane's agent (cross-agent messaging)
    SendKeys {
        /// Target tmux pane ID (e.g. %72)
        #[arg(short = 't', long)]
        pane: String,

        /// Message text to inject
        message: String,

        /// Sender pane ID embedded as the reply address (default: $TMUX_PANE)
        #[arg(long)]
        from: Option<String>,

        /// Inject the raw message only, without the reply hint
        #[arg(long)]
        raw: bool,

        /// Do not send a trailing Enter (leave the text without submitting)
        #[arg(long)]
        no_enter: bool,
    },

    /// Detect whether the given tmux pane is running an AI coding agent.
    /// Prints `agent` (exit 0) or `no-agent` (exit 1) to stdout. Used by the
    /// agentoast-send skill as a single source of truth for agent detection.
    DetectAgent {
        /// Target tmux pane ID (e.g. %72)
        #[arg(short = 't', long)]
        pane: String,
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
    /// Handle Copilot CLI hook events (reads JSON from stdin, event name from --event arg)
    Copilot {
        /// Event name (e.g. agentStop, errorOccurred)
        #[arg(long)]
        event: String,
    },
    /// Handle OpenCode hook events (reads JSON from CLI argument)
    Opencode {
        /// JSON payload containing event type, properties, and directory
        json: String,
    },
}

/// Try to run CLI subcommands. Returns true if a CLI subcommand was handled,
/// false if no CLI subcommand was detected (caller should launch the GUI).
pub fn try_run_cli() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return false;
    }
    let first = &args[1];
    let known = [
        "send",
        "send-keys",
        "detect-agent",
        "hook",
        "list",
        "dismiss",
        "config",
        "--version",
        "-V",
        "-v",
        "--help",
        "-h",
        "help",
    ];
    if !known.contains(&first.as_str()) {
        return false;
    }
    let cli = Cli::parse();
    run(cli);
    true
}

/// Resolve the tmux binary path from config + built-in lookup, or exit 1.
fn resolve_tmux_or_exit() -> std::path::PathBuf {
    let tmux_override = config::load_config().system.tmux;
    match tmux::find_tmux(tmux_override.as_deref()) {
        Some(p) => p,
        None => {
            eprintln!("tmux not found");
            std::process::exit(1);
        }
    }
}

/// Detect which AI coding agent (if any) is running in `pane`. Exits 1 with a
/// "pane not found" error when the pane id is bogus. Shared between
/// `send-keys` (uses it as a guard) and `detect-agent` (returns the verdict).
fn detect_agent_for_pane(tmux_bin: &std::path::Path, pane: &str) -> Option<String> {
    let Some(pid) = tmux::pane_pid(tmux_bin, pane) else {
        eprintln!("pane '{}' not found", pane);
        std::process::exit(1);
    };
    let process_tree = agent_detect::build_process_tree();
    agent_detect::detect_agent(&process_tree, pid)
}

fn run(cli: Cli) {
    if cli.version {
        println!("{APP_VERSION}");
        return;
    }

    let Some(command) = cli.command else {
        use clap::CommandFactory;
        Cli::command().print_help().unwrap();
        std::process::exit(1);
    };

    match command {
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
                    "{} Use 'agentoast', 'claude-code', 'codex', 'copilot-cli', or 'opencode'.",
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
            HookAgent::Copilot { event } => hooks::copilot::handle(&event),
            HookAgent::Opencode { json } => hooks::opencode::handle(&json),
        },
        // Called by tmux hooks (`after-select-pane` et al.) to clear
        // notifications when the user navigates to a pane directly. tmux can
        // hand us an empty pane id in edge cases (hook fires before the pane
        // is fully selected), so treat empty as a no-op rather than erroring.
        Commands::Dismiss { tmux_pane } => {
            if tmux_pane.is_empty() {
                return;
            }
            let db_path = config::db_path();
            let conn = db::open_reader(&db_path).unwrap_or_else(|e| {
                eprintln!("Failed to open database: {}", e);
                std::process::exit(1);
            });
            if let Err(e) = db::delete_notifications_by_pane(&conn, &tmux_pane) {
                eprintln!("Failed to delete notifications: {}", e);
                std::process::exit(1);
            }
        }
        Commands::SendKeys {
            pane,
            message,
            from,
            raw,
            no_enter,
        } => {
            let tmux_bin = resolve_tmux_or_exit();
            let detected_agent = detect_agent_for_pane(&tmux_bin, &pane);

            // Guard: send-keys is for agent-to-agent messaging. If the target
            // pane runs no detected AI agent (e.g. a plain shell), refuse —
            // otherwise the message would be typed into the shell prompt and
            // executed as commands. To send to a pane running an agent the
            // detector doesn't recognize, add its process name to
            // AGENT_PROCESSES in crates/agentoast-shared/src/agent_detect.rs.
            if detected_agent.is_none() {
                eprintln!(
                    "pane '{}' has no detected AI coding agent (it looks like a plain shell). send-keys refused to type the message into a shell prompt.",
                    pane
                );
                eprintln!(
                    "If this pane really runs an agent, add it to AGENT_PROCESSES in crates/agentoast-shared/src/agent_detect.rs."
                );
                std::process::exit(1);
            }

            // Reply address: --from > $TMUX_PANE (the sender agent's own pane).
            let from_pane = from
                .or_else(|| std::env::var("TMUX_PANE").ok())
                .filter(|s| !s.is_empty());

            // Append a single-line reply hint unless --raw, so the receiving
            // agent knows where and how to reply.
            let body = if raw {
                message
            } else if let Some(fp) = &from_pane {
                format!(
                    "[agentoast] from {fp}: {message}  (reply: agentoast send-keys --pane {fp} \"<reply>\")"
                )
            } else {
                message
            };

            match tmux::send_keys(&tmux_bin, &pane, &body, !no_enter) {
                Ok(()) => println!(
                    "sent to {}{} (from {})",
                    pane,
                    detected_agent
                        .map(|a| format!(" [{}]", a))
                        .unwrap_or_default(),
                    from_pane.as_deref().unwrap_or("-")
                ),
                Err(e) => {
                    eprintln!("send-keys failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::DetectAgent { pane } => {
            let tmux_bin = resolve_tmux_or_exit();
            match detect_agent_for_pane(&tmux_bin, &pane) {
                Some(_) => {
                    println!("agent");
                }
                None => {
                    println!("no-agent");
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
