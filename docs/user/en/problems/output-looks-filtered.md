# Why does the tool output look filtered?

**What it is**

The output filter shortens the output of `bash` and `job_output` before the model reads it. It is off by default. When it acts, the output starts with a header that says what was removed.

**What it is for**

- Recognising the header, and knowing where the full output is.
- Getting the full output back, or turning the filter off.

**How**

1. Read the header. It gives the number of lines removed and the file with the full output.
2. Ask for the full output again with `raw: true`, or turn the filter off.

## Example

The header is printed in Spanish, as the app writes it:

```text
[filtrado: 2 líneas omitidas de 5 — regla general; el original sigue en <ruta del volcado>]
inicio

fin
```

## Messages and fixes

| Symptom | Cause | Fix |
|---|---|---|
| The header shows, and a line you need is missing | The filter removed lines that its rule takes for noise. | Ask the agent to run the command again with `raw: true`, or open the file named in the header. |
| The model keeps asking for `raw: true` | The rule removes too much for that command. A rule with 10 or more uses and more than 20 % repeated requests turns itself off for the rest of the process, and Loom shows one notice. | Turn the filter off, as shown below, and report the command. |
| You want no filtering at all | The filter is on in the project. | Set `enabled = false` and restart, or set `LOOMCODING_NO_FILTER` before you start Loom. |

## Turn the filter off

In the project `loomcoding.toml`, then restart Loom:

```toml
[output_filter]
enabled = false
```

For one launch, from PowerShell:

```powershell
$env:LOOMCODING_NO_FILTER = "1"
loom
```

`LOOMCODING_NO_FILTER=1` turns the filter off even when the project turns it on.

## Where the full output is

- The full output is saved in `<profile>/.loomcoding/tool-output/`, and is kept for 7 days.
- The filter applies only to `bash` and `job_output`. It never changes the output of `read`, `grep` or `glob`.
- If Loom cannot save the full output, the output passes unchanged.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[output_filter] enabled` in `loomcoding.toml` | `false` | Yes |
| `LOOMCODING_NO_FILTER` (environment variable) | Not set | Yes. Set it before you start Loom. |

## Result

- With the filter off, the output has no header and is not shortened.
- With the filter on, the header tells you what was removed, and `raw: true` gives the full text.

## Next steps

- [Turn on output compression](../guides/turn-on-output-compression.md)
- [Configuration](../reference/configuration.md)
- [Output filter (technical document)](../../../output-filter.md)
