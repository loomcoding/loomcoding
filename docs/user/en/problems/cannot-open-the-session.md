# Why can I not open the session?

**What it is**

Loomcoding keeps the sessions in a local server, and the Loom client connects to it. When the connection fails, Loom shows one of the messages in the table below.

**What it is for**

- Matching a message to its cause.
- Starting cleanly after an upgrade, or with a separate test profile.

**How**

1. Note the message on screen.
2. Find it in the table and apply the fix.

## Example

List the Loom processes that are running. Check them before you stop anything:

```powershell
Get-Process loom, loomcoding -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, StartTime
```

Stop them only if you are sure they belong to the profile you want to use:

```powershell
Stop-Process -Name loom, loomcoding -Force
```

Stopping a process ends any turn that it is running. Messages already saved stay in `sessions.db`.

## Messages and fixes

| Message or symptom | Cause | Fix |
|---|---|---|
| `autenticación local rechazada` | The running local server has a different key from this client. Usually a server from another profile is still running. | Close all Loom processes. Start Loom again, always with the same `LOOMCODING_HOME`. |
| `protocolo local incompatible` | The running server is from another version, usually an older one. | Close all Loom processes before you start the new version. |
| `servidor Loomcoding incompatible (protocolo N)` | The server answered with another protocol version. | Close all Loom processes, then start again. |
| `el servidor local no está disponible` | No server answered for about 10 seconds after Loom tried to start one. | Close any `loom` process that is still running, then start again. For the cause, see [Measure with RUST_LOG](../guides/measure-with-rust-log.md). |
| `sessions.db` cannot be opened, or stays locked | A server from a `--print` run, or from a test profile, still holds the database. | Close that server process, then start again. |

## Use a separate profile

Loomcoding uses `LOOMCODING_HOME` as the profile folder when it is set. Use the same value in every terminal that starts Loom:

```powershell
$env:LOOMCODING_HOME = "<empty-folder>"
loom
```

## Before you upgrade

Close every `loom` and `loomcoding` process before you install a new version. The old server does not find the new profile folder.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `LOOMCODING_HOME` (environment variable) | Not set: your home folder | Yes. Set it before you start Loom. |

## Result

- When the session opens with your data, the connection works.
- Developers: a running server locks `loomcoding.exe`, so `cargo test` can fail with `Acceso denegado`. Stop the servers first.

## Next steps

- [Sessions](../concepts/sessions.md)
- [Profile folders](../reference/profile-folders.md)
- [Environment variables](../reference/environment.md)
- [Measure with RUST_LOG](../guides/measure-with-rust-log.md)
