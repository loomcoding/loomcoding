# loomcoding-core

**Responsabilidad:** el motor puro: bucle del agente, sesiones, herramientas, providers,
eventos, tareas A2A internas y subagentes.

**No va aquí:** nada de TUI, `ratatui`, `crossterm` ni terminal; tampoco el servidor ni el
cliente de red de A2A (eso es `loomcoding-a2a`).

**Tipos clave:** `AgentLoop` (`agent/mod.rs`; sus métodos, en `agent/*.rs`), `trait Provider` (`provider/mod.rs`),
`trait Tool` y `ToolRegistry` (`tools/mod.rs`), `LiveEvent` y `EventBus` (`event/mod.rs`),
`trait SessionStore` y `SqliteStore` (`session/`), `SubagentPhase` (`subagent/phase.rs`),
`AgentsConfig` (`config/mod.rs`).

**Puntos de entrada:** `AgentLoop::run` y `run_message` (`agent/entry.rs`); el turno, en `agent/turn.rs`; la compactación, en `agent/compaction.rs`;
`dispatch::execute_tools` (`agent/dispatch.rs`); `project_registry` (`tools/mod.rs`, registra
las herramientas integradas); `subagent::watchdog::evaluate` (decisión pura de latidos); `subagent::supervisor::SubagentSupervisor` (salud por hijo, puro) y `supervisor_driver::spawn_supervisor` (reloj y E/S).

**Cómo se testea:** tests unitarios en el propio módulo (`#[cfg(test)]`) y `wiremock` para los
providers. Sin red real: los tests de `tests/*_live.rs` son los únicos que tocan un proveedor
y no corren por defecto.

**Trampas:**
- Las tools que escriben son `ExecutionMode::Exclusive`; las de lectura, `Parallel`.
- `registry/` es el registro de **agentes** (perfiles `.md`), no el de herramientas.
- Todo output, prompt o bucle lleva cota (decálogo de contexto).
- `SubagentPhase::can_transition_to` es la única tabla de transiciones: no la dupliques.

Mapa general: `docs/ARCHITECTURE.md`. Tareas habituales: `docs/CODE-MAP.md`.
