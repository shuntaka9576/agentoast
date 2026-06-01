---
name: agentoast-send
description: >-
  Send a request or question to another AI coding agent running in a different
  tmux pane, and reply to messages from other agents — via the `agentoast
  send-keys` CLI (address = a tmux pane id like %72; no team, login, or setup).
  Use this whenever the user wants to delegate to, ask, hand off to, or relay a
  message to an agent in another pane/window/session — e.g. "ask the agent in
  pane %72 to review this", "have the other session check X", or right after the
  user pastes "Please take a look at tmux pane %72." — and whenever an incoming
  prompt contains a "(reply: agentoast send-keys --pane %NN ...)" hint to answer.
  Prefer this over reading another pane's screen with `tmux capture-pane`, which
  truncates lines and cannot see the full conversation.
---

# agentoast-send: cross-pane agent messaging

Talk to AI coding agents running in OTHER tmux panes by injecting a message into their prompt, and reply to messages they send you. The transport is the `agentoast send-keys` CLI and the address is just a tmux pane id (e.g. `%72`). There is no team, login, or registration: if you know the pane, you can message it.

## Sending a message

1. Resolve the target pane id (`%NN`).
   - The user usually provides it — often by pasting agentoast's clipboard string `Please take a look at tmux pane %72.` Take the `%72` from there.
   - If no pane id is present, ask the user which pane to send to.
2. Run:
   ```
   agentoast send-keys --pane %72 "your message"
   ```
   The text is typed straight into that agent's prompt and submitted. Your own pane is read from `$TMUX_PANE` and appended as a reply address, so the receiver sees a trailing `(reply: agentoast send-keys --pane <you> "<reply>")`. You do not add that hint yourself.
3. Tell the user it was sent. The reply arrives later as a new prompt in YOUR pane — there is nothing to poll or watch.

Write the message yourself from the conversation so far. You hold context the other agent lacks, so phrase a self-contained request (summarize the task or question) rather than a bare "see above" — the other pane can't see your screen.

## Replying to an incoming message

If your prompt contains a line like `(reply: agentoast send-keys --pane %45 "<reply>")`, that is a message from another agent. Do the work it asks for, then run that exact command with your answer in place of `<reply>`.

## Don't scrape the screen — ask the agent

To learn what another agent is doing or what its task is, do NOT run `tmux capture-pane`. In a conversation TUI the captured text is clipped to the pane width and truncated, and scrollback can't hold the whole conversation — you get a broken, partial view. Instead, ask the agent and let it answer in clean, authored prose:

```
agentoast send-keys --pane %72 "What task or blocker are you working on right now?"
```

The agent knows its own context and will summarize it far better than a screen grab.

## Notes

- Address = tmux pane id; ids stay stable for the life of the pane.
- If the target looks busy mid-generation, injected keystrokes can interleave — prefer sending when it is idle or waiting for input.
- `--raw` sends without the reply hint; `--no-enter` types without submitting.
