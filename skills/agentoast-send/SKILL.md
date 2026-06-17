---
name: agentoast-send
description: Send a message TO an AI coding agent running in another tmux pane via the `agentoast send-keys` CLI.
when_to_use: |
  Trigger only when BOTH are true:
  - The prompt contains the agentoast notification text "Please take a look at tmux pane %NN.".
  - The user adds an explicit instruction to send / reply / delegate something to that agent on top of the pasted notification.
license: MIT
compatibility: Requires agentoast CLI and tmux
allowed-tools: Bash(agentoast:*) Read
metadata:
  author: shuntaka9576
  version: "0.49.4"
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
- **`no-agent` (exit ≠ 0)** → Nothing to send. Answer the user's original prompt normally; if you need to know what's in the pane, run `tmux capture-pane -t %NN -p`.

The single source of truth for "what counts as an agent" is `AGENT_PROCESSES` in `crates/agentoast-shared/src/agent_detect.rs`. Both `detect-agent` and `send-keys` share it, so adding a new agent only requires editing that one list — this skill needs no update.

## Step 3: Send (and reply)

Open and follow [references/operate.md](references/operate.md). It covers the `agentoast send-keys` invocation, the reply pattern for incoming messages, and when the right move is to _not_ send anything (handoffs, acknowledgments, status pings). Reading it before Step 2 passes is wasted context.
