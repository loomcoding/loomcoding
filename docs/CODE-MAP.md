# Mapa de código orientado a tareas

Cabe señalar que este mapa responde a «¿dónde toco para X?», no a «¿qué es cada módulo?»
(para eso, `docs/ARCHITECTURE.md`). Todas las rutas existen a fecha de 2026-11-05; si mueves
un archivo, corrígelo aquí en el mismo commit.

Antes de nada, para cualquier tarea: lee el decálogo de contexto y coste
(`docs/specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`) y comprueba en
`docs/INDEX.md` que el spec que vas a seguir no está superado.

## Quiero añadir una herramienta (tool)

1. Crea `crates/loomcoding-core/src/tools/<nombre>.rs` e implementa `trait Tool`
   (`name`, `description`, `schema`, `execution_mode`, `run`), tomando como modelo
   `tools/glob.rs` (lectura) o `tools/write.rs` (escritura).
2. Decláralo con `mod <nombre>;` y regístralo en `project_registry`
   (`crates/loomcoding-core/src/tools/mod.rs`, `tools.register(Arc::new(...))`).
3. `execution_mode`: **`Exclusive` si escribe**, `Parallel` si solo lee.
4. **Pon cota a la salida** (decálogo); si puede ser grande, mira `tools/spill.rs`.
5. Test en el propio archivo (`#[cfg(test)]`), sin red real.
6. Si necesita render propio en la TUI: `crates/loomcoding-cloth/src/state/tool_atom/`.

Ojo: `crates/loomcoding-core/src/registry/` NO es el registro de herramientas, es el de agentes.

## Quiero añadir un provider

1. Variante en `ProviderKind` (`crates/loomcoding-core/src/config/mod.rs`).
2. Cliente en `crates/loomcoding-core/src/provider/<nombre>.rs` implementando `trait Provider`
   (`capabilities`, `stream`); `provider/sse.rs` tiene el análisis de SSE común.
3. Selección en `build_provider` (`crates/loomcoding-cli/src/main.rs`).
4. Tests con `wiremock`; nada de red real. Los `tests/*_live.rs` son opt-in.

## Quiero añadir un evento al bus

1. Variante en `LiveEvent` (`crates/loomcoding-core/src/event/mod.rs`).
2. Clasifícala en `is_child_relevant()` (mismo archivo): es lo que decide qué ve el padre.
3. Publícala donde ocurre el hecho (`EventBus`).
4. Consumo en la TUI: `crates/loomcoding-tui/src/app/events.rs` (`App::apply`).
5. ¿Debe sobrevivir al reinicio? Entonces persiste en `crates/loomcoding-core/src/session/sqlite/` (el módulo del tema: `subagents.rs`, `sessions.rs`, `usage.rs`, `workflows.rs` o `gauntlets.rs`; el esquema, en `schema.rs`).
6. Si viaja por IPC al cliente, el reenvío está en `crates/loomcoding-cli/src/server/` (`worker.rs`: `forward_agent_events`; los tipos del cable, en `protocol.rs`).

## Quiero tocar el Gauntlet (constructor + crítico ciego)

1. Reglas puras (fases, orden A/B, veredicto, parseo acotado): `crates/loomcoding-core/src/gauntlet.rs`;
   `GauntletPhase::can_transition_to` es la única tabla de transiciones.
2. Acciones de la herramienta (`start`, `advance`, `accept`, `cancel`, `status`): `crates/loomcoding-core/src/tools/gauntlet/`
   (`start.rs`, `advance.rs`, `decide.rs`, `status.rs`; prompts y `save` en `helpers.rs`).
3. Todo cambio de fase pasa por `helpers::save`, que persiste y publica `LiveEvent::GauntletChanged`: no llames a
   `update_gauntlet` por otro camino o la conversación no lo verá.
4. Persistencia: `session/sqlite/gauntlets.rs` y los métodos de `SessionStore` en `session/mod.rs`.
5. Línea del transcript y reenvío al abrir sesión: `crates/loomcoding-tui/src/app/events.rs` (`GauntletChanged`),
   `run/mod.rs` (`SessionUpdate::Gauntlet`) y `crates/loomcoding-cli/src/server/snapshot.rs`.
6. Puerta de escritura del padre y fin de turno tras `start`: `agent/dispatch.rs` y `agent/turn.rs`.
7. Prueba de punta a punta: `crates/loomcoding-core/src/subagent/tests/gauntlet_e2e.rs`.
8. Guardas de integridad (lectura del crítico, `check`, `hide_bar`): `gauntlet.rs::unread_candidates`,
   `tools/gauntlet/advance.rs` (`apply_next`) y `tools/gauntlet/helpers.rs` (`run_check`); sus pruebas, en
   `tools/gauntlet/tests/guards.rs`.
