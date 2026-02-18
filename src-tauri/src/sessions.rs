use std::collections::HashMap;
use std::process::Command;

use agentoast_shared::models::{TmuxPane, TmuxPaneGroup};

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
        "#{{pane_id}}{d}#{{pane_pid}}{d}#{{session_name}}{d}#{{window_name}}{d}#{{pane_current_path}}",
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
        agent_type: Option<String>,
    }

    let mut raw_panes: Vec<RawPane> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(5, DELIM).collect();
        if parts.len() < 5 {
            continue;
        }

        let pane_pid: u32 = parts[1].parse().unwrap_or(0);
        let agent_type = detect_agent(&process_tree, pane_pid);
        log::debug!(
            "sessions: pane {} pid={} agent={:?}",
            parts[0],
            pane_pid,
            agent_type
        );

        raw_panes.push(RawPane {
            pane_id: parts[0].to_string(),
            pane_pid,
            session_name: parts[2].to_string(),
            window_name: parts[3].to_string(),
            current_path: parts[4].to_string(),
            agent_type,
        });
    }
    log::debug!("sessions: parsed {} panes total", raw_panes.len());

    // Resolve git info for all unique paths
    let unique_paths: Vec<String> = raw_panes
        .iter()
        .map(|p| p.current_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let git_cache = resolve_git_info(&unique_paths);

    // Build TmuxPane with git info
    let panes: Vec<TmuxPane> = raw_panes
        .into_iter()
        .map(|rp| {
            let git_info = git_cache.get(&rp.current_path).and_then(|o| o.as_ref());
            TmuxPane {
                pane_id: rp.pane_id,
                pane_pid: rp.pane_pid,
                session_name: rp.session_name,
                window_name: rp.window_name,
                current_path: rp.current_path,
                agent_type: rp.agent_type,
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
