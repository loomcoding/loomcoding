# Why was my tool denied, and how do I allow it?

**What it is**

Tools that change files or run shell commands ask for your permission first. A tool that you deny does not run. This page explains the dialog and the modes that answer without asking.

**What it is for**

- Allowing one action, or a kind of action for the session.
- Understanding why a `--print` run denies every request.

**How**

1. Read the dialog. It shows the tool and its input.
2. Press `a` to allow once, `s` to allow for the session, or `d` or `Esc` to deny.

## Example

```text
Dialog: bash
  command: cargo test
  [Permitir]  [Permitir en la sesión]  [Denegar]
```

`Permitir en la sesión` remembers the pair of tool and action, for example `bash` and `run`. It does not remember one exact command.

## Messages and fixes

| Symptom | Cause | Fix |
|---|---|---|
| You denied a tool by mistake | A denied tool does not run. | Ask the agent to run it again. The dialog opens again. |
| `loom --print` ran, but the tools did not | In print mode every request is denied, unless you also use `--yolo`. | Run `loom` without `--print` and answer the dialog. Use `--yolo` only in a project you trust. |
| A subagent waits in the phase "awaiting permission" | A subagent that needs permission waits for your answer. | Answer its request. The flock header shows the phase. |
| A project agent does not run | Agents defined in the project folder need your approval before they run. | Approve the agent when it is asked. To approve every project agent, see the setting below. |

## Start modes

| Mode | How you start it | What happens with a permission request |
|---|---|---|
| Interactive | `loom` | The dialog opens and you decide. |
| Auto-approve | `loom --yolo` | Every request is approved without asking. Use it only in projects you trust. |
| Print | `loom --print "<prompt>"` | Every request is denied, unless you also use `--yolo`. |

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `--yolo` (launch flag) | Off | Not applicable: it applies to the launch in which you use it |
| `[agents] trust_project_agents` in `loomcoding.toml` | `false` | Yes |

## Result

- An allowed tool runs. A denied tool does not run.
- If you are not sure what a command does, deny it and ask the agent to explain it first.

## Next steps

- [Permissions](../concepts/permissions.md)
- [Agents, subagents and jobs](../concepts/agent-subagents-jobs.md)
- [Configuration](../reference/configuration.md)
