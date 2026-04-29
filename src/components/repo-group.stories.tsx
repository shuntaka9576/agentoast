import type { Meta, StoryObj } from "@storybook/react-vite";
import { RepoGroup } from "./repo-group";
import type { FlatItem, Notification, PaneItem, TmuxPane } from "@/lib/types";

function makePane(overrides: Partial<TmuxPane>): TmuxPane {
  return {
    paneId: "%0",
    panePid: 1000,
    sessionName: "agentoast",
    windowName: "main",
    currentPath: "/Users/me/repos/agentoast",
    isActive: false,
    agentType: "claude-code",
    agentStatus: "running",
    waitingReason: null,
    agentModes: [],
    teamRole: null,
    teamName: null,
    gitRepoRoot: "/Users/me/repos/agentoast",
    gitBranch: "main",
    currentCommand: null,
    ...overrides,
  };
}

function makeNotification(overrides: Partial<Notification>): Notification {
  return {
    id: 1,
    badge: "Stop",
    body: "Task completed.",
    badgeColor: "green",
    icon: "claude-code",
    metadata: {},
    repo: "agentoast",
    tmuxPane: "%0",
    terminalBundleId: "com.googlecode.iterm2",
    forceFocus: false,
    isRead: false,
    createdAt: new Date(Date.now() - 30_000).toISOString(),
    ...overrides,
  };
}

const soloRunningPane: PaneItem = { pane: makePane({ paneId: "%1" }), notification: null };
const soloIdlePane: PaneItem = {
  pane: makePane({ paneId: "%2", agentStatus: "idle" }),
  notification: null,
};
const waitingPaneWithNotif: PaneItem = {
  pane: makePane({ paneId: "%3", agentStatus: "waiting", waitingReason: "respond" }),
  notification: makeNotification({
    id: 10,
    badge: "Notification",
    badgeColor: "blue",
    tmuxPane: "%3",
  }),
};

function flatItemsForPanes(groupKey: string, panes: PaneItem[]): FlatItem[] {
  return [
    { type: "group-header", groupKey },
    ...panes.map<FlatItem>((paneItem) => ({ type: "pane-item", groupKey, paneItem })),
  ];
}

const groupKey = "/Users/me/repos/agentoast";
const baseSoloPanes = [soloRunningPane, soloIdlePane, waitingPaneWithNotif];

const meta: Meta<typeof RepoGroup> = {
  title: "Components/RepoGroup",
  component: RepoGroup,
  args: {
    groupKey,
    repoName: "agentoast",
    gitBranch: "main",
    paneItems: baseSoloPanes,
    expanded: true,
    isMuted: false,
    isHeaderSelected: false,
    headerNavIndex: 0,
    newIds: new Set<number>(),
    selectedId: null,
    selectedPaneId: null,
    flatItems: flatItemsForPanes(groupKey, baseSoloPanes),
    autoExpandedPaneId: null,
    recentlyCopiedPaneId: null,
    statusReady: true,
    onDeleteNotification: () => {},
    onDeleteByPanes: () => {},
    onToggleRepoMute: () => {},
    onToggleExpand: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof RepoGroup>;

export const ExpandedWithMixedStatus: Story = {};

export const Collapsed: Story = {
  args: { expanded: false },
};

export const Muted: Story = {
  args: { isMuted: true },
};

export const HeaderSelected: Story = {
  args: { isHeaderSelected: true },
};

export const NoBranch: Story = {
  args: {
    gitBranch: null,
    paneItems: [
      { pane: makePane({ paneId: "%9", gitBranch: null, gitRepoRoot: null }), notification: null },
    ],
    flatItems: flatItemsForPanes(groupKey, [
      { pane: makePane({ paneId: "%9", gitBranch: null, gitRepoRoot: null }), notification: null },
    ]),
  },
};

export const StatusNotReady: Story = {
  args: { statusReady: false },
};

export const WithAgentTeams: Story = {
  args: {
    paneItems: [
      {
        pane: makePane({
          paneId: "%20",
          teamRole: "lead",
          agentModes: ["plan", "2 teammates"],
        }),
        notification: null,
      },
      {
        pane: makePane({
          paneId: "%21",
          teamRole: "teammate",
          teamName: "@agent-alpha",
          agentStatus: "running",
        }),
        notification: null,
      },
      {
        pane: makePane({
          paneId: "%22",
          teamRole: "teammate",
          teamName: "@agent-beta",
          agentStatus: "waiting",
          waitingReason: "respond",
        }),
        notification: null,
      },
    ],
    flatItems: flatItemsForPanes(groupKey, [
      {
        pane: makePane({ paneId: "%20", teamRole: "lead", agentModes: ["plan", "2 teammates"] }),
        notification: null,
      },
      {
        pane: makePane({ paneId: "%21", teamRole: "teammate", teamName: "@agent-alpha" }),
        notification: null,
      },
      {
        pane: makePane({
          paneId: "%22",
          teamRole: "teammate",
          teamName: "@agent-beta",
          agentStatus: "waiting",
          waitingReason: "respond",
        }),
        notification: null,
      },
    ]),
  },
};
