//! tmux control mode (`tmux -C`) client.
//!
//! Maintains a long-lived tmux client process over a single pipe, avoiding per-call
//! fork+exec overhead. Commands are serialized through a worker thread: each caller
//! sends a command string, gets back the response lines collected between tmux's
//! `%begin` / `%end` markers. Asynchronous notifications (window-add etc.) flow into
//! a separate listener channel for event-driven cache invalidation.

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::terminal::find_tmux;

const HANDSHAKE_SENTINEL: &str = "__agentoast_ctrl_ready__";
const RECONNECT_BACKOFF: Duration = Duration::from_secs(3);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const CTRL_SESSION_NAME: &str = "_agentoast_ctrl";

#[derive(Clone)]
pub struct TmuxCtrl {
    tx: Sender<Request>,
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
        thread::spawn(move || loop {
            match run_session(&rx, &self_tx, notifier.as_ref()) {
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
        Self { tx }
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
}

fn run_session(
    rx: &Receiver<Request>,
    self_tx: &Sender<Request>,
    notifier: Option<&Sender<TopologyChanged>>,
) -> Result<(), String> {
    let tmux = find_tmux().ok_or_else(|| "tmux not found".to_string())?;

    let mut child = Command::new(&tmux)
        .env_remove("TMPDIR")
        .args(["-C", "new-session", "-A", "-s", CTRL_SESSION_NAME])
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
