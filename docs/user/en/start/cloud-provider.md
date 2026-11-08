# Connect a cloud provider

**What it is**

A cloud provider is a hosted model that you reach over HTTPS with an API key. This page sets one up from `/settings`, using OpenRouter as the example.

**What it is for**

- Using a hosted model when no local server is running.
- Saving the provider in the project, so the next session starts with it.

**How**

1. Start `loom` in your project and type `/settings`.
2. Set the provider kind, the base URL, the name of the key variable and the model.
3. Enter the key in the key field, then save. Or set the variable outside Loomcoding.

## Example

The shape of the project `loomcoding.toml` after saving, with placeholders:

```toml
model = "<model-id>"
max_steps = 8

[provider]
kind = "openai_compat"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
```

To keep the key in a file, copy the repository's [`.env.example`](../../../../.env.example) to `.env` in your project folder, and write the key after `OPENROUTER_API_KEY=`.

## Where the key comes from

Loomcoding looks for the key in this order, and uses the first one it finds:

1. The key entered in `/settings`, for that save.
2. The variable named in `api_key_env`, in the environment of the `loom` process.
3. The same variable, in the `.env` file of the project folder.
4. The Windows Credential Manager entry saved by `/settings`. Windows only.

The key never goes into `loomcoding.toml`. The file stores only the variable name. A local address (`localhost`, `127.0.0.1` or `::1`) needs no key.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `[provider] kind` | Required, no default. One of `openai_compat`, `openai_responses`, `anthropic`, `gemini` | No, after `/settings` |
| `[provider] base_url` | Required, no default. HTTPS, or HTTP for a local address | No, after `/settings` |
| `[provider] api_key_env` | Empty: no variable | No, after `/settings` |
| `model` | Required, no default | No, after `/model` or `/settings` |
| `effort` | Not set: the default level | No, after `/effort` |

## Result

When the save works, the next prompt goes to the provider. If the save fails, the message names the field to fix:

| Message | Fix |
|---|---|
| `Model cannot be empty` | Enter a model id. |
| `Base URL is invalid` | Check that the URL is complete. |
| `Use HTTPS, or HTTP for a localhost endpoint` | Use `https://`. Plain HTTP works only for a local address. |
| `Keep credentials out of the Base URL; use the API key environment variable` | Remove the user, the password and any query string from the URL. |
| `API key variable must be a valid environment variable name` | Use letters, digits and underscores, and do not start with a digit. |
| `secure API key saving is only available on Windows; use the configured environment variable` | On other systems, set the variable instead. |
| `No API key configured. Open /settings and save your provider key.` | No key was found in any of the four places above. |

## Next steps

- [Send your first prompt](first-prompt.md)
- [Run a local model server](local-provider.md)
