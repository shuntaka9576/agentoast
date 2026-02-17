#!/usr/bin/env -S deno run --allow-run --allow-env --allow-read

// NOTE: `agentoast hook claude` is the recommended approach (no Deno dependency).
// This script is kept as a reference implementation.

interface HookData {
  session_id: string
  transcript_path: string
  hook_event_name: "Stop" | "Notification"
  notification_type?: "permission_prompt" | "idle_prompt" | "auth_success" | "elicitation_dialog"
  stop_hook_active?: boolean
}

// Set to true to auto-focus terminal on Stop/permission_prompt/elicitation_dialog events
// When enabled, these notifications will silently focus the terminal without showing a toast
const ENABLE_FOCUS = false
const FOCUS_EVENTS = new Set(["Stop", "permission_prompt", "elicitation_dialog"])

const runCommand = async (
  cmd: string,
  args: string[],
  cwd?: string,
): Promise<{ success: boolean; stdout: string }> => {
  const command = new Deno.Command(cmd, {
    args,
    cwd,
    stdout: "piped",
    stderr: "piped",
  })
  const result = await command.output()
  return {
    success: result.success,
    stdout: new TextDecoder().decode(result.stdout).trim(),
  }
}

const getGitInfo = async (cwd: string): Promise<{ repoName: string; branchName: string }> => {
  const gitCheck = await runCommand("git", ["rev-parse", "--is-inside-work-tree"], cwd)
  const isGitRepo = gitCheck.success && gitCheck.stdout === "true"

  let repoName = ""
  let branchName = ""

  if (isGitRepo) {
    const remote = await runCommand("git", ["remote", "get-url", "origin"], cwd)

    if (remote.stdout && remote.success) {
      const match = remote.stdout.match(/[/:]([^/]+?)(?:\.git)?$/)
      repoName = match ? match[1] : ""
    }

    if (!repoName) {
      repoName = cwd.split("/").pop() || ""
    }

    const branch = await runCommand("git", ["branch", "--show-current"], cwd)
    branchName = branch.stdout
  } else {
    repoName = cwd.split("/").pop() || ""
  }

  return { repoName, branchName }
}

const main = async () => {
  const input = await new Response(Deno.stdin.readable).text()
  const data: HookData = JSON.parse(input)

  const isStop = data.hook_event_name === "Stop"
  const title = data.hook_event_name
  const color = isStop ? "green" : "blue"
  const eventKey = data.notification_type || data.hook_event_name
  const focus = ENABLE_FOCUS && FOCUS_EVENTS.has(eventKey)

  const cwd = Deno.cwd()
  const { repoName, branchName } = await getGitInfo(cwd)
  const tmuxPane = Deno.env.get("TMUX_PANE") || ""

  const args = [
    "send",
    "--title",
    title,
    "--color",
    color,
    "--icon",
    "claude-code",
    "--repo",
    repoName,
  ]

  if (tmuxPane) {
    args.push("--tmux-pane", tmuxPane)
  }

  if (branchName) {
    args.push("--meta", `branch=${branchName}`)
  }

  if (focus) {
    args.push("--focus")
  }

  await runCommand("agentoast", args)

  console.log(JSON.stringify({ success: true }))
}

try {
  await main()
} catch (error) {
  console.log(
    JSON.stringify({
      success: false,
      error: error instanceof Error ? error.message : String(error),
    }),
  )
}
