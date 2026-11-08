# Why does the turn not finish?

**What it is**

With ODD on, Loomcoding does not close a turn while the tasks of the feature are still open. The header shows the phase, and the task list shows the blocked tasks.

**What it is for**

- Knowing why a turn is still running or has stopped.
- Turning the rule off when it gets in the way.

**How**

1. Read the header and the task list.
2. Answer the question, unblock the task, or turn ODD off and restart Loom.

## Example

Turn ODD off. The change applies when Loom restarts:

```text
> /flow odd off
```

## Messages and fixes

| Symptom | Cause | Fix |
|---|---|---|
| The agent is sent back with a list of open tasks | The rule returns open tasks to the agent. It does this up to 3 times per turn, and never when the list has not changed. | Let the agent finish the tasks. To stop the rule, see the next section. |
| The turn ends with `Necesita tu decisión` | Every open task is blocked. A blocked task shows its reason in the list as `[bloqueada: motivo]`. | Read the reason, answer it, and send a new prompt. |
| A task stays open although the work is done | A task is marked done only with evidence: a real `bash`, `job_start` or `job_output` call from this session. Without one, the task stays open. | Ask the agent to run the check again, and mark the task with that evidence. |
| A reminder about an old feature appears on an unrelated request | After a restart, tasks of an earlier feature can still be open. The first turn can get a reminder. Later turns do not insist. | Turn ODD off, or close the old tasks in `.loomcoding/odd/<feature>.md`. |
| You want the turn to stop now | The turn is still running. | Type `/cancel`. Your cancellation always wins. |

## Turn ODD off

`/flow odd off` saves this setting. Edit it by hand if you prefer:

```toml
[flows.odd]
enabled = false
```

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[flows.odd] enabled` in `loomcoding.toml` | `true` | Yes |

## Result

- With ODD off, the header shows no phase, and the turn can close with open tasks.
- A feature document that is missing or unreadable never blocks a turn.
- The evidence check confirms that the call exists. It does not check that the text is true.

## Next steps

- [ODD and flows (`/flow`)](../concepts/flows-odd.md)
- [Commands](../reference/commands.md)
- [ODD (technical document)](../../../odd.md)
