# Run a local model server

**What it is**

A local provider is a model server on your own machine, such as `llama-server`. Loomcoding declares each server once, in your profile, and any project can use it.

**What it is for**

- Using a model without sending prompts to a hosted service.
- Switching between local servers with `/provider`.

**How**

1. Start your model server on the port that its entry expects.
2. Start `loom` in your project. It probes each server in the background.
3. Run `/provider` and pick the server. Use `/model` to pick another model id.

## Example

One `[[provider]]` entry in `providers.toml`, with placeholder values:

```toml
default = "llama"

[[provider]]
id = "llama"
name = "llama-server"
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "<model-id>"
context_window = 65536
api_key_env = "<VARIABLE_NAME>"   # optional: the name of a variable, never the key
```

To declare reasoning levels for a local model, add them to the project `loomcoding.toml`:

```toml
[provider]
efforts = ["low", "medium", "high"]
```

## Servers in the registry

The first start seeds these three entries. Their model names are placeholders: set your own.

| id | Server | Port | Key |
|---|---|---|---|
| `llama` | `llama-server` (llama.cpp) | 8081 | None |
| `strata` | Strata | 8080 | Variable `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | None |

The key is never written in the file. A server on a loopback address may omit `api_key_env`, and Loomcoding then sends a placeholder, because `llama-server` does not ask for a key. A remote address always needs the variable.

## Server states

| State | Meaning |
|---|---|
| `up (N)` | The server answered with N models. |
| `down` | Connection refused, timeout, or an answer that is not a model list. |
| `sin acceso (clave)` | The server answered 401 or 403: the key is missing or wrong. |

A `down` server keeps its placeholder model, so you can select it before you start it. The probes repeat when you open `/model`, `/settings` or `/provider`, with a 5-second cooldown.

## Commands

| Command | Default | Requires restart | What it does |
|---|---|---|---|
| `/provider [id]` | Not applicable | No | Switches provider and model. Without an id, opens the list with the state of each server. |
| `/model [id]` | Not applicable | No | Changes the model of the current provider. It accepts any id, including one that is not listed. |
| `/effort [level or default]` | `default` | No | Changes the reasoning effort, limited to the levels the model declares. |
| `/settings` | Not applicable | No | Opens the full form: provider, URL, key variable, model and effort. |

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `default` in `providers.toml` | `llama` | Yes |
| `[provider] efforts` in `loomcoding.toml` | Not set: the catalog decides | Yes after a manual edit |
| `[provider] context_window` in `loomcoding.toml` | Not set: the catalog decides | Yes after a manual edit |
| `[provider] request_timeout_secs` in `loomcoding.toml` | `120` seconds; the maximum is `3600` | Yes after a manual edit |

## Result

| Symptom | Cause and fix |
|---|---|
| `/provider` says `Providers are still loading` | The probe has not answered yet. Wait a moment and try again. |
| A server that is running shows `down` | The port or the `/v1` path in `base_url` does not match the server. Compare them. |
| The state shows `sin acceso (clave)` | Define the variable named in `api_key_env` before you start `loom`. |
| The project still uses the old provider | Its `loomcoding.toml` wins over the registry. Change it with `/provider`. |

For the technical details of the registry, see [the local models document](../../../modelos-locales.md).

## Next steps

- [Send your first prompt](first-prompt.md)
- [Connect a cloud provider](cloud-provider.md)
