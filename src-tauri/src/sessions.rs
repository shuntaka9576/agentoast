use std::collections::HashMap;
use std::process::Command;

use agentoast_shared::models::{AgentStatus, TmuxPane, TmuxPaneGroup};
use agentoast_shared::{config, db};

use crate::terminal::{find_git, find_tmux};

const AGENT_PROCESSES: &[(&str, &str)] = &[
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

struct GitInfo {
    repo_root: String,
    repo_name: String,
    branch: Option<String>,
}

/// Resolve git info for each unique path. Caches results per polling cycle.
fn resolve_git_info(paths: &[String]) -> HashMap<String, Option<GitInfo>> {
    let mut cache: HashMap<String, Option<GitInfo>> = HashMap::new();

    let git_path = match find_git() {
        Some(p) => p,
        None => {
            for path in paths {
                cache.insert(path.clone(), None);
            }
            return cache;
        }
    };

    for path in paths {
        if cache.contains_key(path) {
            continue;
        }

        // git rev-parse --show-toplevel
        let repo_root = Command::new(&git_path)
            .env_remove("TMPDIR")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            });

        let info = match repo_root {
            Some(root) => {
                // git remote get-url origin → extract repo name from URL
                let repo_name = Command::new(&git_path)
                    .env_remove("TMPDIR")
                    .args(["remote", "get-url", "origin"])
                    .current_dir(path)
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            let url = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            extract_repo_name_from_url(&url)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        // Fallback: last component of repo_root
                        root.rsplit('/').next().unwrap_or(&root).to_string()
                    });

                // git branch --show-current
                let branch = Command::new(&git_path)
                    .env_remove("TMPDIR")
                    .args(["branch", "--show-current"])
                    .current_dir(path)
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            let b = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            if b.is_empty() {
                                None
                            } else {
                                Some(b)
                            }
                        } else {
                            None
                        }
                    });

                Some(GitInfo {
                    repo_root: root,
                    repo_name,
                    branch,
                })
            }
            None => None,
        };

        cache.insert(path.clone(), info);
    }

    cache
}

