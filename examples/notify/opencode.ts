import { appendFileSync } from "node:fs"

const DEBUG_LOG = "/tmp/opencode-notify-debug.log"
const log = (msg: string) => appendFileSync(DEBUG_LOG, `${new Date().toISOString()} ${msg}\n`)

const ENABLE_FOCUS = false

interface StatusProperties {
  sessionID: string
  status: { type: string }
}

const notify = (
  eventType: string,
  properties: Record<string, unknown>,
): { title: string; color: string; focus: boolean } | undefined => {
  if (eventType === "session.status") {
    const props = properties as unknown as StatusProperties
    if (props.status?.type === "idle") {
      return { title: "Stop", color: "red", focus: true }
    }
    return undefined
  }
  if (eventType === "session.error") {
    return { title: "Error", color: "red", focus: false }
  }
  if (eventType === "permission.asked") {
    return { title: "Permission", color: "blue", focus: true }
  }
  return undefined
}

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
  const getGitInfo = async (cwd: string) => {
    try {
      const remote = await $`git -C ${cwd} remote get-url origin`.text().catch(() => "")
      let repoName = ""
      if (remote.trim()) {
        const match = remote.trim().match(/[/:]([^/]+?)(?:\.git)?$/)
        repoName = match ? match[1] : ""
      }
      if (!repoName) {
        repoName = cwd.split("/").pop() || ""
      }
      const branchName = await $`git -C ${cwd} branch --show-current`.text().catch(() => "")
      return { repoName, branchName: branchName.trim() }
    } catch {
      return { repoName: cwd.split("/").pop() || "", branchName: "" }
    }
  }

  return {
    event: async ({ event }: { event: { type: string; properties: Record<string, unknown> } }) => {
      log(`event: ${event.type} props: ${JSON.stringify(event.properties)}`)
      const config = notify(event.type, event.properties)
      if (!config) return
      log(`matched: ${JSON.stringify(config)}`)

      const { repoName, branchName } = await getGitInfo(directory)
      const tmuxPane = process.env.TMUX_PANE || ""

      const args = [
        "send",
        "--title",
        config.title,
        "--color",
        config.color,
        "--icon",
        "opencode",
        "--group",
        repoName,
      ]

      if (tmuxPane) {
        args.push("--tmux-pane", tmuxPane)
      }

      if (branchName) {
        args.push("--meta", `branch=${branchName}`)
      }

      if (ENABLE_FOCUS && config.focus) {
        args.push("--focus")
      }

      log(`agentoast args: ${args.join(" ")}`)
      try {
        await $`agentoast ${args}`.nothrow().quiet()
        log("agentoast: success")
      } catch (e) {
        log(`agentoast: error ${e}`)
      }
    },
  }
}
