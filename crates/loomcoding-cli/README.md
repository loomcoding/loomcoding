# loomcoding-cli

**Responsabilidad:** construye los binarios `loom` y `loomcoding` (mismo `src/main.rs`), arranca
el daemon que ejecuta el motor y el cliente que lo maneja por IPC.

**No va aquí:** lógica del agente (`loomcoding-core`), widgets o pintado (`loomcoding-tui`,
`cloth`).

**Tipos y funciones clave:** `main` (`main.rs`), `build_provider` (`main.rs`: elige el
proveedor desde la configuración), `run_server`, `connect_or_start`, `serve_connection`,
`run_agent_worker` (`server/worker.rs`; `run_server` y `connect_or_start` en `server/daemon.rs`), `send`/`receive` y `load_or_create_token` (`ipc.rs`).

**Puntos de entrada:** `main.rs` (autodetecta standalone o plugin de `herdr`); `server/`
(daemon, cliente, protocolo, gestor de agentes); `credentials.rs` y `home.rs` (perfil y credenciales); `registro_tracing.rs` (suscriptor de `tracing` a fichero, solo con `RUST_LOG`; guía `docs/logging.md`).

**Cómo se testea:** `tests/e2e.rs` (con `wiremock`) y `tests/tui_pty.rs` (TUI real en un PTY,
vía `loomcoding-e2e`; ver `docs/visual-e2e.md`).

**Trampas:**
- **Nunca ejecutes el binario real sin `LOOMCODING_HOME`** apuntando a un directorio temporal: en
  Windows `dirs::home_dir()` ignora `USERPROFILE` y el cliente se engancha al daemon real.
- Con daemons vivos, `cargo test` directo falla con `Acceso denegado` sobre `loomcoding.exe`; usa
  `scripts\checks.ps1 -TargetDir .verify-target`.
- `server/` agrupa el daemon y el cliente en un mismo módulo; cada zona tiene su fichero (`protocol`, `daemon`, `client`, `manager`, `worker`, `snapshot`, `agent_build`, `agent_ui`, `runtime_tasks`).
