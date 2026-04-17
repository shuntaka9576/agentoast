//! tmux control mode (`tmux -C`) client.
//!
//! Maintains a long-lived tmux client process over a single pipe, avoiding per-call
//! fork+exec overhead. Commands are serialized through a worker thread: each caller
//! sends a command string, gets back the response lines collected between tmux's
//! `%begin` / `%end` markers. Asynchronous notifications (window-add etc.) flow into
//! a separate listener channel for event-driven cache invalidation.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use crate::terminal::find_tmux;

const HANDSHAKE_SENTINEL: &str = "__agentoast_ctrl_ready__";
const RECONNECT_BACKOFF: Duration = Duration::from_secs(3);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// Session name left behind by the previous (dedicated-session) implementation.
/// Cleaned up on startup and excluded from attach candidates as a safety net.
const LEGACY_CTRL_SESSION_NAME: &str = "_agentoast_ctrl";

#[derive(Clone)]
pub struct TmuxCtrl {
    tx: Sender<Request>,
    /// Name of the tmux session this control client is currently attached to.
    /// Updated by `run_session` on every (re)connect. Consumers rely on this
    /// to correct `session_attached` counts (we contribute +1 to our target
    /// session, which would otherwise skew the "is this pane focused" check
    /// in `list_tmux_panes_grouped`).
    attached_session: Arc<RwLock<Option<String>>>,
}

enum Request {
    Cmd {
        cmd: String,
        reply: Sender<Response>,
    },
    ReaderEvent(ReaderEvent),
}

type Response = Result<Vec<String>, String>;

enum ReaderEvent {
    Begin,
    End,
    Error,
    /// Any non-marker line. May start with `%` (tmux pane IDs are `%NN`), so
    /// the dispatcher — not the parser — decides whether it's block content or
    /// an async notification based on whether we're currently inside a block.
    Raw(String),
    Eof,
}

/// Topology change notification emitted by the reader when tmux reports
/// window/session add/close/rename events.
#[derive(Debug, Clone, Copy)]
pub struct TopologyChanged;

impl TmuxCtrl {
    pub fn spawn(notifier: Option<Sender<TopologyChanged>>) -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let self_tx = tx.clone();
        let attached_session: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
        let attached_session_worker = Arc::clone(&attached_session);
        thread::spawn(move || loop {
            let result = run_session(&rx, &self_tx, notifier.as_ref(), &attached_session_worker);
            // Always clear the attached-session record on exit so stale names
            // don't leak into the next reconnect cycle.
            if let Ok(mut guard) = attached_session_worker.write() {
                *guard = None;
            }
            match result {
                Ok(()) => {
                    log::info!("tmux ctrl: session ended cleanly");
                    return;
                }
                Err(e) => {
                    log::warn!("tmux ctrl: {}. reconnecting in {:?}", e, RECONNECT_BACKOFF);
                    thread::sleep(RECONNECT_BACKOFF);
                }
            }
        });
        Self {
            tx,
            attached_session,
        }
    }

    /// Send a tmux command and wait for its response (all lines between `%begin` and `%end`).
    pub fn send(&self, cmd: &str) -> Result<Vec<String>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(Request::Cmd {
                cmd: cmd.to_string(),
                reply: reply_tx,
            })
            .map_err(|e| format!("ctrl send: {}", e))?;
        match reply_rx.recv_timeout(COMMAND_TIMEOUT) {
            Ok(res) => res,
            Err(e) => Err(format!("ctrl recv: {}", e)),
        }
    }

    /// Session name this control client is currently attached to, if any.
    /// Returns `None` while reconnecting or before the first successful attach.
    pub fn attached_session(&self) -> Option<String> {
        self.attached_session.read().ok().and_then(|g| g.clone())
    }
}

