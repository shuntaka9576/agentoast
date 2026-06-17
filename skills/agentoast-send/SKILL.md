---
name: agentoast-send
description: Delegate a task, question, or reply TO an AI coding agent running in another tmux pane via the `agentoast send-keys` CLI. Use only when the user's intent is to send a message TO another agent — not to read or inspect a pane.
when_to_use: |
  Trigger when ANY of:
  - The user asks to send / ask / delegate / forward / hand off / reply TO an agent in pane %NN (e.g. "ask the agent in %72 to review this", "send this to %39", "reply to %45", "have the other Codex in %37 check X").
  - The user pastes the agentoast notification clipboard string "Please take a look at tmux pane %NN." verbatim.
  - The incoming prompt contains a "(reply: agentoast send-keys --pane %NN ...)" sentinel from another agent.

  Do NOT trigger when:
  - The user only wants to inspect, read, check, or look at what is displayed in a pane ("what's in %23", "check pane %15", "show me %4", "look at the output in %9"). Handle those normally without messaging.
  - A pane id (%NN) is mentioned without any send/ask/delegate intent.
  - The intent is ambiguous — defer to the user.
license: MIT
compatibility: Requires agentoast CLI and tmux
allowed-tools: Bash(agentoast:*) Read
metadata:
  author: shuntaka9576
  version: "0.49.2"
---

# agentoast-send: triage before you send

This skill messages an AI coding agent running in another tmux pane via the `agentoast send-keys` CLI. The transport is a tmux pane id (e.g. `%72`).

Trigger conditions in the frontmatter `when_to_use` already filter most non-delegation prompts out. The job of this body is a single thing: **before you reach for the send command, prove there is actually an agent at the target pane.** A wrong target is a more common mistake than a wrong message — once you confirm the target, the actual send/reply flow is straightforward and lives in [references/operate.md](references/operate.md). Don't read that file until Step 2 below passes.

## Step 1: Resolve the target pane id (`%NN`)

- The user usually provides it — often by pasting agentoast's clipboard string `Please take a look at tmux pane %72.` Take the `%72` from there.
- A reply sentinel `(reply: agentoast send-keys --pane %45 "<reply>")` names the target directly.
- If no pane id is anywhere in scope, ask the user which pane they mean. Do not guess.

## Step 2: Verify an agent is actually at `%NN`

Most panes are shells, editors, build runs, or REPLs. Sending into those just dumps the message into a shell prompt and is annoying. Ask the CLI:

```
agentoast detect-agent --pane %NN
```

- **`agent` (exit 0)** → an AI coding agent is running in `%NN`. Proceed to Step 3.
- **`no-agent` (exit ≠ 0)** → STOP. The skill is done here. Do NOT open `references/operate.md`, do NOT call `agentoast send-keys`, do NOT run `tmux capture-pane` or any other tmux probe on behalf of this skill. Reply to the user with one short sentence — "`%NN` is not running an AI coding agent, so I won't send anything." — and then handle the rest of their request as a normal conversation (if they later ask to see what's in the pane, use plain `tmux` at that point, outside this skill).

The single source of truth for "what counts as an agent" is `AGENT_PROCESSES` in `crates/agentoast-shared/src/agent_detect.rs`. Both `detect-agent` and `send-keys` share it, so adding a new agent only requires editing that one list — this skill needs no update.

## Step 3: Send (and reply)

Open and follow [references/operate.md](references/operate.md). It covers the `agentoast send-keys` invocation, the reply pattern for incoming messages, and when the right move is to _not_ send anything (handoffs, acknowledgments, status pings). Reading it before Step 2 passes is wasted context.
