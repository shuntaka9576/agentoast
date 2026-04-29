/// <reference path="./css.d.ts" />
import type { Preview } from "@storybook/react-vite";
import type React from "react";
import "../src/index.css";

type ColorScheme = "light" | "dark";

const darkVars: React.CSSProperties = {
  ["--panel-bg" as never]: "rgba(28, 28, 30, 0.95)",
  ["--toast-bg" as never]: "rgba(44, 44, 46, 0.95)",
  ["--text-primary" as never]: "rgba(255, 255, 255, 0.9)",
  ["--text-secondary" as never]: "rgba(255, 255, 255, 0.7)",
  ["--text-tertiary" as never]: "rgba(255, 255, 255, 0.5)",
  ["--text-muted" as never]: "rgba(255, 255, 255, 0.4)",
  ["--text-faint" as never]: "rgba(255, 255, 255, 0.2)",
  ["--border-primary" as never]: "rgba(255, 255, 255, 0.1)",
  ["--border-subtle" as never]: "rgba(255, 255, 255, 0.05)",
  ["--hover-bg" as never]: "rgba(255, 255, 255, 0.05)",
  ["--hover-bg-strong" as never]: "rgba(255, 255, 255, 0.1)",
  ["--badge-stop-bg" as never]: "rgba(34, 197, 94, 0.2)",
  ["--badge-stop-text" as never]: "#4ade80",
  ["--badge-notif-bg" as never]: "rgba(59, 130, 246, 0.2)",
  ["--badge-notif-text" as never]: "#60a5fa",
  ["--tray-arrow-fill" as never]: "rgba(28, 28, 30, 0.95)",
  ["--scrollbar-thumb" as never]: "rgba(255, 255, 255, 0.15)",
  ["--scrollbar-thumb-hover" as never]: "rgba(255, 255, 255, 0.25)",
  ["--delete-text" as never]: "rgba(255, 255, 255, 0.3)",
  ["--delete-text-hover" as never]: "rgba(255, 255, 255, 0.6)",
  ["--delete-hover-text" as never]: "#f87171",
  ["--toast-focus-bg" as never]: "rgba(55, 40, 80, 0.95)",
  ["--border-focus" as never]: "rgba(139, 92, 246, 0.4)",
  ["--badge-focus-bg" as never]: "rgba(139, 92, 246, 0.25)",
  ["--badge-focus-text" as never]: "#a78bfa",
  ["--new-highlight-bg" as never]: "rgba(59, 130, 246, 0.12)",
  ["--selection-ring" as never]: "inset 0 0 0 1.5px rgba(255, 255, 255, 0.2)",
  ["--group-selection-ring" as never]:
    "inset 0 0 0 1.5px rgba(34, 211, 238, 0.85), 0 0 14px rgba(34, 211, 238, 0.35)",
  ["--surface-card" as never]: "rgba(255, 255, 255, 0.04)",
  ["--surface-elevated" as never]: "rgba(60, 60, 62, 0.95)",
  ["--accent" as never]: "#c9956e",
  ["--accent-hover" as never]: "#d4a586",
  ["--accent-bg" as never]: "rgba(201, 149, 110, 0.18)",
  ["--switch-track" as never]: "rgba(255, 255, 255, 0.2)",
  ["--shadow-card" as never]: "0 1px 0 rgba(0, 0, 0, 0.3), 0 0 0 1px var(--border-subtle)",
  ["--row-hover" as never]: "rgba(255, 255, 255, 0.025)",
  ["--banner-warn-bg" as never]: "rgba(201, 149, 110, 0.1)",
};

const preview: Preview = {
  parameters: {
    controls: { matchers: { color: /(background|color)$/i, date: /Date$/ } },
    layout: "centered",
    backgrounds: { disable: true },
  },
  globalTypes: {
    colorScheme: {
      description: "Light / Dark color scheme override (CSS variables)",
      defaultValue: "light",
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: [
          { value: "light", title: "Light" },
          { value: "dark", title: "Dark" },
        ],
        dynamicTitle: true,
      },
    },
  },
  decorators: [
    (Story, ctx) => {
      const scheme = (ctx.globals.colorScheme ?? "light") as ColorScheme;
      const overrides = scheme === "dark" ? darkVars : ({} as React.CSSProperties);
      return (
        <div
          data-color-scheme={scheme}
          style={{
            minWidth: 360,
            background: "var(--panel-bg)",
            color: "var(--text-primary)",
            padding: 16,
            borderRadius: 8,
            ...overrides,
          }}
        >
          <Story />
        </div>
      );
    },
  ],
};

export default preview;