fn run_session(
    rx: &Receiver<Request>,
    self_tx: &Sender<Request>,
    notifier: Option<&Sender<TopologyChanged>>,
    attached_session: &Arc<RwLock<Option<String>>>,
) -> Result<(), String> {
    let tmux = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    cleanup_legacy_ctrl_session(&tmux);

    let target = pick_attach_target(&tmux)
        .ok_or_else(|| "no tmux session available to attach".to_string())?;
    log::info!("tmux ctrl: attaching to session '{}'", target);

    let mut child = Command::new(&tmux)
        .env_remove("TMPDIR")
        .args(["-C", "attach-session", "-t", &target])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn tmux -C: {}", e))?;

    let mut stdin = child.stdin.take().ok_or_else(|| "no stdin".to_string())?;
    let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
    let mut reader = BufReader::new(stdout);

    // Synchronous handshake: write a sentinel command and drain the initial
    // attach output until we see both the sentinel string and its closing `%end`.
    writeln!(stdin, "display-message -p '{}'", HANDSHAKE_SENTINEL)
        .map_err(|e| format!("handshake write: {}", e))?;
    stdin
        .flush()
        .map_err(|e| format!("handshake flush: {}", e))?;

    let mut seen_sentinel = false;
    let mut line = String::new();
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|e| format!("handshake read: {}", e))?
            == 0
        {
            return Err("handshake: EOF before sentinel".to_string());
        }
        let trimmed = line.trim_end();
        if trimmed.contains(HANDSHAKE_SENTINEL) {
            seen_sentinel = true;
        }
        if seen_sentinel && (trimmed.starts_with("%end ") || trimmed.starts_with("%error ")) {
            break;
        }
    }
    log::info!("tmux ctrl: handshake complete");

    // Publish our attached session name so `list_tmux_panes_grouped` can
    // subtract our contribution from `session_attached` when deciding which
    // pane is "the" focused one.
    if let Ok(mut guard) = attached_session.write() {
        *guard = Some(target.clone());
    }

    // After handshake, delegate stdout reading to a background thread that funnels
    // parsed events back into the same channel the dispatcher reads from.
    let reader_tx = self_tx.clone();
    thread::spawn(move || reader_loop(reader, reader_tx));

    enum State {
        Idle,
        Waiting {
            reply: Sender<Response>,
            lines: Vec<String>,
            in_block: bool,
            is_error: bool,
        },
    }

    let mut state = State::Idle;

    loop {
        let req = rx.recv().map_err(|e| format!("rx closed: {}", e))?;
        match req {
            Request::Cmd { cmd, reply } => match state {
                State::Idle => {
                    if let Err(e) = writeln!(stdin, "{}", cmd).and_then(|_| stdin.flush()) {
                        let _ = reply.send(Err(format!("stdin: {}", e)));
                        return Err(format!("stdin write: {}", e));
                    }
                    state = State::Waiting {
                        reply,
                        lines: Vec::new(),
                        in_block: false,
                        is_error: false,
                    };
                }
                State::Waiting { .. } => {
                    // send() serializes callers, so this should be unreachable.
                    let _ = reply.send(Err("ctrl busy".to_string()));
                }
            },
            Request::ReaderEvent(ev) => match ev {
                ReaderEvent::Begin => {
                    if let State::Waiting {
                        in_block,
                        lines,
                        is_error,
                        ..
                    } = &mut state
                    {
                        *in_block = true;
                        *is_error = false;
                        lines.clear();
                    }
                }
                ReaderEvent::End => {
                    if let State::Waiting {
                        reply,
                        lines,
                        is_error,
                        ..
                    } = std::mem::replace(&mut state, State::Idle)
                    {
                        let resp = if is_error {
                            Err(lines.join("\n"))
                        } else {
                            Ok(lines)
                        };
                        let _ = reply.send(resp);
                    }
                }
                ReaderEvent::Error => {
                    if let State::Waiting { reply, lines, .. } =
                        std::mem::replace(&mut state, State::Idle)
                    {
                        let _ = reply.send(Err(lines.join("\n")));
                    }
                }
                ReaderEvent::Raw(line) => match &mut state {
                    State::Waiting {
                        in_block: true,
                        lines,
                        ..
                    } => {
                        // Inside a %begin..%end block: raw lines are command output,
                        // even if they start with '%' (pane IDs like %71).
                        lines.push(line);
                    }
                    _ => {
                        // Outside any block: a '%'-prefixed line is an async
                        // notification from tmux. Everything else is ignored.
                        if let Some(rest) = line.strip_prefix('%') {
                            let kind = rest.split_whitespace().next().unwrap_or("");
                            if is_topology_event(kind) {
                                log::debug!("tmux ctrl: topology notification: {}", kind);
                                if let Some(n) = notifier {
                                    let _ = n.send(TopologyChanged);
                                }
                            }
                            if kind == "client-detached" {
                                // Our attached session was killed or the
                                // control client was detached. Bail out so
                                // the reconnect loop picks another session.
                                if let State::Waiting { reply, .. } =
                                    std::mem::replace(&mut state, State::Idle)
                                {
                                    let _ = reply.send(Err("tmux client-detached".to_string()));
                                }
                                return Err("tmux: client-detached".to_string());
                            }
                        }
                    }
                },
                ReaderEvent::Eof => {
                    if let State::Waiting { reply, .. } = std::mem::replace(&mut state, State::Idle)
                    {
                        let _ = reply.send(Err("tmux exited".to_string()));
                    }
                    return Err("tmux stdout EOF".to_string());
                }
            },
        }
    }
}

