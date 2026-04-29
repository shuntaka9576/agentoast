import type { Meta, StoryObj } from "@storybook/react-vite";
import { NotificationCard } from "./notification-card";
import type { Notification } from "@/lib/types";

const base: Notification = {
  id: 1,
  badge: "Stop",
  body: "Task completed: tests passing, ready for review.",
  badgeColor: "green",
  icon: "claude-code",
  metadata: { branch: "feat/storybook" },
  repo: "agentoast",
  tmuxPane: "%42",
  terminalBundleId: "com.googlecode.iterm2",
  forceFocus: false,
  isRead: false,
  createdAt: new Date(Date.now() - 90_000).toISOString(),
};

const meta: Meta<typeof NotificationCard> = {
  title: "Components/NotificationCard",
  component: NotificationCard,
  args: {
    notification: base,
    onDelete: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof NotificationCard>;

export const GreenStop: Story = {};

export const BlueNotification: Story = {
  args: {
    notification: { ...base, badge: "Notification", badgeColor: "blue" },
  },
};

export const RedFailure: Story = {
  args: {
    notification: {
      ...base,
      badge: "Build Failed",
      badgeColor: "red",
      icon: "codex",
      body: "exit code 1 — see logs for details.",
    },
  },
};

export const GrayDefault: Story = {
  args: {
    notification: { ...base, badge: "Idle", badgeColor: "gray", icon: "opencode" },
  },
};

export const NewSelected: Story = {
  args: { isNew: true, isSelected: true },
};

export const LongBody: Story = {
  args: {
    notification: {
      ...base,
      body: "Long body that should be truncated by line-clamp-2 because it spans well beyond two visual lines and demonstrates how the card handles overflow text gracefully without breaking the layout grid.",
    },
  },
};

export const WithoutBadge: Story = {
  args: {
    notification: { ...base, badge: "" },
  },
};

export const WithoutBody: Story = {
  args: {
    notification: { ...base, body: "" },
  },
};

export const ManyMetaFields: Story = {
  args: {
    notification: {
      ...base,
      metadata: { branch: "main", env: "prod", duration: "2.4s" },
    },
  },
};
