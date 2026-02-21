import { appendFileSync } from "node:fs"

const DEBUG_LOG = "/tmp/opencode-notify-debug.log"
const log = (msg: string) => appendFileSync(DEBUG_LOG, `${new Date().toISOString()} ${msg}\n`)

export const NotifyPlugin = async ({
  $,
  directory,
}: {
  $: (
    strings: TemplateStringsArray,
    ...values: unknown[]
  ) => {
    text: () => Promise<string>
    catch: (fn: () => string) => Promise<string>
    nothrow: () => { quiet: () => Promise<unknown> }
  }
  directory: string
}) => {
  return {
    event: async ({ event }: { event: { type: string; properties: Record<string, unknown> } }) => {
      log(`event: ${event.type} props: ${JSON.stringify(event.properties)}`)

      const payload = JSON.stringify({
        type: event.type,
        properties: event.properties,
        directory,
      })

      log(`agentoast hook opencode: ${payload}`)
      try {
        await $`agentoast hook opencode ${payload}`.nothrow().quiet()
        log("agentoast: success")
      } catch (e) {
        log(`agentoast: error ${e}`)
      }
    },
  }
}
