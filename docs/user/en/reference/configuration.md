# Configuration reference (`loomcoding.toml`)

**What it is**

Every key that Loomcoding reads from the project file `loomcoding.toml`, and from two profile files, `providers.toml` and `runtime.toml`. Each key has its type, its default value and whether a change needs a restart. The page follows the code, not the other way round.

**What it is for**

- Finding the key that changes a behaviour, with its allowed range.
- Checking the default value of a key that you have not set.
- Knowing which changes take effect at once and which need a restart.

**How**

1. Open `loomcoding.toml` in the project root. Loom creates it on the first open if it is missing.
2. Add only the keys you want to change. A key you leave out takes its default.
3. Restart Loom to apply the change, unless the table says otherwise in the notes.

## Example

```toml
model = "gpt-4o-mini"
max_steps = 12
effort = "high"

[provider]
kind = "openai_compat"
base_url = "https://api.example.invalid/v1"
api_key_env = "EXAMPLE_API_KEY"

[limits]
temperature = 0.2

[agents]
confirm_delegation = "always"
```

## How values are read

- A new project file takes its provider and model from the default entry of `providers.toml`, or from the first entry if the default is not set. Its `max_steps` is 8.
- Loom reads `loomcoding.toml` when it starts and when it opens the agent. A change made by hand while Loom runs does not affect the running session. Every key below therefore requires a restart.
- `/settings` saves its changes and applies them at once. When you save, Loom also reads again `[provider] prompt_cache`, `request_timeout_secs` and `[limits]`.
- `/flow odd on` and `/flow odd off` save `[flows.odd] enabled`. The change applies on restart.
- `/verbose` changes the session only. It does not write the file.
- MCP servers are configured in `mcp.toml` in the profile, not in this file.

## Top level

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `model` | string | not set (required) | Yes | Model id sent to the provider. |
| `max_steps` | integer | 30 when the key is absent | Yes | Maximum number of agent steps in one turn. |
| `effort` | string | not set (the model default) | Yes | Reasoning effort. Use one of the levels that the model offers; `/effort` lists them. |

## `[provider]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `kind` | enum | not set (required) | Yes | API type: `openai_compat`, `openai_responses`, `anthropic` or `gemini`. |
| `base_url` | string | not set (required) | Yes | Base URL of the API. `/settings` accepts HTTPS, or HTTP for localhost, and rejects credentials inside the URL. |
| `api_key_env` | string | empty | Yes | Name of the environment variable that holds the key. Empty for a local server that needs no key. |
| `prompt_cache` | boolean | not set (on) | Yes | Prompt cache points for the Anthropic adapter. `LOOMCODING_PROMPT_CACHE` overrides it. |
| `context_window` | integer, above 0 | not set (the model catalog decides) | Yes | Tokens per request that the server handles within the time limit. |
| `request_timeout_secs` | integer, 1 to 3600 | 120 | Yes | Total time of each request in the OpenAI-compatible adapter. |
| `efforts` | list of strings, at most 16 | empty (the model catalog decides) | Yes | Effort levels that a local model accepts, in the order the interface shows them. Example: `["none", "low", "xhigh"]`. |

Key lookup for `api_key_env`: the process environment, then the project `.env` file, then the operating system credential store under the same name. Loom never writes the key into the file.

## `[limits]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `max_read_steps` | integer | not set (4 times `max_steps`, at least 1) | Yes | Maximum number of read steps in one model response. |
| `max_parallel_tools` | integer | 8 (0 means 8) | Yes | Maximum number of parallel tool calls in one model response. |
| `max_concurrent_requests` | integer | 4 (0 means 4) | Yes | Maximum number of requests to the provider at the same time. |
| `max_output_tokens` | integer | 4096 (0 means 4096) | Yes | Output token limit of each request, reasoning included. Capped at 32768. |
| `temperature` | number, 0.0 to 2.0 | 0.0 | Yes | Sampling temperature. A value outside the range is an error. |

## `[agents]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `global_dir` | path | `~/.agents/agents` | Yes | Folder of global agents. A leading `~/` expands to the home folder. |
| `project_dir` | path | `.agents/agents` in the project | Yes | Folder of project agents. A relative path starts at the project root. |
| `trust_project_agents` | boolean | false | Yes | Approves every project agent without asking. |
| `confirm_delegation` | enum | `doubt` | Yes | When to ask before delegating. `doubt` asks only when several agents fit; `always` asks every time; `never` does not ask. |
| `routing` | enum | `off` | Yes | Automatic choice of agent. `off` does not choose; `rule` uses the labels of the cards and no model call; `classifier` uses the rule first and then a cheap model. |
| `routing_model` | string | the parent model | Yes | Model of the classifier. It is used only with `routing = "classifier"`. |
| `delegation` | enum | not set (current behaviour) | Yes | `off` removes the subagent tools from the model. `on_request` delegates only when you ask. `proactive` delegates independent subtasks. Only `off` is enforced by code; the other two are rules in the prompt. |
| `max_active_subagents` | integer, 1 to 64 | 8 | Yes | Number of subagents that run at the same time. |
| `max_depth` | integer, 1 to 3 | 1 | Yes | Depth of delegation. 1 means no nested delegation. Out-of-range values are clamped. |
| `stall_threshold_secs` | integer, 10 to 3600 | 120 | Yes | Seconds without a heartbeat before a subagent is marked as stalled. |
| `lost_threshold_secs` | integer, 30 to 21600 | 300 | Yes | Seconds without a heartbeat before a subagent is marked as lost. A lower value is raised to the stall threshold plus 30 seconds. |
| `lost_action` | enum | `notify` | Yes | What happens to a lost subagent. `notify` only reports it; `fail` marks its task as failed and stops its work. |
| `catalog_roots` | list of paths, at most 8 | empty | Yes | Extra roots with `.agents/agents` and `.agents/skills`. Their agents come from third parties, so they need approval. |

