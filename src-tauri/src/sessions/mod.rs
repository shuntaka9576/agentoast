use std::collections::{HashMap, HashSet};
use std::process::Command;

use agentoast_shared::db;
use agentoast_shared::git_info::{self, GitInfo};
use agentoast_shared::models::{TmuxPane, TmuxPaneGroup};

use crate::terminal::find_tmux;

pub(crate) mod agents;
pub mod hysteresis;
mod process_tree;
use agents::detect_agent_status_with_content;

use agentoast_shared::agent_detect::detect_agent;
use hysteresis::PaneHysteresis;
use std::sync::Mutex;
use std::time::Instant;

/// Debug-build spawn accounting so the per-cycle external-process count can
/// be verified from the logs (the whole point of the constant-spawn work).
#[cfg(debug_assertions)]
static SPAWN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn note_spawn() {
    #[cfg(debug_assertions)]
    SPAWN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn log_cycle_spawns() {
    #[cfg(debug_assertions)]
    log::debug!(
        "sessions: external process spawns this cycle = {}",
        SPAWN_COUNTER.swap(0, std::sync::atomic::Ordering::Relaxed)
    );
}

pub fn list_tmux_panes_grouped(
    show_non_agent: bool,
    db_conn: &Option<db::Connection>,
    hysteresis: Option<&Mutex<PaneHysteresis>>,
) -> Result<Vec<TmuxPaneGroup>, String> {
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
        note_spawn();
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

    // Build process tree once for all panes (in-process via sysinfo, no spawn)
    let process_tree = process_tree::build_process_tree();
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

    // Git info comes from on-disk .git metadata (cached across cycles in
    // git_info) — file reads, no process spawn, so no need to parallelize.
    let unique_paths: Vec<String> = raw_panes
        .iter()
        .map(|p| p.current_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let git_cache: HashMap<String, Option<GitInfo>> = unique_paths
        .iter()
        .map(|p| (p.clone(), git_info::resolve_git_info(p)))
        .collect();

    // Capture every agent pane's content with a single tmux invocation.
    let agent_pane_indices: Vec<usize> = raw_panes
        .iter()
        .enumerate()
        .filter(|(_, rp)| rp.agent_type.is_some())
        .map(|(i, _)| i)
        .collect();
    let agent_pane_ids: Vec<&str> = agent_pane_indices
        .iter()
        .map(|&idx| raw_panes[idx].pane_id.as_str())
        .collect();
    let mut batch = agents::capture_panes_batch(&agent_pane_ids);
    let captured_contents: HashMap<usize, Option<String>> = agent_pane_indices
        .iter()
        .map(|&idx| {
            let content = batch.remove(raw_panes[idx].pane_id.as_str()).flatten();
            (idx, content)
        })
        .collect();

    // Single-shot hysteresis observation. The write lock spans only the
    // in-memory hash computation (no tmux / DB / git I/O), so contention
    // with `emit_cached_sessions` and the get_sessions command stays
    // bounded even with hundreds of panes.
    let last_changed_map: HashMap<String, Instant> = if let Some(h) = hysteresis {
        let mut guard = h.lock().unwrap_or_else(|e| e.into_inner());
        let observed = guard.observe_batch(agent_pane_indices.iter().filter_map(|&idx| {
            let rp = &raw_panes[idx];
            let agent_type = rp.agent_type.as_deref()?;
            let content = captured_contents.get(&idx).and_then(|c| c.as_deref())?;
            Some((rp.pane_id.as_str(), agent_type, content))
        }));
        guard.retain(raw_panes.iter().map(|p| p.pane_id.as_str()));
        observed
    } else {
        HashMap::new()
    };

    // Build TmuxPane with git info and agent status (DB lookups on main thread)
    let panes: Vec<TmuxPane> = raw_panes
        .into_iter()
        .enumerate()
        .map(|(idx, rp)| {
            let git_info = git_cache.get(&rp.current_path).and_then(|o| o.as_ref());
            let (agent_status, waiting_reason, agent_modes, team_role, team_name) =
                if let Some(ref at) = rp.agent_type {
                    let content = captured_contents.get(&idx).and_then(|c| c.as_deref());
                    // Map entry absent ⇒ first observation, input region
                    // unlocatable, or non-Claude agent — all collapse to
                    // "no hash assist for this pane this cycle".
                    let last_changed = last_changed_map.get(&rp.pane_id).copied();
                    let r = detect_agent_status_with_content(
                        db_conn,
                        &rp.pane_id,
                        at,
                        content,
                        last_changed,
                    );
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

    log_cycle_spawns();

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

    let process_tree = process_tree::build_process_tree();
    let agent_type = detect_agent(&process_tree, pane_pid);

    let git_info = git_info::resolve_git_info(&current_path);

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