/// Extract repository name from a git remote URL.
/// Supports HTTPS (`https://github.com/owner/repo.git`) and SSH (`git@github.com:owner/repo.git`).
fn extract_repo_name_from_url(url: &str) -> Option<String> {
    let path = if let Some(rest) = url.strip_prefix("git@") {
        // SSH: git@github.com:owner/repo.git
        rest.split(':').nth(1)?
    } else {
        // HTTPS: https://github.com/owner/repo.git
        url.split("://").nth(1).unwrap_or(url)
    };
    let name = path.rsplit('/').next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn list_tmux_panes_grouped() -> Result<Vec<TmuxPaneGroup>, String> {
    log::info!("sessions: get_sessions called");
    log::info!(
        "sessions: TMPDIR={:?}, TMUX_TMPDIR={:?}",
        std::env::var("TMPDIR").ok(),
        std::env::var("TMUX_TMPDIR").ok()
    );
    let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;
    log::debug!("sessions: tmux found at {:?}", tmux_path);

    // Use "|||" as delimiter instead of "\t" because macOS Launch Services
    // (Finder double-click) sanitizes control characters in process arguments,
    // converting tabs to underscores.
    const DELIM: &str = "|||";
    let format_str = format!(
        "#{{pane_id}}{d}#{{pane_pid}}{d}#{{session_name}}{d}#{{window_name}}{d}#{{pane_current_path}}{d}#{{pane_active}}{d}#{{window_active}}{d}#{{session_attached}}",
        d = DELIM
    );

    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["list-panes", "-a", "-F", &format_str])
        .output()
        .map_err(|e| {
            log::error!("sessions: tmux list-panes exec failed: {}", e);
            format!("tmux list-panes failed: {}", e)
        })?;

    let stderr_str = String::from_utf8_lossy(&output.stderr);
    log::info!(
        "sessions: tmux list-panes exit={}, stderr={:?}",
        output.status,
        stderr_str.as_ref()
    );

    if !output.status.success() {
        log::error!("sessions: tmux list-panes returned error: {}", stderr_str);
        return Err(format!("tmux list-panes failed: {}", stderr_str));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!(
        "sessions: tmux list-panes stdout len={} lines={}",
        stdout.len(),
        stdout.lines().count()
    );
    log::debug!("sessions: tmux list-panes stdout:\n{}", stdout);

    // Build process tree once for all panes
    let process_tree = build_process_tree();
    log::debug!(
        "sessions: process tree: {} processes, {} parent entries",
        process_tree.commands.len(),
        process_tree.children.len()
    );

    // Parse panes (without git info yet)
    struct RawPane {
        pane_id: String,
        pane_pid: u32,
        session_name: String,
        window_name: String,
        current_path: String,
        is_active: bool,
        agent_type: Option<String>,
    }

    let mut raw_panes: Vec<RawPane> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(8, DELIM).collect();
        if parts.len() < 8 {
            continue;
        }

        let pane_pid: u32 = parts[1].parse().unwrap_or(0);
        let agent_type = detect_agent(&process_tree, pane_pid);
        let is_active = parts[5] == "1" && parts[6] == "1" && parts[7] == "1";
        log::debug!(
            "sessions: pane {} pid={} agent={:?} is_active={}",
            parts[0],
            pane_pid,
            agent_type,
            is_active
        );

        raw_panes.push(RawPane {
            pane_id: parts[0].to_string(),
            pane_pid,
            session_name: parts[2].to_string(),
            window_name: parts[3].to_string(),
            current_path: parts[4].to_string(),
            is_active,
            agent_type,
        });
    }
    log::debug!("sessions: parsed {} panes total", raw_panes.len());

    // Open DB connection for agent status detection (read-only, no schema init)
    let db_conn = db::open_reader(&config::db_path()).ok();

    // Resolve git info for all unique paths
    let unique_paths: Vec<String> = raw_panes
        .iter()
        .map(|p| p.current_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let git_cache = resolve_git_info(&unique_paths);

    // Build TmuxPane with git info and agent status
    let panes: Vec<TmuxPane> = raw_panes
        .into_iter()
        .map(|rp| {
            let git_info = git_cache.get(&rp.current_path).and_then(|o| o.as_ref());
            let (agent_status, agent_modes) = if let Some(ref at) = rp.agent_type {
                let (status, modes) = detect_agent_status(&db_conn, &rp.pane_id, at);
                (Some(status), modes)
            } else {
                (None, Vec::new())
            };
            TmuxPane {
                pane_id: rp.pane_id,
                pane_pid: rp.pane_pid,
                session_name: rp.session_name,
                window_name: rp.window_name,
                current_path: rp.current_path,
                is_active: rp.is_active,
                agent_type: rp.agent_type,
                agent_status,
                agent_modes,
                git_repo_root: git_info.map(|g| g.repo_root.clone()),
                git_branch: git_info.and_then(|g| g.branch.clone()),
            }
        })
        .collect();

    // Group by git_repo_root (fallback to current_path for non-git dirs)
    let mut groups_map: HashMap<String, Vec<TmuxPane>> = HashMap::new();
    for pane in panes {
        let key = pane
            .git_repo_root
            .clone()
            .unwrap_or_else(|| pane.current_path.clone());
        groups_map.entry(key).or_default().push(pane);
    }

    let mut groups: Vec<TmuxPaneGroup> = groups_map
        .into_iter()
        .map(|(key, panes)| {
            // Use repo_name from GitInfo (resolved via git remote), fallback to path last component
            let repo_name = panes
                .iter()
                .find_map(|p| {
                    p.git_repo_root.as_ref().and_then(|_| {
                        git_cache
                            .get(&p.current_path)
                            .and_then(|o| o.as_ref())
                            .map(|g| g.repo_name.clone())
                    })
                })
                .unwrap_or_else(|| key.rsplit('/').next().unwrap_or(&key).to_string());
            // Use git_branch from the first pane that has it
            let git_branch = panes.iter().find_map(|p| p.git_branch.clone());
            TmuxPaneGroup {
                repo_name,
                current_path: key,
                git_branch,
                panes,
            }
        })
        .collect();

    // Keep only panes with active agents, remove empty groups
    for group in &mut groups {
        group.panes.retain(|p| p.agent_type.is_some());
    }
    groups.retain(|g| !g.panes.is_empty());
    log::debug!(
        "sessions: after agent filter: {} groups, panes: {:?}",
        groups.len(),
        groups
            .iter()
            .map(|g| format!("{}({})", g.repo_name, g.panes.len()))
            .collect::<Vec<_>>()
    );

    // Sort alphabetically by repo name
    groups.sort_by(|a, b| a.repo_name.cmp(&b.repo_name));

    Ok(groups)
}

/// Process tree: maps parent PID to (child PID, command name) pairs.
struct ProcessTree {
    children: HashMap<u32, Vec<u32>>,
    commands: HashMap<u32, String>,
}

fn build_process_tree() -> ProcessTree {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut commands: HashMap<u32, String> = HashMap::new();

    let output = match Command::new("/bin/ps")
        .args(["-eo", "pid,ppid,comm"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::error!("sessions: /bin/ps exec failed: {}", e);
            return ProcessTree { children, commands };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let mut iter = line.split_whitespace();
        let pid: u32 = match iter.next().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let ppid: u32 = match iter.next().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let comm: String = iter.collect::<Vec<&str>>().join(" ");
        if comm.is_empty() {
            continue;
        }

        children.entry(ppid).or_default().push(pid);
        commands.insert(pid, comm);
    }

    ProcessTree { children, commands }
}

struct ClaudePaneContentInfo {
    has_spinner: bool, // Spinner chars + "…" / "esc to interrupt" (real-time, reliable)
    has_status_running: bool, // Status bar "(running)" suffix (may be stale)
    at_prompt: bool,
    has_elicitation: bool, // "Enter to select" navigation hint (selection dialog)
    agent_modes: Vec<String>,
}

fn detect_agent_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
    agent_type: &str,
) -> (AgentStatus, Vec<String>) {
    match agent_type {
        "claude-code" => detect_claude_status(db_conn, pane_id),
        "codex" => detect_codex_status(db_conn, pane_id),
        _ => {
            log::debug!(
                "detect_agent_status({}): unknown agent_type='{}', defaulting to Running",
                pane_id,
                agent_type
            );
            (AgentStatus::Running, Vec::new())
        }
    }
}

fn detect_claude_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
) -> (AgentStatus, Vec<String>) {
    let info = check_claude_pane_content(pane_id);

    log::debug!(
        "detect_claude_status({}): spinner={} status_running={} elicitation={} prompt={}",
        pane_id,
        info.has_spinner,
        info.has_status_running,
        info.has_elicitation,
        info.at_prompt
    );

    // Spinners are real-time signals and take highest priority.
    // Status bar "(running)" may be stale (e.g., plan mode waiting with old
    // status bar text), so it does NOT override at_prompt.
    let status = if info.has_spinner {
        AgentStatus::Running
    } else if info.has_elicitation {
        // Elicitation dialog ("Enter to select" detected) — always Waiting.
        // Checked before at_prompt because elicitation option description text
        // (indented continuation lines) causes is_prompt_line() to return false.
        AgentStatus::Waiting
    } else if info.at_prompt {
        if let Some(conn) = db_conn {
            if let Ok(Some(_)) = db::get_latest_notification_by_pane(conn, pane_id) {
                AgentStatus::Waiting
            } else {
                AgentStatus::Idle
            }
        } else {
            AgentStatus::Idle
        }
    } else {
        // has_status_running or no signal — default to Running
        AgentStatus::Running
    };

    (status, info.agent_modes)
}

