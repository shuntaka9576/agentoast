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
}

export interface TmuxPaneGroup {
  repoName: string;
  currentPath: string;
  panes: TmuxPane[];
}

export interface UnifiedGroup {
  groupName: string;
  activeSessions: TmuxPane[];
  notifications: Notification[];
}

export type FlatItem =
  | { type: "session"; groupName: string; pane: TmuxPane }
  | { type: "notification"; groupName: string; notification: Notification };
