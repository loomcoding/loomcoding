# Use a remote A2A agent

**What it is**

A remote agent runs on another server and speaks the A2A protocol. Loomcoding downloads its card, which describes what the agent does, and delegates work to it only after you approve it.

**What it is for**

- Delegating part of a task to an agent that someone else runs.
- Checking what a remote agent can do before you trust it.

**How**

1. Set `role = "consume"` in the `[a2a]` section of the project `loomcoding.toml`, then restart Loomcoding.
2. Create a profile `.md` with `transport: a2a-remote` and `a2a.endpoint`.
3. Open `/agents`, check the card, and press `a` to approve.

## Example

```toml
# loomcoding.toml of the project
[a2a]
role = "consume"
```

```md
---
name: devops
description: Answers questions about the deployment pipeline.
transport: a2a-remote
a2a:
  endpoint: https://agents.example.invalid/.well-known/agent-card.json
---
```

Put the profile in `~/.agents/agents/` for all projects, or in `.agents/agents/` for one project.

## Credentials

If the remote agent needs a key, map its security scheme to the **name** of an environment variable. Never write the secret in the file.

```md
a2a:
  endpoint: https://agents.example.invalid/.well-known/agent-card.json
  credentials:
    apiKey: DEVOPS_API_KEY
```

The profile does not load if the value looks like a secret instead of a variable name.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[a2a] role` | `off` | Yes |
| `[a2a] request_timeout_secs` | `30` seconds | Yes |
| `[a2a] card_refresh_secs` | `3600` seconds. `0` turns the refresh off. The minimum is 60. | Yes |
| `[a2a] max_card_bytes` | `65536` (64 KiB) | Yes |
| `[a2a] max_result_bytes` | `262144` (256 KiB) | Yes |
| `[a2a] ca_file` | not set. A PEM file with extra certificate authorities. | Yes |
| `[a2a] trusted_jwks` | not set. A JWKS file for card signatures. | Yes |

| Command | What it does | Requires restart |
|---|---|---|
| `/agents` | Lists the agents. Press `a` to approve and `r` to revoke. | No |
| `/refresh-agents` | Downloads the cards of the remote agents again. | No |
| `/a2a` | Shows cards, tasks and exchanges. | No |

## Result

After you approve it, `/agents` shows the agent as approved, and the agent can receive work. A remote agent can ask you a question while it works. The phase for that is `awaiting parent input`.

Approval is tied to the endpoint and to the card. If the remote agent changes its card, the agent goes back to pending until you approve it again.

## If it does not work

| Symptom | Fix |
|---|---|
| The message "Remote agents are not enabled" appears. | Set `role = "consume"` and restart Loomcoding. |
| The agent is pending and says its card is not downloaded. | Run `/refresh-agents`. An agent without a card cannot be approved. |
| The profile does not load because of the endpoint. | The endpoint must use HTTPS. Plain HTTP is accepted only for loopback addresses. |
| An approved agent went back to pending. | The card changed. Review it, then approve it again. |

## Next steps

- [What is A2A](../concepts/a2a.md)
- [Agents, subagents and jobs](../concepts/agent-subagents-jobs.md)
- [Create an agent profile](create-a-profile-agent.md)
- [Permissions](../concepts/permissions.md)
