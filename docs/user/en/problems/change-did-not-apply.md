# Why did my change not apply?

**What it is**

Some settings are read only when Loom starts. A change made while Loom runs does not reach the running session until you restart it.

**What it is for**

- Knowing which changes need a restart.
- Choosing the right way to apply each change.

**How**

1. Check the "Requires restart" column in the [configuration reference](../reference/configuration.md).
2. Close every `loom` window, then start Loom again.

## Example

Turning ODD off with a command saves the value. It applies when Loom restarts:

```text
> /flow odd off
```

The same change in the project file:

```toml
[flows.odd]
enabled = false
```

## Cases

| What you changed | What happens until restart | Fix |
|---|---|---|
| `loomcoding.toml` by hand | The running session keeps the old value. | Restart Loom. |
| `[output_filter] enabled` | The filter stays as it was. | Restart Loom. |
| `/flow odd on` or `/flow odd off` | The saved value applies at the next start. | Restart Loom. |
| `LOOMCODING_NO_FILTER` or `RUST_LOG` | Read at launch. A variable set after launch does nothing. | Set it, then start Loom from that terminal. |
| `providers.toml` | Read at start. | Restart Loom. |
| `/model`, `/provider`, `/effort` | They apply now. | Nothing to do. |
| `/verbose` | It changes the session only, and does not write the file. | Nothing to do. |
| `/settings` form | It applies on save, for the keys it covers. | Keys that the form does not cover need a restart. |

## Settings and defaults

| Key | Requires restart |
|---|---|
| `[provider] request_timeout_secs` | Yes |
| `[flows.odd] enabled` | Yes |
| `[output_filter] enabled` | Yes |
| `[agents] trust_project_agents` | Yes |

## Result

- After the restart, Loom uses the new value.
- If Loom does not start after an edit, the message names the key that is wrong. For example, `request_timeout_secs` must be a whole number from `1` to `3600`.
- If the value still does not apply, check that you edited the `loomcoding.toml` in the folder where you start Loom.

## Next steps

- [Configuration](../reference/configuration.md)
- [ODD and flows (`/flow`)](../concepts/flows-odd.md)
- [Profile folders](../reference/profile-folders.md)
