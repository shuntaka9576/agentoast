import type { Meta, StoryObj } from "@storybook/react-vite";
import { AppsTabIcon, IconPreset, TmuxIcon } from "./source-icon";
import type { Icon } from "@/lib/types";

const meta: Meta<typeof IconPreset> = {
  title: "Components/Icons",
  component: IconPreset,
  args: {
    icon: "agentoast",
    size: 16,
  },
};

export default meta;
type Story = StoryObj<typeof IconPreset>;

const PRESETS: Icon[] = ["agentoast", "claude-code", "codex", "copilot-cli", "opencode"];
const SIZES = [12, 14, 16, 20, 24];

export const Agentoast: Story = { args: { icon: "agentoast" } };
export const ClaudeCode: Story = { args: { icon: "claude-code" } };
export const Codex: Story = { args: { icon: "codex" } };
export const CopilotCli: Story = { args: { icon: "copilot-cli" } };
export const Opencode: Story = { args: { icon: "opencode" } };

export const AllPresets: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 16, alignItems: "center" }}>
      {PRESETS.map((icon) => (
        <div
          key={icon}
          style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "center" }}
        >
          <IconPreset icon={icon} size={20} className="text-[var(--text-primary)]" />
          <span style={{ fontSize: 10, color: "var(--text-muted)" }}>{icon}</span>
        </div>
      ))}
    </div>
  ),
};

export const AllSizes: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 16, alignItems: "flex-end" }}>
      {SIZES.map((size) => (
        <div
          key={size}
          style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "center" }}
        >
          <IconPreset icon="claude-code" size={size} className="text-[var(--text-primary)]" />
          <span style={{ fontSize: 10, color: "var(--text-muted)" }}>{size}px</span>
        </div>
      ))}
    </div>
  ),
};

export const TmuxAndAppsTab: Story = {
  render: () => (
    <div style={{ display: "flex", gap: 16, alignItems: "center" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "center" }}>
        <TmuxIcon size={20} className="text-[var(--text-primary)]" />
        <span style={{ fontSize: 10, color: "var(--text-muted)" }}>TmuxIcon</span>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "center" }}>
        <AppsTabIcon size={20} className="text-[var(--text-primary)]" />
        <span style={{ fontSize: 10, color: "var(--text-muted)" }}>AppsTabIcon</span>
      </div>
    </div>
  ),
};