/// Claude Code spinner characters that appear at the start of running lines.
const SPINNER_CHARS: &[char] = &['✢', '✽', '✶', '✻', '·'];

/// Check pane content for running indicators, prompt patterns, and mode indicators.
/// Running: spinner+"…" / spinner+"esc to interrupt" / status bar "(running)".
/// Idle: footer-skipping prompt detection. Plan mode: status bar "plan mode on".
/// Mode detection patterns: (substring to match, label for frontend)
const MODE_PATTERNS: &[(&str, &str)] = &[
    ("plan mode on", "plan"),
    ("bypass permissions on", "bypass"),
    ("accept edits on", "accept"),
];

fn check_claude_pane_content(pane_id: &str) -> ClaudePaneContentInfo {
    let default = ClaudePaneContentInfo {
        has_spinner: false,
        has_status_running: false,
        at_prompt: false,
        has_elicitation: false,
        agent_modes: Vec::new(),
    };

    let tmux_path = match find_tmux() {
        Some(p) => p,
        None => {
            log::debug!("check_claude_pane_content: tmux not found");
            return default;
        }
    };

    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["capture-pane", "-t", pane_id, "-p"])
        .output()
        .ok();

    let Some(output) = output else {
        log::debug!(
            "check_claude_pane_content({}): capture-pane exec failed",
            pane_id
        );
        return default;
    };
    if !output.status.success() {
        log::debug!(
            "check_claude_pane_content({}): capture-pane exit={} stderr={}",
            pane_id,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return default;
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let all_lines: Vec<&str> = content.lines().collect();

    // Get last 30 non-empty, non-separator lines for scanning
    let last_lines: Vec<&str> = all_lines
        .iter()
        .rev()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !is_separator_line(trimmed)
        })
        .take(30)
        .copied()
        .collect();

    log::debug!(
        "check_claude_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut has_spinner = false;
    let mut has_status_running = false;
    let mut has_elicitation = false;
    let mut agent_modes: Vec<String> = Vec::new();

    for line in &last_lines {
        let trimmed = line.trim();

        // Running detection: spinner char + "esc to interrupt" or "…"
        if !has_spinner && is_claude_running_line(trimmed) {
            log::debug!(
                "check_claude_pane_content({}): running detected (spinner): {:?}",
                pane_id,
                trimmed
            );
            has_spinner = true;
        }

        // Status bar "(running)" suffix — may be stale
        // e.g., "⏵⏵ bypass permissions on · for dir in auth admin; do… (running)"
        if !has_status_running && trimmed.ends_with("(running)") {
            log::debug!(
                "check_claude_pane_content({}): status bar running detected: {:?}",
                pane_id,
                trimmed
            );
            has_status_running = true;
        }

        // Elicitation dialog detection: "Enter to select · ↑/↓ to navigate · Esc to cancel"
        if !has_elicitation && trimmed.starts_with("Enter to select") {
            log::debug!(
                "check_claude_pane_content({}): elicitation detected: {:?}",
                pane_id,
                trimmed
            );
            has_elicitation = true;
        }

        // Agent mode detection: plan, bypass, accept
        for &(pattern, label) in MODE_PATTERNS {
            if !agent_modes.iter().any(|m| m == label) && trimmed.contains(pattern) {
                log::debug!(
                    "check_claude_pane_content({}): mode '{}' detected: {:?}",
                    pane_id,
                    label,
                    trimmed
                );
                agent_modes.push(label.to_string());
            }
        }
    }

    // Idle detection: walk from bottom, skip TUI footer, check if first
    // meaningful line is a prompt (❯, $, %, >)
    let at_prompt = is_prompt_line(&all_lines);
    if at_prompt {
        log::debug!(
            "check_claude_pane_content({}): prompt line detected",
            pane_id
        );
    }

    ClaudePaneContentInfo {
        has_spinner,
        has_status_running,
        at_prompt,
        has_elicitation,
        agent_modes,
    }
}

