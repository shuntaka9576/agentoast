//! In-process replacement for `/bin/ps -eo pid,ppid,comm`.
//!
//! The session poller needs the full pid → (ppid, command) table every cycle
//! to find agent processes under each tmux pane. Spawning `ps` costs a
//! fork+exec per cycle; sysinfo reads the same data straight from
//! `sysctl(KERN_PROC_ALL)` + `proc_pidpath` (no privileges needed for
//! same-user processes — which is all the agent panes are).
//!
//! `Process::exe()` yields the resolved executable path, where `ps comm`
//! showed the exec-time argv path. Both forms satisfy `detect_agent`: the
//! basename match covers symlink invocations (`claude`) and the
//! `/claude/versions/` substring match covers resolved install paths.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use agentoast_shared::agent_detect::ProcessTree;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Reused across cycles so the process map isn't reallocated every 2s.
static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

pub(crate) fn build_process_tree() -> ProcessTree {
    let system = SYSTEM.get_or_init(|| Mutex::new(System::new()));
    let mut sys = match system.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );

    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut commands: HashMap<u32, String> = HashMap::new();
    for (pid, process) in sys.processes() {
        let comm = process
            .exe()
            .map(|p| p.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| process.name().to_string_lossy().into_owned());
        if comm.is_empty() {
            continue;
        }
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        children.entry(ppid).or_default().push(pid.as_u32());
        commands.insert(pid.as_u32(), comm);
    }

    if commands.is_empty() {
        // Defensive fallback: an empty table would make every pane look
        // agent-less, so fall back to the ps-based scan rather than lie.
        log::warn!("process_tree: sysinfo returned no processes, falling back to /bin/ps");
        return agentoast_shared::agent_detect::build_process_tree();
    }

    ProcessTree::from_maps(children, commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_own_process_with_parent() {
        let tree = build_process_tree();
        // The test binary itself must be present under its parent.
        let me = std::process::id();
        let parent = std::os::unix::process::parent_id();
        assert!(tree.process_count() > 10);
        let children = tree.children_of(parent);
        assert!(
            children.contains(&me),
            "own pid {} not found under parent {}",
            me,
            parent
        );
    }
}
