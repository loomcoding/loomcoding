# Watch a background job while it runs

**What it is**

A job is a shell command that runs in the background. A watch makes Loomcoding send you the output lines that match a pattern while the job is still running, instead of only at the end.

**What it is for**

- Seeing a compile error or a failed test as soon as it appears.
- Waiting for a server to print that it is ready.
- Avoiding repeated checks of the output.

**How**

1. Ask the agent to start the command as a job, with a pattern to watch.
2. Read the `job:` notices in the transcript as they arrive.
3. Ask the agent to change or remove the watch with `job_watch`, or to stop the job with `job_kill`.

## Example

Your prompt:

```text
> Start "cargo watch -x test" as a job and tell me when a test fails.
```

The agent calls `job_start` with a `watch` object, like this:

```json
{ "command": "cargo watch -x test", "label": "tests",
  "watch": { "pattern": "FAILED|error\\[", "max_events": 20 } }
```

Each notice has this shape in the transcript:

```text
job: [Job 2 watch] <matching output line>
```

## Tools

| Tool | Parameters | What it does |
|---|---|---|
| `job_start` | `command`, optional `label`, optional `watch` (`pattern`, `max_events`) | Starts a job. A bad pattern fails before the command starts. |
| `job_watch` | `job_id`, optional `pattern`, optional `max_events` | Adds, changes or removes the watch of a running job. An empty pattern removes it. Only lines printed after the call are watched. |
| `job_output` | job id | Reads the output of a job. |
| `job_list` | none | Lists the jobs of this session. |
| `job_kill` | job id | Stops a job. |

## Limits

| Limit | Value |
|---|---|
| Length of the pattern | 256 bytes at most |
| `max_events` | From 1 to 100. Default 20. |
| Length of each notice line | 512 bytes at most |
| Notices per job | At most one every 5 seconds, with up to 10 lines each |
| Scope | Only jobs of the current session |

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `watch.max_events` (per call) | `20` | No |
| Interval between notices | `5` seconds, fixed | Not configurable |

When the agent is idle, a watch notice can wake it up, within the same budget of automatic turns that subagents use. When the budget runs out, the notice waits for your next message.

## Result

You see `job:` lines while the command runs. When the job ends, the normal end notice arrives and closes the watch.

## If it does not work

| Symptom | Fix |
|---|---|
| `job_start` fails with a watch error. | The pattern is invalid or too long. Simplify it. The command did not start. |
| `job_watch` says the job is finished. | Watches apply only to running jobs. Start a new job. |
| No notices arrive. | No line printed after the call matches the pattern, or the output has not reached the next 5-second window. |
| Notices stop. | The `max_events` limit was reached. A last notice says so. Use `job_watch` with a new `max_events` to set a higher limit. |

## Next steps

- [Agents, subagents and jobs](../concepts/agent-subagents-jobs.md)
- [Permissions](../concepts/permissions.md)
- [Turn on output compression](turn-on-output-compression.md)