/// Check if a line indicates Claude Code is actively running.
/// Matches spinner characters followed by "esc to interrupt" or "…" (ellipsis).
fn is_claude_running_line(line: &str) -> bool {
    if let Some(c) = line.chars().next() {
        if SPINNER_CHARS.contains(&c) {
            // Spinner char + "esc to interrupt"
            // e.g., "✻ Thinking… (esc to interrupt · 30s · ...)"
            if line.contains("esc to interrupt") {
                return true;
            }
            // Spinner char + "…" (active progress indicator)
            // e.g., "✶ Galloping…", "✻ Thinking…", "✢ Compacting…"
            if line.contains('…') {
                return true;
            }
        }
    }
    // "esc to interrupt" in status line suffix
    // e.g., "4 files +20 -0 · esc to interrupt"
    if line.contains("· esc to interrupt") {
        return true;
    }
    false
}

/// Check if the last meaningful line is a prompt, skipping TUI footer lines.
/// Walks from bottom to top, skipping empty lines, separators, status bar,
/// and help text. Up to MAX_UNKNOWN_LINES non-prompt lines are tolerated
/// (e.g. user-configured statusline) before giving up.
fn is_prompt_line(lines: &[&str]) -> bool {
    const MAX_UNKNOWN_LINES: usize = 3;
    let mut unknown_count = 0;

    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_separator_line(trimmed) {
            continue;
        }
        // Mode indicator: ⏵⏵ bypass permissions, ⏸ plan mode
        if trimmed.starts_with('⏵') || trimmed.starts_with('⏸') {
            continue;
        }
        // ctrl shortcut hints (e.g., "ctrl+b ctrl+b (twice) to run in background",
        // "ctrl-g to edit in Nvim")
        if trimmed.contains("ctrl+") || trimmed.contains("ctrl-") {
            continue;
        }
        // Context auto-compact warning (e.g., "Context left until auto-compact: 8%")
        if trimmed.contains("Context left until auto-compact") {
            continue;
        }
        // Skip Claude Code TUI footer lines
        if trimmed.contains("for shortcuts")
            || trimmed.contains("shift+tab to cycle")
            || is_file_changes_line(trimmed)
        {
            continue;
        }
        // Claude Code elicitation numbered options (e.g., "  2. Yes, and bypass permissions")
        // Skip these so we can reach the ❯-prefixed selected option line underneath.
        if is_numbered_option(trimmed) {
            continue;
        }
        // Claude Code elicitation navigation hint
        // e.g., "Enter to select · ↑/↓ to navigate · Esc to cancel"
        if trimmed.starts_with("Enter to select") {
            continue;
        }
        // Meaningful line: strip box border (│ ... │) then check prompt
        let check = strip_box_border(trimmed);
        if check.starts_with('❯')         // starship / Claude Code prompt
            || check.ends_with("$ ")       // bash
            || check == "$"
            || check.ends_with("% ")       // zsh
            || check == "%"
            || check == ">"                // REPL prompt
            || check.starts_with("> ")
        {
            return true;
        }
        // Non-prompt meaningful line (e.g. statusline). Tolerate up to
        // MAX_UNKNOWN_LINES before concluding agent is not at a prompt.
        unknown_count += 1;
        if unknown_count >= MAX_UNKNOWN_LINES {
            return false;
        }
    }
    false
}

