#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# demo-setup.sh — agentoast demo (2 sessions, multiple agents)
# =============================================================================
#
# Usage:
#   bash scripts/demo-setup.sh          # Create sessions and launch agents
#   bash scripts/demo-setup.sh clean    # Kill sessions

SESSION1="dotfiles"
SESSION2="shuntaka-dev"
REPO_DIR1="$HOME/repos/github.com/shuntaka9576/dotfiles"
REPO_DIR2="$HOME/repos/github.com/shuntaka9576/shuntaka-dev"
CLAUDE="claude --chrome --dangerously-skip-permissions"

# ─── Clean mode ──────────────────────────────────────────────────────────────
if [[ "${1:-}" == "clean" ]]; then
  echo "Cleaning up..."
  tmux kill-session -t "$SESSION1" 2>/dev/null && echo "Killed session: $SESSION1" || echo "Session not found: $SESSION1"
  tmux kill-session -t "$SESSION2" 2>/dev/null && echo "Killed session: $SESSION2" || echo "Session not found: $SESSION2"
  exit 0
fi

# ─── Helper: wait for pattern in capture-pane ────────────────────────────────
wait_for_prompt() {
  local pane=$1
  local pattern=$2
  local timeout=${3:-30}
  local elapsed=0
  while [[ $elapsed -lt $timeout ]]; do
    if tmux capture-pane -t "$pane" -p 2>/dev/null | grep -q "$pattern"; then
      echo "  ✓ $pane ready"
      return 0
    fi
    sleep 1
    ((elapsed++))
  done
  echo "  ✗ $pane timed out (${timeout}s)"
  return 1
}

# ─── Phase 1: Kill existing sessions ────────────────────────────────────────
echo "Phase 1: Kill existing sessions"
tmux kill-session -t "$SESSION1" 2>/dev/null || true
tmux kill-session -t "$SESSION2" 2>/dev/null || true

# ─── Phase 2: Create sessions with panes ────────────────────────────────────
echo "Phase 2: Create sessions"

# dotfiles: 3 panes (claude + codex + opencode)
tmux new-session -d -s "$SESSION1" -c "$REPO_DIR1"
tmux split-window -t "$SESSION1:0" -h -c "$REPO_DIR1"
tmux split-window -t "$SESSION1:0" -h -c "$REPO_DIR1"
tmux select-layout -t "$SESSION1:0" even-horizontal

# shuntaka-dev: 3 panes (claude + codex + opencode)
tmux new-session -d -s "$SESSION2" -c "$REPO_DIR2"
tmux split-window -t "$SESSION2:0" -h -c "$REPO_DIR2"
tmux split-window -t "$SESSION2:0" -h -c "$REPO_DIR2"
tmux select-layout -t "$SESSION2:0" even-horizontal

# ─── Phase 3: Launch agents ─────────────────────────────────────────────────
echo "Phase 3: Launch agents"

# dotfiles
tmux send-keys -t "$SESSION1:0.0" "$CLAUDE" C-m
tmux send-keys -t "$SESSION1:0.1" "codex" C-m
tmux send-keys -t "$SESSION1:0.2" "opencode --prompt \"Run sleep 1000 in the foreground\"" C-m

# shuntaka-dev
tmux send-keys -t "$SESSION2:0.0" "$CLAUDE" C-m
tmux send-keys -t "$SESSION2:0.1" "codex" C-m
tmux send-keys -t "$SESSION2:0.2" "opencode --prompt \"Add a brief description comment to the top of README.md\"" C-m

# ─── Phase 4: Wait for agent initialization ─────────────────────────────────
echo "Phase 4: Waiting for agents to initialize..."
set +e

# dotfiles
wait_for_prompt "$SESSION1:0.0" "❯" 30
wait_for_prompt "$SESSION1:0.1" "›" 30
wait_for_prompt "$SESSION1:0.2" "ctrl+t variants" 30

# shuntaka-dev
wait_for_prompt "$SESSION2:0.0" "❯" 30
wait_for_prompt "$SESSION2:0.1" "›" 30

set -e

# ─── Phase 5: Send prompts ──────────────────────────────────────────────────

# dotfiles: Claude → plan mode + task
echo "Phase 5: dotfiles - Claude plan mode"
tmux send-keys -t "$SESSION1:0.0" "/plan"
sleep 0.15
tmux send-keys -t "$SESSION1:0.0" Enter
sleep 2
tmux send-keys -t "$SESSION1:0.0" "Add a brief description comment to the top of README.md"
sleep 0.15
tmux send-keys -t "$SESSION1:0.0" Enter

# dotfiles: Codex → plan mode + task
echo "Phase 5: dotfiles - Codex plan mode"
tmux send-keys -t "$SESSION1:0.1" "/plan"
sleep 0.15
tmux send-keys -t "$SESSION1:0.1" Enter
sleep 2
tmux send-keys -t "$SESSION1:0.1" "Add a brief description comment to the top of README.md"
sleep 0.15
tmux send-keys -t "$SESSION1:0.1" Enter

# shuntaka-dev: Claude → sleep 1000
echo "Phase 5: shuntaka-dev - Claude sleep 1000"
tmux send-keys -t "$SESSION2:0.0" "Run sleep 1000 in the foreground"
sleep 0.15
tmux send-keys -t "$SESSION2:0.0" Enter

# shuntaka-dev: Codex → long analysis task
echo "Phase 5: shuntaka-dev - Codex analysis"
tmux send-keys -t "$SESSION2:0.1" "Read every source file in this repository one by one, then write a comprehensive architecture document covering all modules, functions, and their relationships"
sleep 0.15
tmux send-keys -t "$SESSION2:0.1" Enter

echo ""
echo "Done! All agents launched."
echo ""
echo "  $SESSION1:"
echo "    Pane 0: claude (plan → Waiting)"
echo "    Pane 1: codex (plan → Waiting)"
echo "    Pane 2: opencode (Running)"
echo ""
echo "  $SESSION2:"
echo "    Pane 0: claude (Running)"
echo "    Pane 1: codex (Running)"
echo "    Pane 2: opencode (Approve → Waiting)"
echo ""
echo "Attach with: tmux attach -t $SESSION1"
