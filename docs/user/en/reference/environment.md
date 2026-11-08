# Environment variables

**What it is**

The environment variables that Loomcoding reads, with their default and their effect. It also lists the names that you choose yourself, such as the variable that holds a provider key, and the ones that herdr sets.

**What it is for**

- Running Loom with a separate profile, for tests or for a clean start.
- Turning on the log file, or turning off the output filter.
- Finding where a provider key is read from.

**How**

1. Set the variable in the shell before you start `loom`, or in the project `.env` file for provider keys.
2. Use the name exactly as written here. Names are case-sensitive.
3. Restart Loom after a change to a variable that is read at start. The table says when a variable is read.

## Example

```powershell
$env:LOOMCODING_HOME = "<temp folder>"
$env:RUST_LOG = "odd=info"
loom
```

This starts Loom with a separate profile, and writes the ODD events to the log file in `logs/` of that profile.

## Loom variables

| Variable | Default | What it does |
|---|---|---|
| `LOOMCODING_HOME` | the system home folder | Root of the profile. Loom keeps its files in `<value>/.loomcoding/`. Use a temporary folder to run with a separate profile. |
| `RUST_LOG` | not set | When set and not empty, Loom writes a log file in `logs/` of the profile. The value is a filter, for example `odd=info`. The file is rotated at 5 MiB. |
| `LOOMCODING_NO_FILTER` | not set | `1` turns off the output filter for the whole process, even if `[output_filter] enabled` is true. |
| `LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE` | 128 | Maximum number of tool calls in one model response. Only a positive whole number is used; other values are ignored. |
| `LOOMCODING_MAX_TOOL_CALLS_PER_TURN` | 256 | Maximum number of tool calls in one turn. Same rule as above. |
| `LOOMCODING_PROMPT_CACHE` | not set | `0`, `off` or `false` turns prompt caching off in the Anthropic adapter. Any other value turns it on. When not set, `[provider] prompt_cache` decides, and the default is on. |
| `LOOMCODING_PROGRAMMATIC_TOOL_CALLING` | off | `1`, `true` or `yes` enables the programmatic tool-calling path of the agent loop. Any other value keeps it off. |
| `LOOMCODING_NO_CLIPBOARD` | not set | When set, to any value, Loom does not read images from the clipboard. Pasting text still works. |
| `NO_COLOR` | not set | When set, to any value, the braille mascot and the logo effects are turned off. |
| `USERPROFILE`, `HOME` | the system values | Used only to find the model catalog cache, `models-dev.json`. `LOOMCODING_HOME` does not move this file. |

## Provider keys and credentials

The name of a provider key variable is the value of `api_key_env` in `loomcoding.toml` or in `providers.toml`. You choose the name. These names appear by default in the files and in the settings screen:

| Name | Where it appears by default |
|---|---|
| `OPENROUTER_API_KEY` | The fallback project file, used when `providers.toml` cannot be used. |
| `STRATA_API_KEY` | The `strata` entry of `providers.toml`. |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` | The provider presets of the settings screen. |

Loom looks for a key in this order, and uses the first one that is not empty:

1. The process environment, under the name in `api_key_env`.
2. The `.env` file in the project root, under the same name.
3. The operating system credential store, under the same name.

For a local address (localhost or a loopback IP) with no key, Loom uses a placeholder key, so a local server that needs no key works without one. Loom does not write keys to any file.

## Variables you name yourself

| Setting | What the variable holds |
|---|---|
| `api_key_env` in `[provider]` or in `providers.toml` | The provider key. See the previous section. |
| `env` in `[a2a.serve.auth]` | The secret that clients must send to the local A2A server. |
| `env` in the authentication of a remote agent profile | The secret for a remote A2A agent. |
| `env` values of an MCP server in `mcp.toml` | The host variables that are passed on to the server. The map goes from the name the server sees to the host name. |

## Variables set by herdr

herdr sets these when it starts Loom as a plugin. You do not set them. The `loomcoding-herdr` plugin reads them.

| Variable | What it holds |
|---|---|
| `HERDR_ENV` | `1` when Loom runs inside herdr. |
| `HERDR_PLUGIN_CONTEXT_JSON` | The context of the workspace, as JSON. |
| `HERDR_PANE_ID` | The identifier of the pane. |
| `HERDR_PLUGIN_ID` | The identifier of the plugin. |
| `HERDR_BIN_PATH` | The path of the herdr program. Default: `herdr`. |

## Variables seen by commands that Loom runs

The `bash` tool does not pass your whole environment to the commands it runs. It passes a fixed list of system variables, among them `PATH`, `SYSTEMROOT`, `TEMP`, `HOME`, `USERPROFILE`, `LANG`, `CARGO_HOME` and `RUSTUP_HOME`. It also passes the variables of the project `.env` file. Other variables of your shell are not passed.

## Next steps

- [Configuration (`loomcoding.toml`)](configuration.md)
- [Profile folders](profile-folders.md)
- [Commands](commands.md)
