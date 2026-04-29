import type { Meta, StoryObj } from "@storybook/react-vite";
import { KeybindHelp } from "./keybind-help";

const meta: Meta<typeof KeybindHelp> = {
  title: "Components/KeybindHelp",
  component: KeybindHelp,
  args: {
    onClose: () => {},
  },
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <div style={{ position: "relative", width: 380, height: 520 }}>
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof KeybindHelp>;

export const Open: Story = {};
