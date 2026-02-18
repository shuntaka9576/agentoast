#!/usr/bin/env -S deno run --allow-run --allow-env --allow-read

interface CodexPayload {
  type: string
  "turn-id"?: string
  cwd?: string
  [key: string]: unknown
}

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
  const jsonArg = Deno.args[Deno.args.length - 1]
  const data: CodexPayload = JSON.parse(jsonArg)

  const cwd = data.cwd || Deno.cwd()
  const badge = "Notification"
  const badgeColor = "blue"

  const { repoName, branchName } = await getGitInfo(cwd)
  const tmuxPane = Deno.env.get("TMUX_PANE") || ""

  const args = [
    "send",
    "--badge",
    badge,
    "--badge-color",
    badgeColor,
    "--icon",
    "codex",
    "--repo",
    repoName,
  ]

  if (tmuxPane) {
    args.push("--tmux-pane", tmuxPane)
  }

  if (branchName) {
    args.push("--meta", `branch=${branchName}`)
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
