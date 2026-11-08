# How do sessions work and where are they stored?

**What it is**

A session is the record of one conversation: your messages, the agent's answers, the tool calls and the subagents. Loomcoding saves each message in a database as it goes.

**What it is for**

- Coming back to an earlier conversation in the same project.
- Starting a clean conversation without losing the old one.
- Going back from a subagent to the session that started it.

**How**

1. Type `/new` to start an empty session.
2. Type `/sessions` to pick a saved session from the list.
3. Type `/parent` to return from a subagent to its parent session.

## Example

```text
> /new          start an empty session
> /sessions     switch to a saved session
> /focus <id>   send your next messages to a subagent
> /parent       return to the session that started it
```

## Where the data lives

| What | Where |
|---|---|
| Sessions, messages and subagents | `<home>/.loomcoding/sessions.db` |
| Project settings | `loomcoding.toml`, in the project folder |

`<home>` is your home folder, or the folder set in `LOOMCODING_HOME` when that variable is set and not empty. To try Loom without touching your real data, point `LOOMCODING_HOME` to an empty temporary folder.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `LOOMCODING_HOME` (environment variable) | Not set: your home folder | Yes. Set it before you start Loom. |

## Result

`/new` and `/sessions` work only when the agent is idle. If a turn is running, wait for it to finish, or cancel it with `/cancel`.

## Next steps

- [Agents, subagents and jobs](agent-subagents-jobs.md)
- [Context and compaction](context-and-compaction.md)
