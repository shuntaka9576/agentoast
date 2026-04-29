type StorybookInvokeRecord = { cmd: string; args: unknown };

declare global {
  interface Window {
    __storybookLastInvoke?: StorybookInvokeRecord;
  }
}

export async function invoke<T = unknown>(cmd: string, args?: unknown): Promise<T> {
  if (typeof window !== "undefined") {
    window.__storybookLastInvoke = { cmd, args };
  }
  console.info("[storybook mock] invoke", cmd, args);
  return undefined as unknown as T;
}
