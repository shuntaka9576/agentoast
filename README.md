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

```bash
brew install shuntaka9576/tap/agentoast          # CLI
brew install --cask shuntaka9576/tap/agentoast   # macOS menu bar app

# Uninstall
# brew untap shuntaka9576/tap --force

# The app is not signed with an Apple Developer ID, so macOS Gatekeeper
# may flag it as "damaged." Remove the quarantine attribute to fix this
xattr -cr /Applications/Agentoast.app
```

## Usage

Hook scripts for Claude Code and Codex require [Deno](https://deno.land/). Grab the right script from [`examples/notify/`](examples/notify/) and `chmod +x` it. opencode has its own plugin system instead.

### Claude Code

Script [`examples/notify/claude.ts`](examples/notify/claude.ts)

`~/.claude/settings.json`

```json
{
  "hooks": {
    "Stop": [
      {
        "type": "command",
        "command": "/path/to/notify/claude.ts"
      }
    ],
    "Notification": [
      {
        "type": "command",
        "command": "/path/to/notify/claude.ts"
      }
    ]
  }
}
```

Update the path to match where you saved the script.

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
  --title "Stop" \
  --body "Task Completed" \
  --color green \
  --icon claude-code \
  --group my-repo \
  --tmux-pane %0 \
  --meta branch=main
```

| Option | Required | Default | Description |
|---|---|---|---|
| `--title` | No | `""` | Notification title (displayed as badge) |
| `--body` | No | `""` | Notification body text |
| `--color` | No | `gray` | Badge color (`green`, `blue`, `red`, `gray`) |
| `--icon` | No | `agentoast` | Icon preset (`agentoast` / `claude-code` / `codex` / `opencode`) |
| `--group` | No | `""` | Group name (e.g. repository name, project name) |
| `--tmux-pane` | No | `""` | tmux pane ID. Used for focus-on-click and batch dismiss (e.g. `%0`) |
| `--focus` | No | `false` | Focus terminal automatically when notification is sent. A toast is shown with "Focused: no history" label, but the notification does not appear in the notification history |
| `--meta` | No | - | Display metadata as key=value pairs (can be specified multiple times). Shown on notification cards |

Clicking a notification dismisses it and brings you back to the terminal. With `--tmux-pane`, all notifications sharing the same `--group` + `--tmux-pane` are dismissed at once.

For a quick test, you can fire off notifications straight from the CLI.

Claude Code

```bash
agentoast send \
  --title "Stop" \
  --color green \
  --icon claude-code \
  --group your-repo \
  --tmux-pane %0 \
  --meta branch=your-branch
```

Codex (OpenAI)

```bash
agentoast send \
  --title "Notification" \
  --color blue \
  --icon codex \
  --group your-repo \
  --meta branch=your-branch
```

opencode

```bash
agentoast send \
  --title "Stop" \
  --color red \
  --icon opencode \
  --group your-repo \
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

[display]
# Maximum number of notifications per group in the main panel (default: 3, 0 = unlimited)
# group_limit = 3
```

