//! Detect which AI coding agent (if any) is running inside a tmux pane.
//!
//! Shared between the GUI (per-pane session scanning in `sessions::mod`) and the
//! CLI (`send-keys` guard that refuses to message a pane with no agent). Keeping
//! `AGENT_PROCESSES` here as the single source of truth means the two callers can
//! never disagree about "what counts as an agent" — e.g. adding a fifth agent
//! updates both the GUI display and the `send-keys` guard at once.

use std::collections::HashMap;
use std::process::Command;

/// `(process basename, agent_type)` pairs. A pane is considered to be running an
/// agent when any descendant process matches one of these names.
pub const AGENT_PROCESSES: &[(&str, &str)] = &[
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("copilot", "copilot-cli"),
    ("opencode", "opencode"),
    (".opencode", "opencode"), // mise/npm install: actual Go binary is named .opencode
];

/// Process tree: maps parent PID to child PIDs, plus each PID's command string.
pub struct ProcessTree {
    children: HashMap<u32, Vec<u32>>,
    commands: HashMap<u32, String>,
}

impl ProcessTree {
    /// Number of processes with a recorded command (for diagnostics/logging).
    pub fn process_count(&self) -> usize {
        self.commands.len()
    }

    /// Number of distinct parent PIDs with at least one child (for diagnostics).
    pub fn parent_count(&self) -> usize {
        self.children.len()
    }
}

/// Build the full process tree from `/bin/ps -eo pid,ppid,comm`.
pub fn build_process_tree() -> ProcessTree {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut commands: HashMap<u32, String> = HashMap::new();

    let output = match Command::new("/bin/ps")
        .args(["-eo", "pid,ppid,comm"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::error!("agent_detect: /bin/ps exec failed: {}", e);
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

/// Walk the descendants of `pane_pid` and return the first matching agent type.
pub fn detect_agent(tree: &ProcessTree, pane_pid: u32) -> Option<String> {
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
                    // Agent Teams spawns the versioned binary directly (e.g. /…/claude/versions/2.1.59)
                    if comm.contains("/claude/versions/") {
                        return Some("claude-code".to_string());
                    }
                }
                stack.push(child);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(edges: &[(u32, u32)], commands: &[(u32, &str)]) -> ProcessTree {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(ppid, pid) in edges {
            children.entry(ppid).or_default().push(pid);
        }
        let commands = commands
            .iter()
            .map(|&(pid, c)| (pid, c.to_string()))
            .collect();
        ProcessTree { children, commands }
    }

    #[test]
    fn detects_claude_among_descendants() {
        // pane(100) -> zsh(200) -> node(300) -> claude(400)
        let t = tree(
            &[(100, 200), (200, 300), (300, 400)],
            &[
                (200, "zsh"),
                (300, "node"),
                (400, "/opt/homebrew/bin/claude"),
            ],
        );
        assert_eq!(detect_agent(&t, 100).as_deref(), Some("claude-code"));
    }

    #[test]
    fn detects_versioned_claude_binary() {
        // Agent Teams spawns the versioned binary directly; the path contains
        // "/claude/versions/" (the install dir), not the basename "claude".
        let t = tree(
            &[(100, 200)],
            &[(200, "/Users/me/.local/share/claude/versions/2.1.59")],
        );
        assert_eq!(detect_agent(&t, 100).as_deref(), Some("claude-code"));
    }

    #[test]
    fn detects_dot_opencode_binary() {
        let t = tree(&[(100, 200)], &[(200, "/home/me/.local/bin/.opencode")]);
        assert_eq!(detect_agent(&t, 100).as_deref(), Some("opencode"));
    }

    #[test]
    fn plain_shell_has_no_agent() {
        // pane(100) -> zsh(200), nothing else
        let t = tree(&[(100, 200)], &[(200, "zsh")]);
        assert_eq!(detect_agent(&t, 100), None);
    }

    #[test]
    fn substring_match_does_not_false_positive() {
        // A process whose name merely contains "claude" must not match.
        let t = tree(&[(100, 200)], &[(200, "claude-helper")]);
        assert_eq!(detect_agent(&t, 100), None);
    }
}
