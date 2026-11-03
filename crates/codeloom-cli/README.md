# codeloom-cli

**Responsabilidad:** construye los binarios `loom` y `codeloom` (mismo `src/main.rs`), arranca
el daemon que ejecuta el motor y el cliente que lo maneja por IPC.

**No va aquí:** lógica del agente (`codeloom-core`), widgets o pintado (`codeloom-tui`,
`batten`).

**Tipos y funciones clave:** `main` (`main.rs`), `build_provider` (`main.rs`: elige el
proveedor desde la configuración), `run_server`, `connect_or_start`, `serve_connection`,
`run_agent_worker` (`server.rs`), `send`/`receive` y `load_or_create_token` (`ipc.rs`).

**Puntos de entrada:** `main.rs` (autodetecta standalone o plugin de `herdr`); `server.rs`
(daemon); `credentials.rs` y `home.rs` (perfil y credenciales).

**Cómo se testea:** `tests/e2e.rs` (con `wiremock`) y `tests/tui_pty.rs` (TUI real en un PTY,
vía `codeloom-e2e`; ver `docs/visual-e2e.md`).

**Trampas:**
- **Nunca ejecutes el binario real sin `CODELOOM_HOME`** apuntando a un directorio temporal: en
  Windows `dirs::home_dir()` ignora `USERPROFILE` y el cliente se engancha al daemon real.
- Con daemons vivos, `cargo test` directo falla con `Acceso denegado` sobre `codeloom.exe`; usa
  `scripts\checks.ps1 -TargetDir .verify-target`.
- `server.rs` es un monolito (más de 3.000 líneas): ver el plan de organización.
