# Why does the model not answer, or stop early?

**What it is**

This page covers answers that never arrive, errors from the provider, and turns that end after a tool call. The timeout setting applies to the `openai_compat` adapter only.

**What it is for**

- Reading the message that Loomcoding shows.
- Raising the request timeout when a large model needs more time.

**How**

1. Find the message in the first table and read its cause.
2. Apply the fix. If the fix edits `loomcoding.toml`, restart Loom.

## Example

A request that waits longer than the timeout ends with this message:

```text
OpenAI-compatible request timed out before response headers
```

Raise the timeout in the project `loomcoding.toml`:

```toml
[provider]
request_timeout_secs = 600
```

## Messages and fixes

| Message | Cause | Fix |
|---|---|---|
| `OpenAI-compatible request timed out before response headers` | The server did not send the start of the answer within `request_timeout_secs`. | Raise `request_timeout_secs`, or check that the server is not busy. |
| `OpenAI-compatible request could not connect before response headers` | No connection to `base_url`: the server is off, or the port or the `/v1` path is wrong. | See [Why does a local server show down or no access?](local-server-down.md). |
| `OpenAI-compatible request failed before response headers` | Another error before the answer started. | Check `base_url` and the network, then send the prompt again. |
| `OpenAI-compatible response stream timed out` | The answer started, then stopped for longer than the timeout. | Raise `request_timeout_secs`. |
| `OpenAI-compatible response stream failed` | The connection broke during the answer. | Send the prompt again. |
| `http <status>: <text>` | The provider refused the request. The text after the status gives the reason. | `401` or `403`: check the key in [Connect a cloud provider](../start/cloud-provider.md). A message about the context length: start a clean session with `/new`. |

The Anthropic and Gemini adapters use a fixed 120-second timeout. `request_timeout_secs` does not change it.

## Turns that stop early

| What you see | Cause | Fix |
|---|---|---|
| `[tool result cleared: N bytes; run the tool again if you need it]` in place of an earlier result | Old results were removed to save context. The fix is in the code since 2026-11-05: the current turn keeps its results, and results up to 2,048 bytes are never removed. See the [diagnosis](../../../status/2026-11-05-diagnostico-llamadas-cortadas.md). | Build Loom from the current source (see [Install from source](../start/install.md)). If it still happens, start a clean session with `/new` and send the prompt again. |
| The turn ends after a tool result, with no further steps | The model replied with text and no tool call, so the turn closes. | Send a new prompt. If ODD tasks are open, see [Why does the turn not finish?](turn-wont-finish.md). |

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[provider] request_timeout_secs` in `loomcoding.toml` | `120` seconds; from `1` to `3600` | Yes |

## Result

- After you raise the timeout, a slow answer completes if it arrives within the new limit.
- A value outside `1` to `3600` stops Loom at start. The message names `request_timeout_secs`.

## Next steps

- [Why does a local server show down or no access?](local-server-down.md)
- [Connect a cloud provider](../start/cloud-provider.md)
- [Configuration](../reference/configuration.md)
- [Measure with RUST_LOG](../guides/measure-with-rust-log.md)
