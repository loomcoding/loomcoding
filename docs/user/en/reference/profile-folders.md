# Profile folders

**What it is**

The folders and files that Loomcoding keeps in the profile and in the project. The profile is the `.loomcoding` folder under the profile root. The profile root is `LOOMCODING_HOME` when it is set, and your system home folder otherwise. This page writes that folder as `<profile>/.loomcoding/`.

**What it is for**

- Finding the session database, the logs or the saved tool outputs.
- Knowing which files Loom creates by itself, and which ones you write.
- Deleting or backing up a profile on purpose.

**How**

1. Set `LOOMCODING_HOME` to a temporary folder to use a separate profile.
2. Read the first table for the profile, and the second for the project.
3. Do not edit the database or the runtime files by hand. Use the settings and the commands instead.

## Example

```text
<profile>/.loomcoding/
├── providers.toml
├── sessions.db
├── logs/loomcoding-daemon.log
├── tool-output/tool-<call>-<n>.txt
├── runtime/server.key
└── agent-trust.json
```

## Profile folder

| Name | Created by | When | What it holds |
|---|---|---|---|
| `providers.toml` | Loom | When the profile is first read, if the file is missing | Local model servers and the default entry for new projects. See [configuration](configuration.md#providers-toml). |
| `sessions.db` | The local server | When the local server starts | The database (SQLite) of sessions and their messages. |
| `logs/loomcoding-daemon.log` | Loom | Only when `RUST_LOG` is set | Log of the local server. |
| `logs/loomcoding-cliente.log` | Loom | Only when `RUST_LOG` is set | Log of the client. |
| `tool-output/` | Loom | When a tool output is long, or the output filter keeps the original | The full text of each tool output, in files named `tool-<call>-<n>.txt`. The model receives the start and the path. |
| `runtime/server.key` | The local server | When it starts | The secret that the client uses to connect to the local server. On Unix the folder is private to your user. |
| `runtime/agents/` | Loom | When `[a2a] role` is `consume` or `both` | Folders for remote agent peers, one per server start and project. |
| `agent-trust.json` | Loom | When you approve an agent | Your approvals of project and catalog agents. |
| `mcp.toml` | You | Optional | MCP servers. Loom only reads it. |
| `runtime.toml` | You | Optional | Settings read by the local server. See [configuration](configuration.md#runtime-toml). |
| `models-dev.json` | Loom | When it caches the model catalog | The model catalog cache. Its location is not inside the profile; see the note below. |

The cache `models-dev.json` is written to `.loomcoding/` in the folder given by `USERPROFILE`, or by `HOME` when `USERPROFILE` is not set. `LOOMCODING_HOME` does not move it. A test profile therefore does not isolate this file.

## Project folder

These files live in the project root, the folder where you start Loom.

| Name | Created by | When | What it holds |
|---|---|---|---|
| `loomcoding.toml` | Loom | On the first open, if the file is missing | Project settings. See [configuration](configuration.md). |
| `.env` | You | Read when the agent opens | Variables for provider keys and for the commands that Loom runs. Loom never writes this file. See [environment variables](environment.md). |
| `AGENTS.md`, `LOOMCODING.md` | You | Read when the agent opens | Context added to the prompt. Each file is cut at 16,000 bytes, with a notice. |
| `.loomcoding/odd/<feature>.md` | The agent | For large work | The task document of a feature. See [ODD and flows](../concepts/flows-odd.md). |
| `.agents/agents/` | You | Optional | Project agents. This is the default `project_dir`. |

Global agents are read from `.agents/agents/` in your home folder, not from the profile. This is the default `global_dir`.

## Next steps

- [Configuration (`loomcoding.toml`)](configuration.md)
- [Environment variables](environment.md)
- [Permissions](../concepts/permissions.md)
