<div align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="agentoast icon" />
  <h1>agentoast</h1>
  <p>
    <img alt="macOS" src="https://img.shields.io/badge/macOS-000?logo=apple&logoColor=white" />
  </p>
</div>

A macOS menu bar app for tmux users. Get a toast notification whenever an AI coding agent (Claude Code, Codex, opencode) finishes or needs your input — click it to jump right back to the tmux pane it came from.

You kick off a long-running agent task, switch over to a browser or another window, and completely miss the moment it wraps up or asks for permission. agentoast makes sure you never miss it.

When an agent completes or needs attention, a toast pops up at the top-right corner — click it to jump right back to the tmux pane.

![toast](docs/assets/toast.gif)

<img src="docs/assets/toast.png" width="400" alt="toast" />

With `--focus`, the terminal is brought to the foreground automatically — no click needed. See [`Send Notification`](#send-notification) for details.

All notifications are grouped by repository in the menu bar. Clicking one takes you straight to its tmux pane.

![menubar](docs/assets/menubar.gif)

<img src="docs/assets/menubar.png" width="400" alt="menubar" />

## Installation

### CLI

```bash
brew install shuntaka9576/tap/agentoast-cli
```

### App

Download the DMG from [Releases](https://github.com/shuntaka9576/agentoast/releases) or install via Homebrew Cask.

```bash
brew install --cask shuntaka9576/tap/agentoast
xattr -cr /Applications/Agentoast.app
```

> The app is not yet signed with an Apple Developer ID. macOS Gatekeeper may flag it as "damaged" — the `xattr` command above removes the quarantine attribute to fix this. Apple Developer signing is in progress and will remove this requirement in a future release.

### Uninstall

```bash
brew uninstall --cask shuntaka9576/tap/agentoast
brew uninstall shuntaka9576/tap/agentoast-cli
```

## Usage

Hook scripts for Codex require [Deno](https://deno.land/). Grab the script from [`examples/notify/`](examples/notify/) and `chmod +x` it. Claude Code uses the built-in CLI subcommand instead. opencode has its own plugin system.

### Claude Code

`~/.claude/settings.json`

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "agentoast hook claude"
          }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "agentoast hook claude"
          }
        ]
      }
    ]
  }
}
```

No Deno dependency required. The CLI reads hook data from stdin and writes directly to the notification database. See [`examples/notify/claude.ts`](examples/notify/claude.ts) for a Deno-based alternative.

### Codex

Script [`examples/notify/codex.ts`](examples/notify/codex.ts)

`~/.codex/config.toml`

```toml
notify = [
  "/path/to/notify/codex.ts",
]
```

Update the path to match where you saved the script.

### opencode

Plugin [`examples/notify/opencode.ts`](examples/notify/opencode.ts)

opencode uses a [plugin system](https://opencode.ai/docs/plugins) rather than hook scripts. Drop the plugin file into `~/.config/opencode/plugins/` and it gets picked up automatically.

```bash
mkdir -p ~/.config/opencode/plugins
cp examples/notify/opencode.ts ~/.config/opencode/plugins/
```

Supported events

| Event | Notification |
|---|---|
| `session.status` (idle) | Stop (red) |
| `session.error` | Error (red) |
| `permission.asked` | Permission (blue) |

### Send Notification

```bash
agentoast send \
  --badge "Stop" \
  --body "Task Completed" \
  --badge-color green \
  --icon claude-code \
  --repo my-repo \
  --tmux-pane %0 \
  --meta branch=main