/// Check if a line consists entirely of box-drawing characters (U+2500..U+257F).
fn is_separator_line(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| ('\u{2500}'..='\u{257F}').contains(&c))
}

/// Strip leading/trailing box drawing vertical bar (│ U+2502) and whitespace.
/// Used to detect prompts inside Claude Code's bordered input box.
fn strip_box_border(line: &str) -> &str {
    line.trim_start_matches('│')
        .trim_start()
        .trim_end_matches('│')
        .trim_end()
}

/// Check if a line is a Claude Code elicitation numbered option (e.g., "  2. Yes, and bypass permissions").
/// These appear in plan approval and other selection dialogs.
fn is_numbered_option(line: &str) -> bool {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => chars.as_str().starts_with(". "),
        _ => false,
    }
}

/// Check if a line shows file changes (e.g., "4 files +42 -0").
fn is_file_changes_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        && trimmed.contains("file")
        && (trimmed.contains('+') || trimmed.contains('-'))
}

fn detect_agent(tree: &ProcessTree, pane_pid: u32) -> Option<String> {
    // DFS through descendants of pane_pid
    let mut stack = vec![pane_pid];
    let mut visited = std::collections::HashSet::new();
    while let Some(current) = stack.pop() {
        if !visited.insert(current) {
            continue;
        }
        if let Some(child_pids) = tree.children.get(&current) {
            for &child in child_pids {
                if let Some(comm) = tree.commands.get(&child) {
                    let basename = comm.rsplit('/').next().unwrap_or(comm);
                    for (process_name, agent_type) in AGENT_PROCESSES {
                        if basename == *process_name {
                            return Some(agent_type.to_string());
                        }
                    }
                }
                stack.push(child);
            }
        }
    }
    None
}

