# Create an agent profile as a Markdown file

**What it is**

An agent profile is a Markdown file named `<name>.md`. The frontmatter at the top sets its name and options. The body holds the instructions that the agent follows.

**What it is for**

- Giving a subagent a specialty, such as reviewing changes.
- Sharing one agent across all your projects, or keeping it in one repository.
- Running it with `/agent <name> <task>`.

**How**

1. Create `<name>.md` in the agents folder: `~/.agents/agents/` (global) or `.agents/agents/` in the project.
2. Write the frontmatter, with `name` (the same as the file name) and `description`.
3. Write the instructions in the body and save. Loomcoding reloads the folder while it runs.

## Example

```md
---
name: reviewer
description: Reviews a change for bugs and missing tests.
---

You review the changes you are given.
Report the bugs first, then the missing tests.
```

```text
> /agents
> /agent reviewer check the last change in src/parser.rs
```

## Where agent folders live

| Level | Default folder | Setting | Approval |
|---|---|---|---|
| Global | `~/.agents/agents/` | `[agents] global_dir` | Not needed |
| Project | `.agents/agents/` in the project root | `[agents] project_dir` | Needed, in `/agents` |

## Fields

| Field | Required | What it is |
|---|---|---|
| `name` | Yes | Must equal the file name. Up to 64 characters: lowercase letters, digits and hyphens, with no hyphen at the start or end. |
| `description` | Yes | Short text that describes the agent. |
| `model` | No | Model id for this agent. |
| `provider` | No | Its own provider block: `kind`, `base_url` and `api_key_env`. |
| `effort` | No | Reasoning effort level. |
| `max_turns` | No | Turn limit for the agent. |
| `max_depth` | No | How many levels of subagents it may start. |
| `persona` | No | Optional `role`, `goal` and `backstory`. |

Unknown fields are ignored and shown as a warning. The fields `scope` and `delegation` are read, but they are not applied yet.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[agents] global_dir` | `~/.agents/agents` | Yes |
| `[agents] project_dir` | `.agents/agents` in the project | Yes |
| `[agents] trust_project_agents` | `false` | Yes |

Loomcoding reads the `[agents]` section when it starts. Profile files do not need a restart: changes in the folders are picked up while Loomcoding runs.

## Approve a project agent

Project agents wait for your approval before they run.

1. Open `/agents`. A new project agent shows `awaiting approval`.
2. Select it and press `a` to approve, or `r` to revoke.
3. If the file changes after you approve it, the approval stops applying until you approve it again.

Set `trust_project_agents = true` to approve every project agent without asking. Use it only for repositories that you trust.

## Result

`/agents` lists the agent, and `/agent <name> <task>` runs it.

## If it does not work

| Symptom | Fix |
|---|---|
| The agent does not appear. | Check that `name` equals the file name, and that the frontmatter starts and ends with `---` lines. |
| A warning names an unknown field. | Check the spelling. Unknown fields are ignored. |
| A project agent stays `awaiting approval`. | Approve it in `/agents`. |
| An approved project agent stopped working. | The file changed. Review it and approve it again. |

## Next steps

- [Agents, subagents and jobs](../concepts/agent-subagents-jobs.md)
- [Permissions](../concepts/permissions.md)
- [Use a remote A2A agent](use-remote-a2a-agents.md)