## `[a2a]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `role` | enum | `off` | Yes | `off` disables A2A. `serve` exposes local agents to other clients, read-only. `consume` calls remote agents that you have approved. `both` does both. |
| `request_timeout_secs` | integer, 1 to 600 | 30 | Yes | Timeout of each HTTP request to a remote agent. |
| `max_card_bytes` | integer, 1 to 8388608 | 65536 | Yes | Maximum size of a remote agent card. |
| `max_result_bytes` | integer, 1 to 8388608 | 262144 | Yes | Maximum size of the body of a remote response. |
| `card_refresh_secs` | integer | 3600 | Yes | Seconds between downloads of the cards. 0 turns the refresh off. Values below 60 become 60. |
| `ca_file` | path | not set | Yes | PEM file with extra certificate authorities for remote agents. It adds to the public roots. |
| `trusted_jwks` | path | not set | Yes | JWKS file with the keys that sign cards. When it is set, every remote card needs a valid signature. |

## `[a2a.serve]`

Used only when `[a2a] role` is `serve` or `both`. An unknown key in this section is an error.

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `listen` | string | `127.0.0.1:7077` | Yes | Address where the local A2A server listens. |
| `agent` | string | `default` | Yes | Name of the local agent offered to clients. Up to 200 characters. |
| `auth` | table | not set | Yes | Authentication of the clients. See `[a2a.serve.auth]`. |
| `public_url` | string | not set | Yes | Public address advertised in the agent card. Checked with the same rules as `base_url`. |
| `max_concurrent_tasks` | integer, 1 to 64 | 4 | Yes | Number of tasks served at the same time. |
| `max_wait_secs` | integer, 1 to 300 | 30 | Yes | Upper bound, in seconds, for waiting on a task. |
| `max_task_secs` | integer, 1 to 14400 | 600 | Yes | Upper bound, in seconds, for one task. |

## `[a2a.serve.auth]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `scheme` | enum | not set (required with `auth`) | Yes | Authentication scheme: `bearer` or `apikey`. |
| `name` | string | not set | Yes | Header name of the key. Used only with `apikey`. |
| `env` | string | not set (required) | Yes | Environment variable that holds the secret. The secret is never written in the file. |

## `[ui]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `verbose_provider_notices` | boolean | false | Yes | Shows the start and end notices of each provider request in the transcript. Without it, a spinner shows while a request runs. |

## `[compaction]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `background` | boolean | false | Yes | Builds the summary in the background when the context nears its limit. Off uses the synchronous path. |

## `[budget]`

Warnings appear at 80 % and at 100 % of each limit.

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `session_tokens` | integer, above 0 | not set | Yes | Limit of input plus output tokens for the session. |
| `session_cost_usd` | number, above 0 | not set | Yes | Limit in US dollars for the session, from known prices. |
| `hard_stop` | boolean | false | Yes | When true, the agent stops asking the model once the session reaches 100 % of a limit. Otherwise the budget only warns. |

## `[output_filter]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `enabled` | boolean | false | Yes | Compacts the output of `bash` and `job_output` with rules for cargo, git and package managers. The full output stays in `tool-output`. `LOOMCODING_NO_FILTER=1` turns it off. |

## `[flows.<kind>]`

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `enabled` | boolean | true for `odd` | Yes | Turns a flow mode on or off. Only `odd` exists today. A kind has 1 to 32 characters: lowercase letters, digits, `_` or `-`. |

## `providers.toml`

This file lives in the profile folder, not in the project. Loom creates it on first use with three entries: `llama` (a local `llama-server`), `strata` and `polystrata`. Keys are never stored here, only the name of the variable that holds them.

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `default` | string | `llama` in the seed file | Yes | Id of the entry that a new project takes its first settings from. |
| `id` | string | not set (required) | Yes | Short name that you type in `/provider`. A repeated id is ignored; the first entry wins. |
| `name` | string | not set (required) | Yes | Label shown in the interface. |
| `kind` | string | not set (required) | Yes | API type. Only `openai_compat` is accepted today. |
| `base_url` | string | not set (required) | Yes | Base URL, including `/v1`. |
| `api_key_env` | string | not set | Yes | Variable that holds the key, if the server needs one. |
| `model` | string | not set (required) | Yes | Fallback model, used when the server does not answer the probe. |
| `context_window` | integer | not set | Yes | Declared context window. |

## `runtime.toml`

This optional file lives in the profile folder. Loom does not create it. The local server reads it when it starts, so a change needs a restart of that server. It holds an `[a2a]` section with the same keys as above, and a `[supervisor]` section.

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `supervisor` | table | not set | Yes | Starts the supervisor agent, with the settings of the next section. |

## `[supervisor]` in `runtime.toml`

The supervisor answers with a timeout of 15 seconds, fixed in the code. If its settings cannot be used, no supervisor starts.

| Key | Type | Default | Requires restart | What it does |
|---|---|---|---|---|
| `provider` | table | not set (required) | Yes | Provider of the supervisor, with the keys `kind`, `base_url` and `api_key_env` of `[provider]`. |
| `model` | string | not set (required) | Yes | Model of the supervisor. |

## Next steps

- [Environment variables](environment.md)
- [Profile folders](profile-folders.md)
- [Commands](commands.md)
- [ODD and flows (`/flow`)](../concepts/flows-odd.md)
- [A2A](../concepts/a2a.md)
