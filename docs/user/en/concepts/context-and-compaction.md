# What happens when the context fills up, and how do I compact it?

**What it is**

A model reads a limited amount of text at once, its context window. Loomcoding estimates how much of that window the history uses. When the history gets close to the limit, it summarizes the older part.

**What it is for**

- Keeping long sessions going without losing the last part of the conversation.
- Sending less text to the model, which lowers the cost.
- Compacting by hand before a large step.

**How**

1. Watch the context meter in the interface.
2. Type `/compact` to summarize the older history now. It works only when the agent is idle.
3. Otherwise, Loomcoding compacts by itself when the history reaches 80% of the window.

## Example

```text
> /compact
```

To prepare the summary in the background, enable it in the project file:

```toml
[compaction]
background = true
```

## How it works

| Item | Value |
|---|---|
| Threshold for automatic compaction | 80% of the model's context window (fixed) |
| Recent history kept intact | The last 10,000 estimated tokens |
| Size of a summary | At most 16,000 bytes, not counting its header |
| Automatic compactions in one turn | At most 3 |

Compacting never deletes the session. The full history stays in the database. Only the text sent to the model becomes shorter.

With `background = true`, a summary that is ready and up to date is used without calling the model. If a summary is still being written, Loom waits a short time for it. If it is not ready in time, Loom compacts in the normal way.

Tool output has its own limit. A long output keeps its beginning and its end, with a note about the omitted part and the path to the full text. An optional output filter removes noise from `bash` and `job_output` results. It is off by default; see the [output filter guide](../../../output-filter.md).

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[compaction] background` | `false` | Yes |
| `[output_filter] enabled` | `false` | Yes |
| `LOOMCODING_NO_FILTER` (environment variable) | Not set. Value `1` turns the filter off always. | Yes. Set it before you start Loom. |

## Result

After `/compact`, the session keeps all its messages, and the next answers use the shorter history. If `/compact` does nothing, the agent is probably busy. Wait for the turn to finish and try again.

## Next steps

- [Sessions](sessions.md)
- [Permissions](permissions.md)
- [Agents, subagents and jobs](agent-subagents-jobs.md)
