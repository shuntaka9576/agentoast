# Changelog

## [v0.32.1](https://github.com/shuntaka9576/agentoast/compare/v0.32.0...v0.32.1) - 2026-04-18
- fix(panel): simplify T key help description by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/182
- fix(panel): auto-expand manually-collapsed active pane group by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/185
- ci(tagpr): wait for commit-PR association before running tagpr by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/188

## [v0.32.0](https://github.com/shuntaka9576/agentoast/compare/v0.31.1...v0.32.0) - 2026-04-17
- feat(sessions): detect local agent count and auto mode by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/180
- fix(deps): update rust crate clap to 4.6.1 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/175

## [v0.31.1](https://github.com/shuntaka9576/agentoast/compare/v0.31.0...v0.31.1) - 2026-04-17
- fix(panel): jump cursor to active tmux pane on Shift+T by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/178

## [v0.31.0](https://github.com/shuntaka9576/agentoast/compare/v0.30.0...v0.31.0) - 2026-04-17
- chore(deps): update actions/create-github-app-token action to v3.1.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/170
- chore(deps): update actions/create-github-app-token action to v3.1.1 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/171
- chore(deps): update songmu/tagpr action to v1.18.2 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/172
- perf(sessions): tmux control mode + event-driven refresh by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/176
- feat(panes): add toggle to show non-agent tmux panes by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/177
- chore(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/174

## [v0.30.0](https://github.com/shuntaka9576/agentoast/compare/v0.29.2...v0.30.0) - 2026-04-11
- chore(deps): update dependency @types/node to ^25.6.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/165
- style(panel): add inset ring to selected item for cursor visibility by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/167

## [v0.29.2](https://github.com/shuntaka9576/agentoast/compare/v0.29.1...v0.29.2) - 2026-04-11
- fix(deps): update frontend to ^19.2.5 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/160
- fix(deps): update rust crate toml_edit to 0.25.11 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/161
- fix(deps): update dependency lucide-react to ^1.8.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/163
- perf(panel): parallelize get_sessions and eliminate panel open/close lag by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/164

## [v0.29.1](https://github.com/shuntaka9576/agentoast/compare/v0.29.0...v0.29.1) - 2026-04-07
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/150
- fix(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/151
- chore(deps): update songmu/tagpr action to v1.18.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/154
- chore: add gh-infra config for repository settings management by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/156
- chore(deps): update songmu/tagpr action to v1.18.1 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/155
- chore(deps): update dependency cspell to v10 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/158
- fix(panel): eliminate flicker when showing panel via shortcut or tray click by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/159
- chore(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/157

## [v0.29.0](https://github.com/shuntaka9576/agentoast/compare/v0.28.5...v0.29.0) - 2026-04-06
- chore(deps): update dependency vite-plus to ^0.1.15 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/147
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/148
- feat: unify CLI and app into a single binary by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/152

## [v0.28.5](https://github.com/shuntaka9576/agentoast/compare/v0.28.4...v0.28.5) - 2026-03-29
- fix: position panel on active monitor in multi-monitor setups by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/145

## [v0.28.4](https://github.com/shuntaka9576/agentoast/compare/v0.28.3...v0.28.4) - 2026-03-28
- fix: prevent crash when opening panel via shortcut in fullscreen mode by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/143

## [v0.28.3](https://github.com/shuntaka9576/agentoast/compare/v0.28.2...v0.28.3) - 2026-03-28
- feat: add --version/-v flag to CLI with git revision by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/141

## [v0.28.2](https://github.com/shuntaka9576/agentoast/compare/v0.28.1...v0.28.2) - 2026-03-28
- fix(deps): update dependency lucide-react to ^1.7.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/135
- feat: add [system] config for manual tmux/git binary path override by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/140

## [v0.28.1](https://github.com/shuntaka9576/agentoast/compare/v0.28.0...v0.28.1) - 2026-03-27
- fix: support Nix-managed tmux/git in find_tmux/find_git by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/137

## [v0.28.0](https://github.com/shuntaka9576/agentoast/compare/v0.27.0...v0.28.0) - 2026-03-27
- feat: migrate from oxlint to Vite+ (vp) by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/131
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/133
- fix(deps): update frontend (major) by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/134
- fix(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/129
- feat: add GitHub Copilot CLI support by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/136

## [v0.27.0](https://github.com/shuntaka9576/agentoast/compare/v0.26.0...v0.27.0) - 2026-03-20

- chore(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/123
- feat: auto-cleanup notifications when agent resumes running by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/128
- fix(deps): update frontend to ^4.2.2 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/127
- fix(deps): update rust-workspace - autoclosed by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/126

## [v0.26.0](https://github.com/shuntaka9576/agentoast/compare/v0.25.1...v0.26.0) - 2026-03-17

- chore(deps): update dependency lefthook to ^2.1.3 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/111
- fix(deps): update rust crate toml to 1.0.6 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/110
- chore(deps): update dependency oxlint to ^1.52.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/113
- chore(deps): update rust crate tempfile to 3.27.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/114
- chore(deps): update dependency oxlint to ^1.53.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/115
- fix(deps): update rust crate clap to 4.6.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/118
- chore(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/116
- chore(deps): update frontend (major) by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/117
- chore(deps): update actions/create-github-app-token action to v3 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/121
- chore(deps): update dependency @vitejs/plugin-react to ^6.0.1 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/119
- chore(deps): update github-actions by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/120
- feat: detect and display Claude Code background bash tasks by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/124
- fix(deps): update rust crate rusqlite to 0.39.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/122

## [v0.25.1](https://github.com/shuntaka9576/agentoast/compare/v0.25.0...v0.25.1) - 2026-03-06

- fix(deps): update dependency lucide-react to ^0.576.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/102
- chore(deps): update dependency oxlint to ^1.51.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/104
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/106
- chore(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/105
- chore(deps): update oven-sh/setup-bun action to v2.1.3 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/108
- fix(deps): update dependency lucide-react to ^0.577.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/107
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/109

## [v0.25.0](https://github.com/shuntaka9576/agentoast/compare/v0.24.0...v0.25.0) - 2026-03-03

- feat: add TeammateIdle and TaskCompleted hook events for Agent Teams by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/99
- feat: add delayed auto-expand for pane card body text by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/101

## [v0.24.0](https://github.com/shuntaka9576/agentoast/compare/v0.23.0...v0.24.0) - 2026-03-03

- feat: add last_assistant_message body to Claude Code Stop notifications by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/95
- chore(deps): update songmu/tagpr action to v1.17.1 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/94
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/81
- fix(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/80
- chore: add postVersionCommand to auto-update Cargo.lock on release by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/97
- chore: pin linker to Apple clang for macOS builds by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/98

## [v0.23.0](https://github.com/shuntaka9576/agentoast/compare/v0.22.2...v0.23.0) - 2026-03-01

- feat: add Agent Teams subgroup display in Sessions tab by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/93

## [v0.22.2](https://github.com/shuntaka9576/agentoast/compare/v0.22.1...v0.22.2) - 2026-02-27

- refactor: split CLI hook handlers into per-agent modules by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/85
- refactor: split sessions.rs into per-agent modules by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/87
- refactor: extract emit_after_delete helper in lib.rs by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/88
- refactor: introduce NotificationInput struct to reduce insert_notification args by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/89
- refactor: introduce DismissButtonParams struct to reduce make_dismiss_button args by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/90
- fix: detect .opencode binary for mise/npm installed OpenCode by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/91

## [v0.22.1](https://github.com/shuntaka9576/agentoast/compare/v0.22.0...v0.22.1) - 2026-02-26

- chore: remove unused dependencies (env_logger, objc2-web-kit) by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/83

## [v0.22.0](https://github.com/shuntaka9576/agentoast/compare/v0.21.1...v0.22.0) - 2026-02-26

- feat: add auto-update UI to panel header by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/57

## [v0.21.1](https://github.com/shuntaka9576/agentoast/compare/v0.21.0...v0.21.1) - 2026-02-24

- chore(ci): pin GitHub Actions to commit SHAs for supply chain security by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/74
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/76
- chore(deps): update songmu/tagpr action to v1.17.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/77
- fix(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/78

## [v0.21.1](https://github.com/shuntaka9576/agentoast/compare/v0.21.0...v0.21.1) - 2026-02-24

- chore(ci): pin GitHub Actions to commit SHAs for supply chain security by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/74
- fix(deps): update rust-workspace by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/76
- chore(deps): update songmu/tagpr action to v1.17.0 by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/77
- fix(deps): update frontend by @renovate[bot] in https://github.com/shuntaka9576/agentoast/pull/78

## [v0.21.0](https://github.com/shuntaka9576/agentoast/compare/v0.20.0...v0.21.0) - 2026-02-23

- docs: simplify Installation section into a single block by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/73

## [v0.20.0](https://github.com/shuntaka9576/agentoast/compare/v0.19.0...v0.20.0) - 2026-02-23

- feat: re-add auto-update support with Apple signing and tauri-action CI by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/60
- fix: regenerate updater signing keypair by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/70

## [v0.19.0](https://github.com/shuntaka9576/agentoast/compare/v0.18.1...v0.19.0) - 2026-02-23

- fix: unify waiting_reason 'ask' and 'approve' into 'respond' by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/64
- feat: detect Codex question dialog and plan approval as Waiting by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/66
- fix: emit notifications:refresh on notification deletion to update agent status dots by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/67
- refactor: rename Claude Code detection signals to match Codex naming by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/68

## [v0.18.1](https://github.com/shuntaka9576/agentoast/compare/v0.18.0...v0.18.1) - 2026-02-22

- fix: rename config section agents.claude to agents.claude_code by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/61

## [v0.18.0](https://github.com/shuntaka9576/agentoast/compare/v0.17.0...v0.18.0) - 2026-02-22

- feat: add auto-update support with Apple signing and tauri-action CI by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/51
- Release for v0.17.1 by @github-actions[bot] in https://github.com/shuntaka9576/agentoast/pull/52
- feat: add OpenCode agent status detection by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/53
- fix: sort panes within group by agent status priority by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/54
- feat: add `agentoast hook opencode` CLI subcommand by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/56
- feat: rename config sections for clarity by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/58
- revert: remove auto-update support due to Apple notarization stalling by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/59

## [v0.17.0](https://github.com/shuntaka9576/agentoast/compare/v0.16.0...v0.17.0) - 2026-02-20

- feat: add built-in `agentoast hook codex` CLI subcommand by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/49
- fix: change agent status sort priority to waiting > running > idle by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/50

## [v0.16.0](https://github.com/shuntaka9576/agentoast/compare/v0.15.0...v0.16.0) - 2026-02-20

- fix: add missing spinner char (U+2733) to SPINNER_CHARS by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/44
- fix: add missing spinner char (U+2733) to SPINNER_CHARS by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/46
- feat: detect plan approval dialog as Waiting status with reason labels by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/47

## [v0.15.0](https://github.com/shuntaka9576/agentoast/compare/v0.14.2...v0.15.0) - 2026-02-20

- feat: add Codex-specific agent status detection by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/42

## [v0.14.2](https://github.com/shuntaka9576/agentoast/compare/v0.14.1...v0.14.2) - 2026-02-19

- fix: change filter_notified_only default to false by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/40

## [v0.14.1](https://github.com/shuntaka9576/agentoast/compare/v0.14.0...v0.14.1) - 2026-02-19

- fix: resolve toast layout overlap in release builds by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/37

## [v0.14.0](https://github.com/shuntaka9576/agentoast/compare/v0.13.0...v0.14.0) - 2026-02-19

- feat: add agent status detection with Running/Idle/Waiting states by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/34
- fix: use character wrapping for toast body text by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/36

## [v0.13.0](https://github.com/shuntaka9576/agentoast/compare/v0.12.1...v0.13.0) - 2026-02-19

- feat: replace WebView toast with native NSPanel implementation by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/31
- feat: replace agent type text label with tooltip on green dot by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/32

## [v0.12.1](https://github.com/shuntaka9576/agentoast/compare/v0.12.0...v0.12.1) - 2026-02-18

## [v0.12.0](https://github.com/shuntaka9576/agentoast/compare/v0.11.0...v0.12.0) - 2026-02-18

- feat: replace text labels with SVG icons for pane and branch metadata by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/28

## [v0.11.0](https://github.com/shuntaka9576/agentoast/compare/v0.10.0...v0.11.0) - 2026-02-18

- feat: unified tmux session view with hook integration and keyboard navigation by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/27

## [v0.10.0](https://github.com/shuntaka9576/agentoast/compare/v0.9.0...v0.10.0) - 2026-02-17

- feat: suppress notifications when originating tmux pane is active by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/23
- fix: explicitly hide panel when activating a notification by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/25

## [v0.9.0](https://github.com/shuntaka9576/agentoast/compare/v0.8.0...v0.9.0) - 2026-02-16

- feat: add keybind help overlay toggled by ? key by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/19
- fix: remove notification count badge from panel header by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/21
- feat: add d/D keyboard shortcuts for deleting notifications in panel by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/22

## [v0.8.0](https://github.com/shuntaka9576/agentoast/compare/v0.7.0...v0.8.0) - 2026-02-16

- feat: add global shortcut and keyboard navigation for notification panel by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/18

## [v0.7.0](https://github.com/shuntaka9576/agentoast/compare/v0.6.0...v0.7.0) - 2026-02-16

- feat: add dismiss buttons to toast notifications by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/16

## [v0.6.0](https://github.com/shuntaka9576/agentoast/compare/v0.5.1...v0.6.0) - 2026-02-16

- chore: rename Homebrew Formula to agentoast-cli by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/14

## [v0.5.1](https://github.com/shuntaka9576/agentoast/compare/v0.5.0...v0.5.1) - 2026-02-15

- fix: add explicit version field to Homebrew Formula template by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/11

## [v0.5.0](https://github.com/shuntaka9576/agentoast/compare/v0.4.0...v0.5.0) - 2026-02-15

- feat: use \_\_CFBundleIdentifier for terminal focus instead of hardcoded list by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/9

## [v0.4.0](https://github.com/shuntaka9576/agentoast/compare/v0.3.0...v0.4.0) - 2026-02-15

- feat: change toast notification queue from FIFO to LIFO by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/8

## [v0.3.0](https://github.com/shuntaka9576/agentoast/compare/v0.2.0...v0.3.0) - 2026-02-15

- fix: preserve notification group insertion order by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/4
- feat: add toast notification queue, configurable duration, and new notification highlight by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/6

## [v0.2.0](https://github.com/shuntaka9576/agentoast/compare/v0.1.0...v0.2.0) - 2026-02-15

- feat: add mute notifications feature by @shuntaka9576 in https://github.com/shuntaka9576/agentoast/pull/3

## [v0.1.0](https://github.com/shuntaka9576/agentoast/commits/v0.1.0) - 2026-02-14
