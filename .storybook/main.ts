import type { StorybookConfig } from "@storybook/react-vite";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { mergeConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

const dirname = path.dirname(fileURLToPath(import.meta.url));

const config: StorybookConfig = {
  framework: { name: "@storybook/react-vite", options: {} },
  stories: ["../src/**/*.stories.@(ts|tsx|mdx)"],
  addons: ["@storybook/addon-docs", "@storybook/addon-a11y"],
  typescript: {
    check: false,
    reactDocgen: "react-docgen-typescript",
    reactDocgenTypescriptOptions: {
      shouldExtractLiteralValuesFromEnum: true,
      propFilter: (prop) => (prop.parent ? !/node_modules/.test(prop.parent.fileName) : true),
    },
  },
  async viteFinal(viteConfig) {
    return mergeConfig(viteConfig, {
      configFile: false,
      plugins: [tailwindcss()],
      resolve: {
        alias: {
          "@": path.resolve(dirname, "../src"),
          "@tauri-apps/api/core": path.resolve(dirname, "./mocks/tauri-core.ts"),
          "@tauri-apps/api/event": path.resolve(dirname, "./mocks/tauri-event.ts"),
        },
      },
    });
  },
};

export default config;
