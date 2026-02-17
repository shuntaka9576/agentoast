use std::collections::HashMap;
use std::process::Command;

use agentoast_shared::models::{TmuxPane, TmuxPaneGroup};

use crate::terminal::find_tmux;

const AGENT_PROCESSES: &[(&str, &str)] = &[
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

pub fn list_tmux_panes_grouped() -> Result<Vec<TmuxPaneGroup>, String> {
    let tmux_path = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    let output = Command::new(&tmux_path)
        .env_remove("TMPDIR")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}\t#{pane_pid}\t#{session_name}\t#{window_name}\t#{pane_current_path}",
        ])
        .output()
        .map_err(|e| format!("tmux list-panes failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tmux list-panes failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Build process tree once for all panes
    let process_tree = build_process_tree();

    let mut panes: Vec<TmuxPane> = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 5 {
            continue;
        }

        let pane_pid: u32 = parts[1].parse().unwrap_or(0);
        let agent_type = detect_agent(&process_tree, pane_pid);

        panes.push(TmuxPane {
            pane_id: parts[0].to_string(),
            pane_pid,
            session_name: parts[2].to_string(),
            window_name: parts[3].to_string(),
            current_path: parts[4].to_string(),
            agent_type,
        });
    }

    // Group by current_path
    let mut groups_map: HashMap<String, Vec<TmuxPane>> = HashMap::new();
    for pane in panes {
        groups_map
            .entry(pane.current_path.clone())
            .or_default()
            .push(pane);
    }

    let mut groups: Vec<TmuxPaneGroup> = groups_map
        .into_iter()
        .map(|(path, panes)| {
            let repo_name = path.rsplit('/').next().unwrap_or(&path).to_string();
            TmuxPaneGroup {
                repo_name,
                current_path: path,
                panes,
            }
        })
        .collect();

    // Keep only panes with active agents, remove empty groups
    for group in &mut groups {
        group.panes.retain(|p| p.agent_type.is_some());
    }
    groups.retain(|g| !g.panes.is_empty());

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

    let output = match Command::new("ps").args(["-eo", "pid,ppid,comm"]).output() {
        Ok(o) => o,
        Err(_) => {
            return ProcessTree {
                children,
                commands,
            }
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
    // BFS through descendants of pane_pid
    let mut queue = vec![pane_pid];
    while let Some(current) = queue.pop() {
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
                queue.push(child);
            }
        }
    }
    None
}
