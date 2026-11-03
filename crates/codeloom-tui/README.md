# codeloom-tui

**Responsabilidad:** la interfaz de terminal: estado de la aplicación, teclas, comandos del
composer y pintado, sobre `ratatui` y los widgets de `batten`.

**No va aquí:** lógica del motor (`codeloom-core`); widgets genéricos reutilizables (`batten`);
el daemon y el IPC (`codeloom-cli`).

**Tipos clave:** `App` (`app.rs`: todo el estado), `Command` y la tabla de comandos
(`commands.rs`), `PermissionDialog` (`permission_flow.rs`), el rail de subagentes (`rail.rs`).

**Puntos de entrada:** `run::run` (arranque y bucle), `run::handle_key` y `run::handle_event`
(teclas y eventos), `render/` (pintado, un fichero por zona; `render/mod.rs` orquesta), `commands.rs` más el `match` de comandos en `run.rs`.

**Cómo se testea:** tests unitarios de estado y de render en cada módulo; el comportamiento de
extremo a extremo, en `crates/codeloom-cli/tests/tui_pty.rs`.

**Trampas:**
- **El composer no puede parpadear, colgarse ni ir con retraso**: es el corazón de la app.
- `app.rs` y `run.rs` son monolitos (miles de líneas; `render/` ya está partido): localiza antes de leer.
- Si la UI agrupa estados de A2A, nombra entre paréntesis los originales.
- Toda llamada a herramienta o subagente debe verse en el transcript.
