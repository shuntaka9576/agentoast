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