// ──────────────────────────────────────────────────────────
// Codex-specific agent status detection
// ──────────────────────────────────────────────────────────

struct CodexPaneContentInfo {
    is_running: bool, // "(XXs • esc to interrupt)" pattern
    at_prompt: bool,  // › (U+203A) prompt character
}

fn detect_codex_status(
    db_conn: &Option<db::Connection>,
    pane_id: &str,
) -> (AgentStatus, Vec<String>) {
    let info = check_codex_pane_content(pane_id);

    log::debug!(
        "detect_codex_status({}): running={} prompt={}",
        pane_id,
        info.is_running,
        info.at_prompt
    );

    let status = if info.is_running {
        AgentStatus::Running
    } else if info.at_prompt {
        if let Some(conn) = db_conn {
            if let Ok(Some(_)) = db::get_latest_notification_by_pane(conn, pane_id) {
                AgentStatus::Waiting
            } else {
                AgentStatus::Idle
            }
        } else {
            AgentStatus::Idle
        }
    } else {
        // No clear signal — default to Running (conservative)
        AgentStatus::Running
    };

    // Codex has no mode indicators (plan/bypass/accept)
    (status, Vec::new())
}

fn check_codex_pane_content(pane_id: &str) -> CodexPaneContentInfo {
    let default = CodexPaneContentInfo {
        is_running: false,
        at_prompt: false,
    };

    let tmux_path = match find_tmux() {
        Some(p) => p,
        None => {
            log::debug!("check_codex_pane_content: tmux not found");
            return default;
        }
    };

    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args(["capture-pane", "-t", pane_id, "-p"])
        .output()
        .ok();

    let Some(output) = output else {
        log::debug!(
            "check_codex_pane_content({}): capture-pane exec failed",
            pane_id
        );
        return default;
    };
    if !output.status.success() {
        log::debug!(
            "check_codex_pane_content({}): capture-pane exit={} stderr={}",
            pane_id,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return default;
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let all_lines: Vec<&str> = content.lines().collect();

    // Get last 30 non-empty lines for scanning
    let last_lines: Vec<&str> = all_lines
        .iter()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(30)
        .copied()
        .collect();

    log::debug!(
        "check_codex_pane_content({}): last lines (bottom→up, first 5): {:?}",
        pane_id,
        &last_lines[..last_lines.len().min(5)]
    );

    let mut is_running = false;

    for line in &last_lines {
        let trimmed = line.trim();
        if !is_running && is_codex_running_line(trimmed) {
            log::debug!(
                "check_codex_pane_content({}): running detected: {:?}",
                pane_id,
                trimmed
            );
            is_running = true;
        }
    }

    let at_prompt = is_codex_prompt_line(&all_lines);
    if at_prompt {
        log::debug!(
            "check_codex_pane_content({}): prompt line detected",
            pane_id
        );
    }

    CodexPaneContentInfo {
        is_running,
        at_prompt,
    }
}

/// Check if a line indicates Codex is actively running.
/// Matches the pattern "(XXs • esc to interrupt)" where XX is a duration.
/// e.g., "• Working (48s • esc to interrupt) · 1 background terminal running"
fn is_codex_running_line(line: &str) -> bool {
    line.contains("s \u{2022} esc to interrupt") && line.contains('(')
}

/// Check if the last meaningful line is a Codex prompt (›), skipping footer lines.
fn is_codex_prompt_line(lines: &[&str]) -> bool {
    const MAX_UNKNOWN_LINES: usize = 3;
    let mut unknown_count = 0;

    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_codex_footer_line(trimmed) {
            continue;
        }
        // › (U+203A SINGLE RIGHT-POINTING ANGLE QUOTATION MARK) is the Codex prompt
        if trimmed.starts_with('\u{203A}') {
            return true;
        }
        unknown_count += 1;
        if unknown_count >= MAX_UNKNOWN_LINES {
            return false;
        }
    }
    false
}

/// Check if a line is a Codex TUI footer element that should be skipped.
fn is_codex_footer_line(line: &str) -> bool {
    line.contains("for shortcuts")
        || line.contains("context left")
        || line.contains("background terminal running")
        || line.contains("/ps to view")
        || line.contains("/clean to close")
}
