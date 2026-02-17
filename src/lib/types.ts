export type Icon = "agentoast" | "claude-code" | "codex" | "opencode";

export interface Notification {
  id: number;
  badge: string;
  body: string;
  badgeColor: string;
  icon: Icon;
  metadata: Record<string, string>;
  repo: string;
  tmuxPane: string;
  terminalBundleId: string;
  forceFocus: boolean;
  isRead: boolean;
  createdAt: string;
}

export interface MuteState {
  globalMuted: boolean;
  mutedRepos: string[];
}

export interface TmuxPane {
  paneId: string;
  panePid: number;
  sessionName: string;
  windowName: string;
  currentPath: string;
  agentType: Icon | null;
  gitRepoRoot: string | null;
  gitBranch: string | null;
}

export interface TmuxPaneGroup {
  repoName: string;
  currentPath: string;
  gitBranch: string | null;
  panes: TmuxPane[];
}

export interface PaneItem {
  pane: TmuxPane;
  notification: Notification | null;
}

export interface UnifiedGroup {
  groupKey: string;
  repoName: string;
  gitBranch: string | null;
  paneItems: PaneItem[];
}

export type FlatItem =
  | { type: "group-header"; groupKey: string }
  | { type: "pane-item"; groupKey: string; paneItem: PaneItem };
