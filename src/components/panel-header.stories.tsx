import type { Meta, StoryObj } from "@storybook/react-vite";
import { PanelHeader } from "./panel-header";

const meta: Meta<typeof PanelHeader> = {
  title: "Components/PanelHeader",
  component: PanelHeader,
  args: {
    globalMuted: false,
    filterNotifiedOnly: false,
    showNonAgentPanes: false,
    appsViewActive: false,
    onToggleFilter: () => {},
    onToggleShowNonAgentPanes: () => {},
    onDeleteAll: () => {},
    onToggleGlobalMute: () => {},
    appVersion: "0.40.0",
    updateStatus: { status: "idle" },
    onUpdateInstall: () => {},
    onUpdateCheck: () => {},
    onToggleAppsView: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof PanelHeader>;

export const Default: Story = {};

export const Muted: Story = {
  args: { globalMuted: true },
};

export const FilterOn: Story = {
  args: { filterNotifiedOnly: true },
};

export const ShowNonAgentPanes: Story = {
  args: { showNonAgentPanes: true },
};

export const AllTogglesOn: Story = {
  args: {
    globalMuted: true,
    filterNotifiedOnly: true,
    showNonAgentPanes: true,
  },
};

export const UpdateChecking: Story = {
  args: { updateStatus: { status: "checking" } },
};

export const UpdateUpToDate: Story = {
  args: { updateStatus: { status: "up-to-date" } },
};

export const UpdateDownloading: Story = {
  args: { updateStatus: { status: "downloading", progress: 42 } },
};

export const UpdateInstalling: Story = {
  args: { updateStatus: { status: "installing" } },
};

export const UpdateReady: Story = {
  args: { updateStatus: { status: "ready" } },
};

export const UpdateError: Story = {
  args: { updateStatus: { status: "error", message: "Update check failed" } },
};
