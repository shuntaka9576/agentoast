export const NotifyPlugin = async ({
  $,
  directory,
}: {
  $: (
    strings: TemplateStringsArray,
    ...values: unknown[]
  ) => {
    text: () => Promise<string>;
    catch: (fn: () => string) => Promise<string>;
    nothrow: () => { quiet: () => Promise<unknown> };
  };
  directory: string;
}) => {
  return {
    event: async ({ event }: { event: { type: string; properties: Record<string, unknown> } }) => {
      const payload = JSON.stringify({
        type: event.type,
        properties: event.properties,
        directory,
      });

      await $`agentoast hook opencode ${payload}`.nothrow().quiet();
    },
  };
};