```

| Option | Short | Required | Default | Description |
|---|---|---|---|---|
| `--badge` | `-B` | No | `""` | Badge text displayed on notification card |
| `--body` | `-b` | No | `""` | Notification body text |
| `--badge-color` | `-c` | No | `gray` | Badge color (`green`, `blue`, `red`, `gray`) |
| `--icon` | `-i` | No | `agentoast` | Icon preset (`agentoast` / `claude-code` / `codex` / `opencode`) |
| `--repo` | `-r` | No | auto | Repository name for grouping notifications. Auto-detected from git remote or directory name if omitted |
| `--tmux-pane` | `-t` | No | `""` | tmux pane ID. Used for focus-on-click and batch dismiss (e.g. `%0`) |
| `--bundle-id` | — | No | auto | Terminal bundle ID for focus-on-click (e.g. `com.github.wez.wezterm`). Auto-detected from `__CFBundleIdentifier` env var if not specified |
| `--focus` | `-f` | No | `false` | Focus terminal automatically when notification is sent. A toast is shown with "Focused: no history" label, but the notification does not appear in the notification history |
| `--meta` | `-m` | No | - | Display metadata as key=value pairs (can be specified multiple times). Shown on notification cards |

Clicking a notification dismisses it and brings you back to the terminal. With `--tmux-pane`, all notifications sharing the same `--tmux-pane` are dismissed at once. Sending a new notification with the same `--tmux-pane` replaces the previous one, so only the latest notification per pane is kept.

When a terminal is focused and the notification's originating tmux pane is the active pane, notifications are automatically suppressed — since you're already looking at it.

For a quick test, you can fire off notifications straight from the CLI.

Claude Code

```bash
agentoast send \
  --badge "Stop" \
  --badge-color green \
  --icon claude-code \
  --repo your-repo \
  --tmux-pane %0 \
  --meta branch=your-branch
```

Codex (OpenAI)

```bash
agentoast send \
  --badge "Notification" \
  --badge-color blue \
  --icon codex \
  --repo your-repo \
  --meta branch=your-branch
```

opencode

```bash
agentoast send \
  --badge "Stop" \
  --badge-color red \
  --icon opencode \
  --repo your-repo \
  --tmux-pane %0 \
  --meta branch=your-branch
```

### Config

Opens `~/.config/agentoast/config.toml` in your editor, creating a default one if it doesn't exist yet.

```bash
agentoast config
```

Editor resolution priority is `config.toml` `editor` field → `$EDITOR` → `vim`

```toml
# agentoast configuration

# Editor to open when running `agentoast config`
# Falls back to $EDITOR environment variable, then vim
# editor = "vim"

# Toast popup notification
[toast]
# Display duration in milliseconds (default: 4000)
# duration_ms = 4000

# Keep toast visible until clicked (default: false)
# persistent = false

# Menu bar notification panel
[panel]
# Maximum number of notifications per group (default: 3, 0 = unlimited)
# group_limit = 3

# Mute all notifications (default: false)
# muted = false

# Show only groups with notifications (default: true)
# filter_notified_only = true

# Global keyboard shortcut
[shortcut]
# Shortcut to toggle the notification panel (default: ctrl+alt+n)
# Format: modifier+key (modifiers: ctrl, shift, alt/option, super/cmd)
# Set to "" to disable
# toggle_panel = "ctrl+alt+n"

# Claude Code hook settings
[hook.claude]
# Events that trigger notifications (default: all)
# Available: Stop, permission_prompt, idle_prompt, auth_success, elicitation_dialog
# events = ["Stop", "permission_prompt", "idle_prompt", "auth_success", "elicitation_dialog"]

# Events that auto-focus the terminal (default: none)
# These events set force_focus=true, causing silent terminal focus without toast (when not muted)
# focus_events = []
```

### Keyboard Shortcuts

Panel shortcuts (press `?` in the panel to see this list).

| Key | Action |
|---|---|
| `j` / `k` | Next / Previous |
| `Enter` | Open / Fold |
| `d` | Delete notif |
| `D` | Delete all notifs |
| `C` / `E` | Collapse all / Expand all |
| `F` | Filter notified |
| `Tab` / `Shift+Tab` | Jump to next / prev notified pane |
| `Esc` | Close |
| `?` | Help |

The global shortcut to toggle the panel is `Ctrl+Alt+N` (configurable in `config.toml`).

### Tips

Set up a shell alias for command completion notifications. With `--tmux-pane`, clicking the notification jumps back to the pane.

```bash
alias an='agentoast send --badge Done --badge-color green --tmux-pane "$TMUX_PANE"'
```

```bash
sleep 10; an -b "body"
```