fn reader_loop(mut reader: BufReader<ChildStdout>, tx: Sender<Request>) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = tx.send(Request::ReaderEvent(ReaderEvent::Eof));
                return;
            }
            Ok(_) => {
                let ev = parse_line(line.trim_end());
                if tx.send(Request::ReaderEvent(ev)).is_err() {
                    return;
                }
            }
            Err(e) => {
                log::warn!("tmux ctrl reader io: {}", e);
                let _ = tx.send(Request::ReaderEvent(ReaderEvent::Eof));
                return;
            }
        }
    }
}

fn parse_line(line: &str) -> ReaderEvent {
    if line.starts_with("%begin ") {
        ReaderEvent::Begin
    } else if line.starts_with("%end ") {
        ReaderEvent::End
    } else if line.starts_with("%error ") {
        ReaderEvent::Error
    } else {
        ReaderEvent::Raw(line.to_string())
    }
}

/// Pick a user session to attach to via `tmux list-sessions`. Prefers sessions
/// that already have a client attached (where the user's GUI terminal is
/// pointing), so our control client doesn't fork off to a session the user
/// isn't looking at — which would otherwise double-count `session_attached`
/// on the real focused session. `None` means either no server is running or
/// no eligible session exists; the caller falls through to reconnect backoff.
fn pick_attach_target(tmux: &Path) -> Option<String> {
    let output = Command::new(tmux)
        .env_remove("TMPDIR")
        .args(["list-sessions", "-F", "#{session_attached} #{session_name}"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut candidates: Vec<(u32, String)> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        let (count_str, name) = match trimmed.split_once(' ') {
            Some(pair) => pair,
            None => continue,
        };
        if name.is_empty() || name == LEGACY_CTRL_SESSION_NAME {
            continue;
        }
        let count: u32 = count_str.parse().unwrap_or(0);
        candidates.push((count, name.to_string()));
    }
    // Highest attached count first — that's where the user's terminal lives.
    // Stable sort keeps tmux's original ordering as a tiebreaker.
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, n)| n)
}

/// Best-effort removal of the `_agentoast_ctrl` session left over from the
/// previous implementation. Failure (session missing, no server) is the
/// normal case, so it is intentionally silent.
fn cleanup_legacy_ctrl_session(tmux: &Path) {
    let _ = Command::new(tmux)
        .env_remove("TMPDIR")
        .args(["kill-session", "-t", LEGACY_CTRL_SESSION_NAME])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn is_topology_event(kind: &str) -> bool {
    matches!(
        kind,
        // Structural changes
        "window-add"
            | "window-close"
            | "window-renamed"
            | "unlinked-window-add"
            | "unlinked-window-close"
            | "unlinked-window-renamed"
            | "session-changed"
            | "sessions-changed"
            | "session-renamed"
            // Active-pane / focus changes — needed so the UI's "jump cursor to
            // currently-focused pane on panel open" reflects the latest tmux
            // state. is_active = pane_active && window_active && session_attached
            // (sessions/mod.rs), so we listen to all three transitions.
            | "window-pane-changed"
            | "session-window-changed"
            | "client-session-changed"
            | "client-detached"
    )
}
