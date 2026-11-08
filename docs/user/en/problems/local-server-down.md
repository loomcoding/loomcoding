# Why does a local server show down or no access?

**What it is**

The `/provider` list shows a state next to each local server. This page explains the two states that mean the server cannot answer: `down` and `sin acceso (clave)`.

**What it is for**

- Finding out why a server that is running shows `down`.
- Fixing a key that a server needs.

**How**

1. Run `/provider` and read the state of the server.
2. Apply the fix for that state in the table below.

## Example

Ask the server the same question that Loomcoding asks when it probes it, `GET <base_url>/models`. Use the port of your `base_url`:

```powershell
curl.exe -i http://127.0.0.1:8081/v1/models
```

A `200` answer with a list of models means that Loomcoding can read the server.

## States and fixes

| State | Cause | Fix |
|---|---|---|
| `down` | The connection was refused, the probe timed out after 1.5 seconds, or the answer is not a model list. | Start the server on the port in `base_url`, and check that `base_url` ends in `/v1`. Run the example above. |
| `sin acceso (clave)` | The server answered `401` or `403`: the key is missing or wrong. | Set the variable named in `api_key_env` before you start Loom, then restart Loom. |
| `/provider` says `Providers are still loading` | The probes have not answered yet. | Wait a moment, then open `/provider` again. |
| The project still uses the old provider | The project `loomcoding.toml` wins over the registry. | Run `/provider` and pick the server. |

## Default servers

| id | Server | Port | Key |
|---|---|---|---|
| `llama` | `llama-server` (llama.cpp) | 8081 | None |
| `strata` | Strata | 8080 | `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | None |

A server on a loopback address (`127.0.0.1` or `localhost`) may have no `api_key_env`. A remote address always needs the variable.

## Set the key for one terminal

Set the variable in the same terminal where you start Loom. Replace the placeholder with your key:

```powershell
$env:STRATA_API_KEY = "<your-key>"
loom
```

You can also write the variable in the `.env` file of the project folder. The key never goes into `providers.toml` or `loomcoding.toml`.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `default` in `providers.toml` | `llama` | Yes |

## Result

- When the state shows `up (N)`, the server is ready and you can select its models.
- A `down` server keeps its placeholder model, so you can select it before you start it.

## Next steps

- [Run a local model server](../start/local-provider.md)
- [Use local models](../guides/use-local-models.md)
- [Environment variables](../reference/environment.md)
