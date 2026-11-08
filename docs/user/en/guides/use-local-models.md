# Declare and switch local model servers

**What it is**

A local model server runs on your own machine, for example `llama-server`, Strata or PolyStrata. Loomcoding declares each one once, in a global registry, and every project can use it.

**What it is for**

- Using a model that runs on your machine, without a cloud key.
- Switching between a cloud provider and a local server during a session.
- Adding another server that speaks the OpenAI-compatible API.

**How**

1. Start the server on the port that its entry declares.
2. Start Loomcoding. It checks each server once, in the background.
3. Type `/provider` to choose a server, then `/model <id>` to choose its model.

## Example

```toml
# <profile>/.loomcoding/providers.toml
default = "llama"

[[provider]]
id = "llama"
name = "llama-server"
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "your-model-name"
```

Then, in Loomcoding:

```text
> /provider llama
> /model your-model-name
```

## Where the registry lives

`<profile>` is your home folder, or the folder in `LOOMCODING_HOME` when that variable is set. Loomcoding creates `providers.toml` on the first start, with the three servers below. It never overwrites a file that already exists.

| id | Server | Default port | Key |
|---|---|---|---|
| `llama` | llama-server (llama.cpp) | 8081 | none |
| `strata` | Strata | 8080 | variable `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | none |

The key is never written in the file. `api_key_env` holds the name of the variable that contains it. A server on the loopback address (`localhost` or `127.0.0.1`) can leave `api_key_env` out.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `default` in `providers.toml` | `"llama"` | Yes |
| `[[provider]] base_url` | `http://127.0.0.1:8081/v1` for `llama` | Yes |
| `[[provider]] api_key_env` | not set (allowed only on loopback) | Yes |
| `[[provider]] context_window` | not set | Yes |

| Command | What it does | Requires restart |
|---|---|---|
| `/provider [id]` | Switches provider and model. Without an id, opens the picker with the status of each server. | No |
| `/model [id]` | Changes the model of the current provider. Accepts any id, even one that is not listed. | No |
| `/settings` | Opens the full form: provider, URL, key variable, model and effort. | No |

`/provider` and `/model` save the choice in the `loomcoding.toml` of the project.

## Server status

| Status | Meaning |
|---|---|
| `up (N)` | The server answered with N models. |
| `down` | The connection was refused, the wait ran out, or the answer is not a model list. |
| `sin acceso (clave)` | The server answered 401 or 403: the key is missing or wrong. |

A server that is down keeps its fallback model, so you can select it before you start it.

## Result

When it works, `/provider` lists each server with its status. The next prompt goes to the server and model you chose.

## If it does not work

| Symptom | Fix |
|---|---|
| `/provider` says "Providers are still loading". | The first check has not finished. Wait a moment. |
| A running server shows `down`. | Check that the port and the `/v1` path in `base_url` match the server. |
| A server shows `sin acceso (clave)`. | Define the variable named in `api_key_env` before you start Loomcoding. |
| A project still uses the old provider. | Its `loomcoding.toml` wins. Change it with `/provider`. |

To declare reasoning effort levels for a local model, see the technical note.

## Next steps

- [Run a local model server](../start/local-provider.md)
- [Connect a cloud provider](../start/cloud-provider.md)
- [Customize the interface](customize-the-interface.md)
- [Technical note: local models](../../../modelos-locales.md)
