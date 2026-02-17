export type Icon = "agentoast" | "claude-code" | "codex" | "opencode";

export interface Notification {
  id: number;
  title: string;
  body: string;
  color: string;
  icon: Icon;
  groupName: string;
  metadata: Record<string, string>;
  tmuxPane: string;
  terminalBundleId: string;
  forceFocus: boolean;
  isRead: boolean;
  createdAt: string;
}

export interface NotificationGroup {
  groupName: string;
  notifications: Notification[];
  unreadCount: number;
}

export interface MuteState {
  globalMuted: boolean;
  mutedGroups: string[];
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
  orphanNotifications: Notification[];
}

export type FlatItem =
  | { type: "group-header"; groupKey: string }
  | { type: "pane-item"; groupKey: string; paneItem: PaneItem }
  | { type: "orphan-notification"; groupKey: string; notification: Notification };
