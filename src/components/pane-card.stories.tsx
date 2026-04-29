import type { Meta, StoryObj } from "@storybook/react-vite";
import { PaneCard } from "./pane-card";
import type { Notification, PaneItem, TmuxPane } from "@/lib/types";

const basePane: TmuxPane = {
  paneId: "%42",
  panePid: 12345,
  sessionName: "dotfiles",
  windowName: "claude",
  currentPath: "/Users/me/repos/dotfiles",
  isActive: false,
  agentType: "claude-code",
  agentStatus: "running",
  waitingReason: null,
  agentModes: ["plan"],
  teamRole: null,
  teamName: null,
  gitRepoRoot: "/Users/me/repos/dotfiles",
  gitBranch: "main",
  currentCommand: null,
};

const baseNotification: Notification = {
  id: 1,
  badge: "Notification",
  body: "Need your input on the next step.",
  badgeColor: "blue",
  icon: "claude-code",
  metadata: { branch: "feat/storybook" },
  repo: "agentoast",
  tmuxPane: "%42",
  terminalBundleId: "com.googlecode.iterm2",
  forceFocus: false,
  isRead: false,
  createdAt: new Date(Date.now() - 60_000).toISOString(),
};

const baseItem: PaneItem = { pane: basePane, notification: null };

const meta: Meta<typeof PaneCard> = {
  title: "Components/PaneCard",
  component: PaneCard,
  args: {
    paneItem: baseItem,
    statusReady: true,
    onDeleteNotification: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof PaneCard>;

export const RunningPlan: Story = {};

export const RunningWithShellAndLocalAgent: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: { ...basePane, agentModes: ["plan", "1 shell", "1 local agent"] },
    },
  },
};

export const Idle: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: { ...basePane, agentStatus: "idle", agentModes: [] },
    },
  },
};

export const WaitingForInput: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: { ...basePane, agentStatus: "waiting", agentModes: [], waitingReason: null },
    },
  },
};

export const WaitingForResponse: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: {
        ...basePane,
        agentStatus: "waiting",
        agentModes: [],
        waitingReason: "respond",
      },
    },
  },
};

export const TeamLead: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: { ...basePane, teamRole: "lead", teamName: null, agentModes: ["plan", "3 teammates"] },
    },
  },
};

export const TeamMate: Story = {
  args: {
    paneItem: {
      ...baseItem,
      pane: {
        ...basePane,
        teamRole: "teammate",
        teamName: "@agent-alpha",
        agentModes: [],
      },
    },
  },
};

export const WithBlueNotification: Story = {
  args: {
    paneItem: { ...baseItem, notification: baseNotification },
    isNew: true,
  },
};

export const WithGreenNotification: Story = {
  args: {
    paneItem: {
      ...baseItem,
      notification: {
        ...baseNotification,
        badge: "Stop",
        badgeColor: "green",
        icon: "claude-code",
      },
    },
  },
};

export const WithRedNotification: Story = {
  args: {
    paneItem: {
      ...baseItem,
      notification: { ...baseNotification, badge: "Build Failed", badgeColor: "red" },
    },
  },
};

export const SelectedAutoExpanded: Story = {
  args: {
    paneItem: {
      ...baseItem,
      notification: {
        ...baseNotification,
        body: "This is a longer body so we can see how the auto-expanded state lays out across multiple lines if needed.",
      },
    },
    isSelected: true,
    isAutoExpanded: true,
  },
};

export const Copied: Story = {
  args: {
    paneItem: { ...baseItem, notification: baseNotification },
    copied: true,
  },
};

export const BarePane: Story = {
  args: {
    paneItem: {
      pane: {
        ...basePane,
        agentType: null,
        agentStatus: null,
        agentModes: [],
        currentCommand: "vim",
      },
      notification: null,
    },
  },
};
