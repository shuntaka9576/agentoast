# agentoast-send: send and reply flow

You should be reading this file only after SKILL.md Step 2 confirmed an AI coding agent is actually running at the target pane. If you got here without verifying, go back and verify first.

## Sending a message

Run:

```
agentoast send-keys --pane %72 "your message"
```

The text is typed into that agent's prompt and submitted. If you are running inside tmux (`$TMUX_PANE` set) or you pass `--from %NN`, your pane id is appended as a reply address so the receiver sees `(reply: agentoast send-keys --pane <you> "<reply>")` — you do not add that hint yourself.

Step 2 (`agentoast detect-agent`) already confirmed an agent is at the target, so `send-keys` should not refuse. In the rare case it does (a one-off race in detection), report the refusal to the user and stop. Do not retry.

After a successful send, tell the user it was sent. The reply arrives later as a new prompt in YOUR pane — there is nothing to poll or watch.

Write the message yourself from the conversation so far. You hold context the other agent lacks, so phrase a self-contained request (summarize the task or question) rather than a bare "see above" — the other pane can't see your screen.

## Replying to an incoming message

If your prompt contains a line like `(reply: agentoast send-keys --pane %45 "<reply>")`, that is a message from another agent. Do the work it asks for, then send your answer back. Always pass the reply via a single-quoted heredoc so `"`, backticks, `$(...)`, and newlines in the body do not break shell quoting or get expanded:

```
agentoast send-keys --pane %45 "$(cat <<'EOF'
your reply, including "quotes", $vars, and `backticks`
EOF
)"
```

## When NOT to reply

`send-keys` is a working channel, not a chat. Every message you send another agent costs them a turn of context. Don't spend it on social glue — bare acknowledgments ("got it", "thanks", "will share progress"), echoing the plan back to confirm understanding, or a standalone "done" ping with no result attached. If you finished, the result _is_ the message: send the diff, the path, or the answer in the same call. If the artifact is already visible to them (commit pushed, file saved), send nothing and let it speak.

A reply is the right call only when (a) you have a substantive answer to a question they asked, (b) you hit a blocker only they can resolve, or (c) you finished and the result isn't otherwise visible. Otherwise, finish the work and stay silent.

## Handoffs: act, don't ack

When the incoming message hands the task to you — "take it from here", "you own this now", "hand it over to you" — that is a transfer of responsibility, not a question. The sender has stepped out. Don't ack before starting, and don't send mid-task status pings; the handoff already said they're not waiting. Report back only when you finish (with the result in the same message) or when you genuinely cannot proceed without their input.

If you finish a handed-off task, the audience for the result is usually the human in _your_ pane, not the original agent. Surface the result in your normal output — don't ping the previous agent over `send-keys` just to close the loop.

## Don't scrape the screen — ask the agent

To learn what another agent is doing or what its task is, do NOT run `tmux capture-pane`. In a conversation TUI the captured text is clipped to the pane width and truncated, and scrollback can't hold the whole conversation — you get a broken, partial view. Instead, ask the agent and let it answer in clean, authored prose:

```
agentoast send-keys --pane %72 "What task or blocker are you working on right now?"
```

The agent knows its own context and will summarize it far better than a screen grab.

## Notes

- Address = tmux pane id; ids stay stable for the life of the pane.
- `send-keys` refuses a pane with no detected AI coding agent (a plain shell), since the message would just be typed into the shell prompt — a sign you picked the wrong pane. If a real agent isn't being detected, that's an `AGENT_PROCESSES` gap to fix in Rust, not something this skill bypasses.
- If the target looks busy mid-generation, injected keystrokes can interleave — prefer sending when it is idle or waiting for input.
- `--raw` sends without the reply hint; `--no-enter` types without submitting.