9. Prueba real contra un servidor local (ignorada): `crates/loomcoding-core/tests/gauntlet_local_live.rs`.

## Quiero añadir una fase de subagente

1. Variante en `SubagentPhase` y en `ALL` (`crates/loomcoding-core/src/subagent/phase.rs`).
2. Completa **todos** los `match` del mismo archivo: `to_task_state`, `from_remote_state`,
   `as_str`, y la tabla de `can_transition_to` (única fuente de verdad de transiciones).
3. Decide si el vigilante debe ignorarla (esperas al humano): `subagent/watchdog.rs`.
4. La fase se persiste por su `as_str` (`session/sqlite/codec.rs` y `subagents.rs`); una fase nueva no rompe filas viejas.
5. Si la UI agrupa estados, nombra entre paréntesis los A2A originales
   (`crates/loomcoding-tui/src/rail/`, sección `subagents.rs`).
6. Tests: `todas_las_fases_proyectan` y compañía ya recorren `ALL`.

## Quiero añadir un widget de TUI

1. Estado puro y testeable en `crates/loomcoding-cloth/src/state/<nombre>.rs` (sin `ratatui`).
2. Widget en `crates/loomcoding-cloth/src/widgets/<nombre>.rs`; decláralo en `widgets/mod.rs`.
3. Colores del tema en `crates/loomcoding-cloth/src/widgets/theme/`; no uses colores sueltos.
4. Úsalo desde la zona que corresponda en `crates/loomcoding-tui/src/render/` (`chrome.rs` cabecera y pie, `transcript.rs`, `sidebar.rs`, `settings.rs`, `overlays.rs`, `a2a.rs`); `render/mod.rs` solo orquesta.
5. Si es específico de Loomcoding y no genérico, va en `crates/loomcoding-tui/src/widgets/`.

## Quiero añadir un comando de composer (`/algo`)

1. Variante en `Command` y fila en la tabla de `crates/loomcoding-tui/src/commands.rs`.
2. Su ejecución, en el `match` de comandos de `crates/loomcoding-tui/src/run/slash.rs`
   (ejemplo: `Command::Verbose`).
3. Estado que necesite: `crates/loomcoding-tui/src/app/` (`mod.rs` define `App`; ver el índice de módulos en el README del crate).
4. El composer no puede parpadear ni ir con retraso: comprueba con `tui_pty` si tocas teclas.

## Quiero cambiar el flujo de permisos

1. Política y servicio: `crates/loomcoding-core/src/permission/mod.rs` (`PermissionService`,
   `PermissionRequest`, `Decision`).
2. Llamada desde la ejecución de tools: `crates/loomcoding-core/src/agent/dispatch.rs`.
3. Diálogo: `crates/loomcoding-tui/src/permission_flow.rs`,
   `crates/loomcoding-tui/src/widgets/permission.rs` y `crates/loomcoding-cloth/src/widgets/permission.rs`.

## Quiero tocar A2A (cliente o servidor)

1. Nombres del cable y tarjeta: `crates/loomcoding-a2a/src/card.rs`, `wire.rs`.
2. Cliente (consumir agentes remotos): `crates/loomcoding-a2a/src/client/` (`mod.rs` llama; `stream.rs` sigue el SSE; `factory.rs` construye clientes), `remote.rs`,
   `oauth.rs`, `security/`, `signing.rs`.
3. Servidor (servir Loomcoding): `crates/loomcoding-a2a/src/serve/` (`rpc/`, `auth.rs`, `host/`).
4. Streaming: `crates/loomcoding-a2a/src/sse.rs`.
5. Modelo interno de tarea: `crates/loomcoding-core/src/task.rs`, `task_events.rs`.
6. **No inventes estados**: contrasta con el proto normativo y rotula lo propio como propio.
7. Anota el avance en `docs/plans/2026-10-22-codeloom-a2a-roadmap.md`.

## Quiero añadir una columna persistida

1. Campo en la estructura (p. ej. `SubagentDescriptor` en `crates/loomcoding-core/src/session/mod.rs`).
2. Migración en `crates/loomcoding-core/src/session/sqlite/schema.rs`: añade la columna a la lista
   `(columna, "ALTER TABLE ... ADD COLUMN ...")` del bucle de migración (idempotente).
3. Lectura y escritura en el mismo archivo; un valor desconocido debe leerse sin romper.
4. Test de ida y vuelta con `SqliteStore`.

## Quiero añadir un umbral o ajuste en `[agents]`

1. Campo `Option<...>` en `AgentsConfig` (`crates/loomcoding-core/src/config/mod.rs`); la clave del
   TOML **es el nombre del campo**.
2. Constantes `DEFAULT_*`/`MIN_*`/`MAX_*` y un método que acote con `clamp` (patrón de
   `active_subagents` y `stall_threshold_ms`).
3. Test con `AgentsConfig::from_toml`.
