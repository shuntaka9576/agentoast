use std::collections::{HashMap, HashSet};
use std::process::Command;

use agentoast_shared::models::{TmuxPane, TmuxPaneGroup};
use agentoast_shared::{config, db};

use crate::terminal::{find_git, find_tmux};

pub(crate) mod agents;
use agents::{capture_pane, detect_agent_status_with_content};

use agentoast_shared::agent_detect::{build_process_tree, detect_agent};

struct GitInfo {
    repo_root: String,
    repo_name: String,
    branch: Option<String>,
}

/// Resolve git info for a single path (3 git commands: rev-parse, remote, branch).
fn resolve_single_git_info(git_path: &std::path::Path, path: &str) -> Option<GitInfo> {
    // git rev-parse --show-toplevel
    let repo_root = Command::new(git_path)
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
        })?;

    // git remote get-url origin → extract repo name from URL
    let repo_name = Command::new(git_path)
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
            repo_root
                .rsplit('/')
                .next()
                .unwrap_or(&repo_root)
                .to_string()
        });

    // git branch --show-current
    let branch = Command::new(git_path)
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
        repo_root,
        repo_name,
        branch,
    })
}

/// Resolve git info for each unique path in parallel.
fn resolve_git_info(paths: &[String]) -> HashMap<String, Option<GitInfo>> {
    let git_path = match find_git() {
        Some(p) => p,
        None => {
            return paths.iter().map(|p| (p.clone(), None)).collect();
        }
    };

    // Deduplicate paths
    let unique: Vec<&String> = paths
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Resolve git info in parallel (one thread per unique path)
    std::thread::scope(|s| {
        let handles: Vec<_> = unique
            .iter()
            .map(|path| {
                let git_path = &git_path;
                let path_str = path.as_str();
                s.spawn(move || {
                    (
                        path_str.to_string(),
                        resolve_single_git_info(git_path, path_str),
                    )
                })
            })
            .collect();

        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
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

pub fn list_tmux_panes_grouped(show_non_agent: bool) -> Result<Vec<TmuxPaneGroup>, String> {
    log::info!(
        "sessions: get_sessions called (show_non_agent={})",
        show_non_agent
    );
    log::info!(
        "sessions: TMPDIR={:?}, TMUX_TMPDIR={:?}",
        std::env::var("TMPDIR").ok(),
        std::env::var("TMUX_TMPDIR").ok()
    );

    // Use "|||" as delimiter instead of "\t" because macOS Launch Services
    // (Finder double-click) sanitizes control characters in process arguments,
    // converting tabs to underscores.
    const DELIM: &str = "|||";
    let format_str = format!(
        "#{{pane_id}}{d}#{{pane_pid}}{d}#{{session_name}}{d}#{{window_name}}{d}#{{pane_current_path}}{d}#{{pane_active}}{d}#{{window_active}}{d}#{{session_attached}}{d}#{{pane_current_command}}",
        d = DELIM
    );

    let stdout_lines: Vec<String> = {
        let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;
        log::debug!("sessions: tmux found at {:?}", tmux_path);
        let output = Command::new(&tmux_path)
            .env_remove("TMPDIR")
            .args(["list-panes", "-a", "-F", &format_str])
            .output()
            .map_err(|e| {
                log::error!("sessions: tmux list-panes exec failed: {}", e);
                format!("tmux list-panes failed: {}", e)
            })?;
        if !output.status.success() {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            log::error!("sessions: tmux list-panes returned error: {}", stderr_str);
            return Err(format!("tmux list-panes failed: {}", stderr_str));
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect()
    };

    log::info!("sessions: tmux list-panes {} lines", stdout_lines.len());

    // Build process tree once for all panes
    let process_tree = build_process_tree();
    log::debug!(
        "sessions: process tree: {} processes, {} parent entries",
        process_tree.process_count(),
        process_tree.parent_count()
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
        current_command: Option<String>,
    }

    let mut raw_panes: Vec<RawPane> = Vec::new();

    for line in &stdout_lines {
        let parts: Vec<&str> = line.splitn(9, DELIM).collect();
        if parts.len() < 9 {
            continue;
        }

        let pane_pid: u32 = parts[1].parse().unwrap_or(0);
        let agent_type = detect_agent(&process_tree, pane_pid);
        let raw_attached: u32 = parts[7].parse().unwrap_or(0);
        let is_active = parts[5] == "1" && parts[6] == "1" && raw_attached >= 1;
        log::debug!(
            "sessions: pane {} pid={} agent={:?} is_active={}",
            parts[0],
            pane_pid,
            agent_type,
            is_active
        );

        let current_command = if parts[8].is_empty() {
            None
        } else {
            Some(parts[8].to_string())
        };

        raw_panes.push(RawPane {
            pane_id: parts[0].to_string(),
            pane_pid,
            session_name: parts[2].to_string(),
            window_name: parts[3].to_string(),
            current_path: parts[4].to_string(),
            is_active,
            agent_type,
            current_command,
        });
    }
    log::debug!("sessions: parsed {} panes total", raw_panes.len());

    // Open DB connection for agent status detection (read-only, no schema init)
    let db_conn = db::open_reader(&config::db_path()).ok();

    // Collect unique paths and agent panes for parallel execution
    let unique_paths: Vec<String> = raw_panes
        .iter()
        .map(|p| p.current_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Collect agent pane indices for parallel capture-pane
    let agent_pane_indices: Vec<usize> = raw_panes
        .iter()
        .enumerate()
        .filter(|(_, rp)| rp.agent_type.is_some())
        .map(|(i, _)| i)
        .collect();

    // Run git info resolution and capture-pane in parallel
    let (git_cache, captured_contents) = std::thread::scope(|s| {
        // Git info: one thread per unique path
        let git_handle = s.spawn(|| resolve_git_info(&unique_paths));

        // Capture-pane: fan out threads to parallelize fork+exec overhead.
        let captured: HashMap<usize, Option<String>> = {
            let handles: Vec<_> = agent_pane_indices
                .iter()
                .map(|&idx| {
                    let pane_id = raw_panes[idx].pane_id.as_str();
                    s.spawn(move || (idx, capture_pane(pane_id)))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        };

        let git_cache = git_handle.join().unwrap();

        (git_cache, captured)
    });

    // Build TmuxPane with git info and agent status (DB lookups on main thread)
    let panes: Vec<TmuxPane> = raw_panes
        .into_iter()
        .enumerate()
        .map(|(idx, rp)| {
            let git_info = git_cache.get(&rp.current_path).and_then(|o| o.as_ref());
            let (agent_status, waiting_reason, agent_modes, team_role, team_name) =
                if let Some(ref at) = rp.agent_type {
                    let content = captured_contents.get(&idx).and_then(|c| c.as_deref());
                    let r = detect_agent_status_with_content(&db_conn, &rp.pane_id, at, content);
                    (
                        Some(r.status),
                        r.waiting_reason,
                        r.agent_modes,
                        r.team_role,
                        r.team_name,
                    )
                } else {
                    (None, None, Vec::new(), None, None)
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
                waiting_reason,
                agent_modes,
                team_role,
                team_name,
                git_repo_root: git_info.map(|g| g.repo_root.clone()),
                git_branch: git_info.and_then(|g| g.branch.clone()),
                current_command: rp.current_command,
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

    if !show_non_agent {
        // Promote is_active to a sibling agent pane when the tmux-focused pane is
        // a shell (not an agent) in the same tmux window. Match by (session, window)
        // rather than by group key — panes in the same window can have different
        // current_paths, which puts them in different git-rooted groups.
        let active_windows: HashSet<(String, String)> = groups
            .iter()
            .flat_map(|g| g.panes.iter())
            .filter(|p| p.is_active)
            .map(|p| (p.session_name.clone(), p.window_name.clone()))
            .collect();
        for group in &mut groups {
            // Only promote within the group that actually hosts the attached pane.
            // The same tmux (session, window) can span multiple worktree groups;
            // promoting across groups makes the UI cursor land on the wrong worktree.
            let has_active_in_group = group.panes.iter().any(|p| p.is_active);
            if has_active_in_group {
                let any_agent_active = group
                    .panes
                    .iter()
                    .any(|p| p.is_active && p.agent_type.is_some());
                if !any_agent_active {
                    if let Some(first_agent) = group.panes.iter_mut().find(|p| {
                        p.agent_type.is_some()
                            && active_windows
                                .contains(&(p.session_name.clone(), p.window_name.clone()))
                    }) {
                        log::debug!(
                            "sessions: promoted is_active to agent pane {} (session={} window={})",
                            first_agent.pane_id,
                            first_agent.session_name,
                            first_agent.window_name
                        );
                        first_agent.is_active = true;
                    }
                }
            }
            group.panes.retain(|p| p.agent_type.is_some());
        }
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

/// Return the tmux pane that is actually focused right now, without the
/// agent-promotion logic that `list_tmux_panes_grouped` applies when
/// `show_non_agent=false`. Returns `Ok(None)` when no pane is attached.
pub fn find_focused_pane() -> Result<Option<TmuxPane>, String> {
    const DELIM: &str = "|||";
    let format_str = format!(
        "#{{pane_id}}{d}#{{pane_pid}}{d}#{{session_name}}{d}#{{window_name}}{d}#{{pane_current_path}}{d}#{{pane_active}}{d}#{{window_active}}{d}#{{session_attached}}{d}#{{pane_current_command}}",
        d = DELIM
    );

    let stdout_lines: Vec<String> = {
        let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;
        let output = Command::new(&tmux_path)
            .env_remove("TMPDIR")
            .args(["list-panes", "-a", "-F", &format_str])
            .output()
            .map_err(|e| format!("tmux list-panes failed: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "tmux list-panes failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect()
    };

    let mut focused: Option<(String, u32, String, String, String, Option<String>)> = None;
    for line in &stdout_lines {
        let parts: Vec<&str> = line.splitn(9, DELIM).collect();
        if parts.len() < 9 {
            continue;
        }
        let raw_attached: u32 = parts[7].parse().unwrap_or(0);
        let is_active = parts[5] == "1" && parts[6] == "1" && raw_attached >= 1;
        if !is_active {
            continue;
        }
        let pane_pid: u32 = parts[1].parse().unwrap_or(0);
        let current_command = if parts[8].is_empty() {
            None
        } else {
            Some(parts[8].to_string())
        };
        focused = Some((
            parts[0].to_string(),
            pane_pid,
            parts[2].to_string(),
            parts[3].to_string(),
            parts[4].to_string(),
            current_command,
        ));
        break;
    }

    let Some((pane_id, pane_pid, session_name, window_name, current_path, current_command)) =
        focused
    else {
        return Ok(None);
    };

    let process_tree = build_process_tree();
    let agent_type = detect_agent(&process_tree, pane_pid);

    let git_info =
        find_git().and_then(|git_path| resolve_single_git_info(&git_path, &current_path));

    Ok(Some(TmuxPane {
        pane_id,
        pane_pid,
        session_name,
        window_name,
        current_path,
        is_active: true,
        agent_type,
        agent_status: None,
        waiting_reason: None,
        agent_modes: Vec::new(),
        team_role: None,
        team_name: None,
        git_repo_root: git_info.as_ref().map(|g| g.repo_root.clone()),
        git_branch: git_info.as_ref().and_then(|g| g.branch.clone()),
        current_command,
    }))
}
