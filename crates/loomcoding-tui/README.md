# loomcoding-tui

**Responsabilidad:** la interfaz de terminal: estado de la aplicación, teclas, comandos del
composer y pintado, sobre `ratatui` y los widgets de `cloth`.

**No va aquí:** lógica del motor (`loomcoding-core`); widgets genéricos reutilizables (`cloth`);
el daemon y el IPC (`loomcoding-cli`).

**Tipos clave:** `App` (`app/mod.rs`: todo el estado; sus métodos viven en `app/*.rs` por responsabilidad), `Command` y la tabla de comandos
(`commands.rs`), `PermissionDialog` (`permission_flow.rs`), el rail de subagentes (`rail.rs`).

**Puntos de entrada:** `run::run` (arranque y bucle), `run::handle_key` y `run::handle_event`
(teclas y eventos), `render/` (pintado, un fichero por zona; `render/mod.rs` orquesta), `commands.rs` más el `match` de comandos en `run/slash.rs`.

**Cómo se testea:** tests unitarios de estado y de render en cada módulo; el comportamiento de
extremo a extremo, en `crates/loomcoding-cli/tests/tui_pty.rs`.

**Trampas:**
- **El composer no puede parpadear, colgarse ni ir con retraso**: es el corazón de la app.
- `run/`, `app/` y `render/` están partidos por responsabilidad. En `run/`: `keys.rs` (teclado del composer y transcript), `dialogs.rs` (teclado de ventanas modales), `slash.rs` (comandos con barra), `events.rs` (eventos del motor y envío al worker), `clipboard.rs`; `mod.rs` tiene el bucle `run`.
- Dónde está cada cosa en `app/`: `session.rs` (carga y borrador por sesión, `open_child`), `events.rs` (`apply` de `LiveEvent`), `settings.rs`, `agents.rs`, `usage.rs` (tokens y coste), `children.rs`, `activity.rs`, `tool_sync.rs`, `pointer.rs` (ratón y selección), `prompts.rs` (permisos y preguntas), `composer.rs`, `target.rs` (destinatario del composer y despacho a hijos), `turn.rs`, `prefs.rs`; los tipos, en `types.rs`, `settings_types.rs` y `child_types.rs`.
- Si la UI agrupa estados de A2A, nombra entre paréntesis los originales.
- Toda llamada a herramienta o subagente debe verse en el transcript.
