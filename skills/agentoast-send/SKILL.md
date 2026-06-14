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
---

# agentoast-send: triage before you send

This skill messages an AI coding agent running in another tmux pane via the `agentoast send-keys` CLI. The transport is a tmux pane id (e.g. `%72`).

Trigger conditions in the frontmatter `when_to_use` already filter most non-delegation prompts out. The job of this body is a single thing: **before you reach for the send command, prove there is actually an agent at the target pane.** A wrong target is a more common mistake than a wrong message — once you confirm the target, the actual send/reply flow is straightforward and lives in [references/operate.md](references/operate.md). Don't read that file until Step 2 below passes.

## Step 1: Resolve the target pane id (`%NN`)

- The user usually provides it — often by pasting agentoast's clipboard string `Please take a look at tmux pane %72.` Take the `%72` from there.
- A reply sentinel `(reply: agentoast send-keys --pane %45 "<reply>")` names the target directly.
- If no pane id is anywhere in scope, ask the user which pane they mean. Do not guess.

## Step 2: Verify an agent is actually at `%NN`

Most panes are shells, editors, build runs, or REPLs. Sending into those just dumps the message into a shell prompt and is annoying. Always check before you send by listing every process attached to the pane's tty and matching against known agent binary names:

```
ps -t "$(tmux display-message -t %NN -p '#{pane_tty}' | sed 's|^/dev/||')" -o command= 2>/dev/null | grep -qiE '(^|/)(claude|codex|cursor-agent|aider|cody|continue)( |$)' && echo agent || echo no-agent
```

- **`agent`** → an AI coding agent (Claude Code, Codex, Cursor, Aider, etc.) is genuinely running in `%NN`. Proceed to Step 3.
- **`no-agent`** → STOP. Tell the user that `%NN` is not running an agent (you can show them the process list above to clarify) and handle the rest of their request as a normal task: if they wanted to see what's in the pane, run `tmux capture-pane -t %NN -p` and discuss it; if they just mentioned the pane in passing, carry on with the original conversation. Do NOT call `agentoast send-keys`.

Why this check rather than `pane_current_command` or `tmux capture-pane`:

- `pane_current_command` reports the foreground binary, which is `node` for Codex, a version string like `2.1.177` for Claude Code, `cargo-make` for a build run — too coarse to tell agents apart from build tools.
- `tmux capture-pane` is unreliable for agents whose TUI is cursor-positioned (Codex via ratatui, others): their scrollback can be empty even while the UI is on screen.
- The process list attached to the tty always contains the actual agent CLI invocation (`claude --chrome ...`, `.../bin/codex`, etc.), so grepping it is robust regardless of how the UI is drawn.

If Step 2 says STOP, the skill's job is done. Hand control back to the main task.

## Step 3: Send (and reply)

Open and follow [references/operate.md](references/operate.md). That file contains the actual `agentoast send-keys` invocation, the reply-via-heredoc pattern, and a short Notes section on flags. Reading it before Step 2 passes is wasted context.
