# Measure with RUST_LOG and read the log file

**What it is**

`RUST_LOG` makes Loomcoding write its trace events to a log file in your profile. With the variable unset, nothing is written and no file is created.

**What it is for**

- Measuring how much output compression saves.
- Reading the counters of the ODD workflow.
- Checking what happened in a session without reading the terminal.

**How**

1. Set `RUST_LOG` before you start Loomcoding.
2. Use Loomcoding as usual.
3. Read `<profile>/.loomcoding/logs/loomcoding-daemon.log`.

## Example

On a Unix shell, set the variable for one start:

```text
RUST_LOG=output_filter=info loom
```

In PowerShell, set it for the current window:

```powershell
$env:RUST_LOG = 'output_filter=info'
loom
```

A real line from the log looks like this:

```text
2026-11-07T23:47:59.628314Z  INFO output_filter: salida filtrada regla="cargo" original_bytes=2258 sent_bytes=653
```

## Choosing what to log

| Value of `RUST_LOG` | What it records |
|---|---|
| `output_filter=info` | Output compression: one line per filtered output. |
| `odd=info` | ODD workflow counters. |
| `output_filter=info,odd=info` | Both. |
| `info` | Everything at level `info` or higher. |

## Log files

| Process | File |
|---|---|
| Client (the terminal interface or `--print`) | `<profile>/.loomcoding/logs/loomcoding-cliente.log` |
| Daemon (runs the agent) | `<profile>/.loomcoding/logs/loomcoding-daemon.log` |

`<profile>` is your home folder, or the folder in `LOOMCODING_HOME` when that variable is set. Agent events go to the daemon log, because the daemon runs the agent loop.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `RUST_LOG` | not set. Nothing is written. | Yes |
| Size limit of each log | 5 MiB. Then the file rotates to `<name>.log.1`, which keeps one old copy. | Not configurable |

The daemon is a separate process that can stay alive between runs and keeps the setting it started with. Close it, or close all `loom` processes, so that it reads the new value.

## Privacy

The events record rule names, counters and byte counts. They never record command text, paths or tool output. A broad setting such as `debug` can include messages from third-party libraries. Review a log before you share it.

## Result

Each line is one event. To get the savings of a rule, add up `original_bytes` and subtract `sent_bytes` for the same `regla`.

## If it does not work

| Symptom | Fix |
|---|---|
| The log file does not exist. | `RUST_LOG` was not set when Loomcoding started, or an older daemon is still running. Close all `loom` processes and start again. |
| The client log is empty. | This is normal. Agent events are in the daemon log. |
| The file shows size 0 on Windows. | The size can show 0 while the file is open. Read its content. |
| The log is not written at all. | Loomcoding ignores a file it cannot open and keeps running. Check the folder permissions. |

## Next steps

- [Turn on output compression](turn-on-output-compression.md)
- [Watch a background job](watch-a-job.md)
- [Technical note: logging](../../../logging.md)
