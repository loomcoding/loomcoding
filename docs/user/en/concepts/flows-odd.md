# What is ODD, and how do I turn flows on or off with /flow?

**What it is**

ODD (organic development) is a light workflow for feature work. It is built into Loomcoding and is on by default. It shows only when the work is large. ODD is a Loomcoding label. It is not an A2A state.

**What it is for**

- Keeping the tasks of a feature in a document in your project.
- Seeing the current phase in the header.
- Stopping a turn from closing while tasks are still open.

**How**

1. Ask for a feature.
2. For large work, the agent creates `.loomcoding/odd/<feature>.md` in the project, with the tasks as checkboxes.
3. Type `/flow` to see the flow modes. To turn ODD off, type `/flow odd off`, then restart Loom.

## Example

```text
> /flow
> /flow odd off
```

The change is saved in `loomcoding.toml` and applies when Loom restarts.

## How it works

The header shows the phase of the work:

| Header | Set by |
|---|---|
| `ODD · exploring` | Reading files |
| `ODD · implementing (2/5)` | Writing or editing files, with progress |
| `ODD · checking` | Running a known test, build, clippy or fmt command |
| `ODD · deciding` | Asking you a question |
| `ODD · planning` | Updating the feature document |

While a `gauntlet` or a `development_workflow` runs, the header shows that workflow's phase instead.

A task is marked as done only with evidence: a real `bash`, `job_start` or `job_output` call from this session. Loomcoding checks that the call exists. It does not check that the text of the evidence is true.

If the agent tries to finish the turn with open tasks, Loomcoding sends the list back to it. This happens at most 3 times per turn, and never when the list has not changed. A blocked task shows its reason. If every open task is blocked, the turn ends with the message "Necesita tu decisión" (the app shows it in Spanish, and it means "Needs your decision").

A feature document that is missing or unreadable never blocks a turn. Subagents do not see the ODD task tool or the closing rule.

To count ODD events in the log, set `RUST_LOG=odd=info`. See the [logging guide](../../../logging.md).

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[flows.odd] enabled` | `true` | Yes |

`/flow` lists the modes and their state. `/flow odd on` and `/flow odd off` save this setting. The change applies only when Loom restarts.

## Known limits

- The evidence check confirms that the call exists, not that its text is true.
- After a restart, if tasks from an earlier feature are still open, the first turn can receive a reminder even when your request is unrelated. After that, it does not insist.
- The phase is kept in memory. After a restart it is "idle" until the first tool runs.
- `/flow odd on` and `/flow odd off` do not act on the running session.

## Next steps

- [Agents, subagents and jobs](agent-subagents-jobs.md)
- [Permissions](permissions.md)
