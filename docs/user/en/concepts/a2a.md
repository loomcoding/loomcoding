# What is A2A, and how does Loomcoding use it?

**What it is**

A2A is an open protocol for agents to exchange work as tasks. Loomcoding uses it as the base for talking to other agents, instead of a protocol of its own. Where Loomcoding adds a rule or a state of its own, this page says so.

**What it is for**

- Running subagents as A2A tasks.
- Using remote agents that someone else runs.
- Sharing your local agents with other clients, if you turn that on.

**How**

1. Set the A2A role in `loomcoding.toml`. It is `off` by default.
2. Approve each remote agent in `/agents`.
3. Inspect cards, tasks and exchanges with `/a2a`.

## Example

```toml
[a2a]
role = "consume"   # use remote agents that you have approved
```

## Roles and statuses

| Role (`[a2a] role`) | What it does |
|---|---|
| `off` | Default. No A2A client and no listener. |
| `consume` | Delegates to remote agents that you have already approved. |
| `serve` | Exposes your local agents to other clients. It needs an explicit setting and authentication, kept in `~/.loomcoding/runtime.toml`. |
| `both` | Does both: `consume` and `serve`. |

The A2A protocol defines eight task states: `submitted`, `working`, `input-required`, `auth-required`, `completed`, `failed`, `canceled` and `rejected`.

Subagents have a richer state of their own, which Loomcoding shows in the flock. Each of those states is projected to one A2A state:

| Loomcoding phase (own) | A2A state shown to other agents |
|---|---|
| Queued, Starting | `submitted` |
| Streaming, Reasoning, ToolRunning, AwaitingPermission, Working, Stalled, Unknown | `working` |
| AwaitingParentInput | `input-required` |
| AwaitingCredentials | `auth-required` |
| SettledOk | `completed` |
| SettledErr, Lost | `failed` |
| Rejected | `rejected` |
| Canceled | `canceled` |

Loomcoding's own phases and rules, not defined by A2A:

- The phases `Stalled` and `Lost`, which come from the progress watch on subagents.
- The delegation modes `off`, `on_request` and `proactive`.
- The routing setting, which chooses an agent by itself. A2A does not say how to choose an agent. The routing is off by default.
- The ODD phases in the header, see [ODD](flows-odd.md).

## Remote agents

A remote agent is a profile file with `transport: a2a-remote` and the address of the agent. Loomcoding downloads its card, which describes what the agent can do.

1. Open `/agents` and approve the remote agent. Nothing runs before you approve it.
2. Use `/refresh-agents` to download the cards again.
3. A remote agent can ask you a question while it works. The phase for that is "awaiting parent input".

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[a2a] role` | `off` | Yes |
| `[a2a] request_timeout_secs` | `30` seconds | Yes |
| `[a2a] card_refresh_secs` | `3600` seconds. `0` turns the refresh off. The minimum is 60. | Yes |
| `[a2a] max_card_bytes` | `65536` bytes (64 KiB) | Yes |
| `[a2a] max_result_bytes` | `262144` bytes (256 KiB) | Yes |

Loom reads the `[a2a]` section when it creates the agent. Restart Loom after you edit it.

For the status of each A2A task in the project, see the [A2A roadmap](../../../plans/2026-10-22-codeloom-a2a-roadmap.md).

## Next steps

- [Agents, subagents and jobs](agent-subagents-jobs.md)
- [Permissions](permissions.md)
