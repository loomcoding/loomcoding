# Sacar los flujos de orquestación del núcleo — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que `loomcoding-core` no nombre ningún flujo concreto y que el Gauntlet viva en el crate `loomcoding-workflows`, sin cambiar nada de lo que el usuario ve.

**Architecture:** el núcleo ofrece tres puntos de extensión genéricos (almacén de flujos, evento `FlowChanged`, `Tool::is_orchestrator`); el Gauntlet se reescribe sobre ellos dentro del núcleo, paso a paso, y solo cuando ya no tiene dependencias específicas se mueve al crate nuevo. Un guardarraíl de tamaño endurecido a 400 líneas congela los ficheros gordos.

**Tech Stack:** Rust, `rusqlite`/`sqlx` ya presentes en el núcleo, `serde_json`, `tokio`.

**Spec:** `docs/specs/2026-11-05-loomcoding-extraer-flujos-del-core.md`.

## Global Constraints

- Cero cambios de comportamiento observable: mismas fases, mismos textos en pantalla, mismos datos.
- Los cuatro chequeos en cada commit: `cargo build`, `cargo test --no-fail-fast`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`. El número total de pruebas que pasan no baja de 2 415 (la base del 2026-11-05).
- `loomcoding-core` NO depende de `ratatui`, de `crossterm` ni de `loomcoding-workflows`.
- Errores de biblioteca con `thiserror`. Sin red real en pruebas (proveedor simulado).
- Todo fichero nuevo o movido: como mucho **400 líneas** (excluidas las pruebas).
- Comentarios en español pijo; identificadores y mensajes de error sin tocar. Respeta el final de línea (CRLF o LF) de cada fichero que edites.
- Un commit por tarea, con `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Orden estricto: 0 y 1 son independientes; después 2 → 3 → 4 → 5 → 6 → 7 → 8.

## Review Focus

- Base de datos antigua con filas en `gauntlets`: se copian una vez a `flows` y no se duplican al reabrir (Tarea 3).
- Dos sesiones con un flujo activo cada una, y un segundo `start` en la misma sesión: se rechaza igual que hoy (Tareas 3 y 4).
- `update_flow` con una `version` obsoleta falla y no pisa al ganador (Tarea 3).
- El texto que la TUI pinta para un flujo es idéntico antes y después (Tarea 5).
- Un subagente nunca recibe un orquestador aunque su perfil lo nombre (Tarea 6).

## Tareas

### Task 0: Endurecer el guardarraíl de tamaño (de 1 000 a 400 líneas de código)

**Files:** modificar `crates/loomcoding-e2e/tests/guardarrailes.rs`; modificar `docs/INVARIANTS.md` (una fila nueva en la tabla).

**Interfaces — Produces:** `const TECHO_CODIGO: usize = 400` y `const TECHO_PRUEBAS: usize = 1000` (sustituyen a `TECHO`); `fn es_de_pruebas(ruta: &str) -> bool` (verdadero si algún componente de la ruta es `tests`, o el fichero se llama `tests.rs` o termina en `_tests.rs`, o está bajo `examples/`); `LISTA_BLANCA` con una entrada `(ruta, tope, justificación)` por cada fichero de código que hoy pasa de 400. El tope es el que ya define la regla del propio fichero (tamaño actual redondeado a la centena superior).

- [ ] **Step 1: pruebas primero:** en el mismo fichero, `un_fichero_de_codigo_de_401_lineas_no_listado_falla`, `un_fichero_de_pruebas_de_900_lineas_pasa`, `es_de_pruebas_reconoce_las_cuatro_formas` (ejemplos de ruta que valen y que no valen). Refactoriza la comprobación en una función pura `fallos(lineas: &BTreeMap<String, usize>, lista: &[(&str, usize, &str)]) -> Vec<String>` para poder probarla sin tocar el disco.
- [ ] **Step 2:** aplica los dos techos según `es_de_pruebas`; genera la lista blanca inicial con una prueba `#[ignore]` `imprime_la_lista_blanca` (se ejecuta con `-- --ignored --nocapture`) y pégala en `LISTA_BLANCA` con la justificación «previo al plan de flujos 2026-11-05».
- [ ] **Step 3:** añade a `docs/INVARIANTS.md` la fila «Ningún fichero de código nuevo pasa de 400 líneas» con su comprobación (`cargo test -p loomcoding-e2e --test guardarrailes`). Los cuatro chequeos; commit.

### Task 1: Partir `gauntlet.rs` en un módulo de carpeta (mover, no cambiar)

**Files:** borrar `crates/loomcoding-core/src/gauntlet.rs`; crear `gauntlet/mod.rs`, `phase.rs`, `order.rs`, `verdict.rs`, `model.rs`, `reading.rs`, `tests.rs` en `crates/loomcoding-core/src/gauntlet/`.

**Interfaces — Produces:** exactamente los mismos nombres públicos de hoy bajo `crate::gauntlet::*` (los reexporta `mod.rs`); ningún `use` de fuera cambia.

Reparto de lo que hoy hay en el fichero: `phase.rs` = `GauntletId`, `GauntletPhase` (y su `FromStr`), `DEFAULT_MAX_ROUNDS`, `MAX_ROUNDS_LIMIT`; `order.rs` = `AbOrder`; `verdict.rs` = `Verdict`, `MAX_GAP_BYTES`, `MAX_REPORT_BYTES`, `parse_verdict` y sus ayudas de corte; `model.rs` = `Next`, `RoundRecord`, `GauntletSummary`, `GauntletEvent`, `GauntletProblem`, `NewGauntlet`, `Gauntlet` y su `impl`; `reading.rs` = `unread_candidates`; `tests.rs` = el módulo `tests` entero.

- [ ] **Step 1:** anota cuántas pruebas pasan con `cargo test -p loomcoding-core --lib gauntlet` (el número debe ser idéntico al final).
- [ ] **Step 2:** mueve cada trozo sin tocar cuerpos ni firmas; solo cambian los `use` y la visibilidad mínima (`pub(super)`) que exija el reparto.
- [ ] **Step 3:** retira `gauntlet.rs` de `LISTA_BLANCA` y actualiza su ruta en `docs/INVARIANTS.md` (fila de `GauntletPhase`) y en `docs/CODE-MAP.md`; los cuatro chequeos; el recuento de pruebas del Step 1 no cambia; ningún fichero nuevo pasa de 400 líneas (el guardarraíl lo comprueba); commit.

### Task 2: Tipos genéricos de flujo y métodos del almacén

**Files:** crear `crates/loomcoding-core/src/flow.rs`; declarar `pub mod flow;` en `lib.rs`; modificar `session/mod.rs` (métodos con implementación por defecto).

**Interfaces — Produces:**
- `pub struct FlowId(pub i64)` (con `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`).
- `pub struct FlowSummary { pub kind: String, pub id: FlowId, pub phase: String, pub round: u8, pub max_rounds: u8, pub terminal: bool, pub label: String }`.
- `pub struct FlowRecord { pub id: FlowId, pub kind: String, pub parent: SessionId, pub phase: String, pub terminal: bool, pub version: u32, pub summary: FlowSummary, pub payload: serde_json::Value, pub created_at_ms: i64, pub updated_at_ms: i64 }`.
- `pub struct FlowEventRecord { pub id: i64, pub flow: FlowId, pub from_phase: String, pub to_phase: String, pub actor: String, pub detail: String, pub created_at_ms: i64 }`.
- En `SessionStore` (cada uno con implementación por defecto que devuelve el error «flow storage unavailable», como hacen hoy los del Gauntlet): `create_flow(&self, record: FlowRecord, detail: &str) -> Result<FlowRecord>`, `update_flow(&self, record: FlowRecord, actor: &str, detail: &str) -> Result<FlowRecord>`, `flow(&self, id: FlowId) -> Result<Option<FlowRecord>>`, `active_flow(&self, parent: SessionId, kind: Option<&str>) -> Result<Option<FlowRecord>>`, `latest_flow(&self, parent: SessionId, kind: Option<&str>) -> Result<Option<FlowRecord>>`, `flow_events(&self, id: FlowId) -> Result<Vec<FlowEventRecord>>`.

- [ ] **Step 1: pruebas primero** en `flow.rs`: `a_flow_record_round_trips_through_json`, `a_flow_summary_round_trips_through_json`.
- [ ] **Step 2:** define los tipos y los seis métodos; los cuatro chequeos; commit.

### Task 3: Tablas `flows` y `flow_events` en SQLite, con migración

**Files:** modificar `session/sqlite/schema.rs`; crear `session/sqlite/flows.rs` (se generaliza `session/sqlite/gauntlets.rs`, que NO se toca aún); modificar `session/sqlite/mod.rs` (implementa los seis métodos); pruebas en `session/sqlite/tests/flows.rs`.

**Interfaces — Consumes:** los tipos y métodos de la Tarea 2. **Produces:** tablas `flows(id, kind, parent_session_id, phase, terminal, version, summary, payload, created_at_ms, updated_at_ms)` y `flow_events(id, flow_id, from_phase, to_phase, actor, detail, created_at_ms)`; índice único `one_active_flow_per_session_and_kind` sobre `flows(parent_session_id, kind) WHERE terminal = 0`; `fn migrate_gauntlets_into_flows(...)` que copia una sola vez las filas y eventos de `gauntlets`/`gauntlet_events` con `kind = 'gauntlet'`.

- [ ] **Step 1: pruebas primero:** `create_then_read_a_flow`, `update_with_a_stale_version_fails`, `only_one_active_flow_per_session_and_kind`, `a_terminal_flow_frees_the_slot`, `flow_events_come_back_in_order`, `two_sessions_can_each_have_an_active_flow`, `old_gauntlet_rows_are_copied_once` (base creada con el esquema antiguo, se abre, se comprueba la fila y los eventos, se reabre y no hay duplicados).
- [ ] **Step 2:** implementa tomando `gauntlets.rs` como modelo del control de versión y de la unicidad; los cuatro chequeos; commit.

### Task 4: El Gauntlet guarda por el almacén genérico

**Files:** crear `crates/loomcoding-core/src/tools/gauntlet/store.rs`; modificar `tools/gauntlet/{helpers,start,advance,decide,status}.rs`, `agent/dispatch.rs` (`active_gauntlet` pasa a `active_flow(session, Some("gauntlet"))`), `loomcoding-cli/src/server/snapshot.rs`; borrar `session/sqlite/gauntlets.rs` y los seis métodos `*_gauntlet` de `SessionStore` y de `SqliteStore`.

**Interfaces — Produces:** en `store.rs`, `pub(super) fn to_record(g: &Gauntlet, ...) -> FlowRecord` y `pub(super) fn from_record(r: FlowRecord) -> Result<Gauntlet>` (`payload` = el `Gauntlet` serializado; `phase = phase.as_str()`; `terminal = phase.is_terminal()`; `kind = "gauntlet"`; `summary` calculado como hoy).

- [ ] **Step 1:** las pruebas existentes del Gauntlet son la red de seguridad; añade `a_gauntlet_survives_a_store_round_trip` (guardar y leer por `to_record`/`from_record` da un `Gauntlet` igual).
- [ ] **Step 2:** cambia la persistencia; nada de la lógica de fases se toca. Se conservan las tablas viejas en el esquema (origen de la migración).
- [ ] **Step 3:** `grep -rn "create_gauntlet\|update_gauntlet\|latest_gauntlet\|active_gauntlet\|gauntlet_events" crates` no devuelve fuentes; los cuatro chequeos; commit.

### Task 5: Evento genérico `FlowChanged`

**Files:** `event/mod.rs` (sustituye `GauntletChanged { summary, actor, detail }` por `FlowChanged { summary: FlowSummary, actor: String, detail: String }`); `gauntlet/model.rs` (método `GauntletSummary::flow_summary(&self) -> FlowSummary` cuyo `label` es **exactamente** el texto que hoy compone la TUI, `gauntlet #N · fase · ronda a/b`); `tools/gauntlet/helpers.rs` (publica el nuevo evento); `loomcoding-tui/src/app/events.rs` y `run/mod.rs` (pintan desde `FlowSummary`; `SessionUpdate::Gauntlet` pasa a `SessionUpdate::Flow`); `loomcoding-cli/src/server/snapshot.rs` (usa `FlowRecord.summary`); pruebas de la TUI que mencionan el evento.

- [ ] **Step 1: prueba primero:** `the_flow_label_matches_what_the_tui_showed_before` (fases representativas, texto idéntico al de las pruebas actuales de la TUI).
- [ ] **Step 2:** si la TUI distingue algo más que `terminal` y `label` (por ejemplo, que espera una decisión), añade a `FlowSummary` solo los campos genéricos imprescindibles y actualiza la spec.
- [ ] **Step 3:** adapta las pruebas de la TUI sin cambiar lo que afirman sobre la pantalla; los cuatro chequeos; commit.

### Task 6: Orquestadores en lugar de listas de nombres

**Files:** `tools/mod.rs` (`Tool::is_orchestrator(&self) -> bool` por defecto `false`; `ToolRegistry::without_orchestrators(&self) -> ToolRegistry`); `tools/gauntlet/mod.rs` y `tools/development_workflow/mod.rs` (devuelven `true`); `agent/dispatch.rs`, `agent/turn.rs`, `agent/stop.rs`, `agent/mod.rs`, `subagent/launcher.rs` (cada lista de nombres con `"gauntlet"` o `"development_workflow"` pasa a consultar `is_orchestrator`).

- [ ] **Step 1: pruebas primero:** `an_orchestrator_is_not_offered_to_subagents`, `parent_writes_are_blocked_while_any_flow_is_active`, `a_start_by_an_orchestrator_in_the_same_batch_blocks_writes`.
- [ ] **Step 2:** sustituye cada lista; el flujo de desarrollo sigue consultando su propio almacén (se migra en su plan).
- [ ] **Step 3:** `grep -rn '"gauntlet"' crates/loomcoding-core/src --include=*.rs` solo devuelve el `name()` de la propia herramienta y pruebas; los cuatro chequeos; commit.

### Task 7: Crear `loomcoding-workflows` y mover el Gauntlet

**Files:** `Cargo.toml` (miembro nuevo); crear `crates/loomcoding-workflows/{Cargo.toml,src/lib.rs}`; mover `crates/loomcoding-core/src/gauntlet/` y `tools/gauntlet/` (con sus pruebas) a `crates/loomcoding-workflows/src/`; mover `subagent/tests/gauntlet_e2e.rs` y `tests/gauntlet_local_live.rs`; quitar `pub mod gauntlet;` de `lib.rs` y el registro de `project_registry` en el núcleo; modificar `loomcoding-cli` (depende del crate nuevo y lo registra al arrancar).

**Interfaces — Produces:** `pub fn register(tools: &mut ToolRegistry)` en `loomcoding-workflows`, que registra `GauntletTool`. El crate depende de `loomcoding-core`; el núcleo no depende de él.

- [ ] **Step 1:** esqueleto del crate; compila vacío con la dependencia del núcleo.
- [ ] **Step 2:** mueve reglas, herramienta y pruebas juntas (las reglas y la herramienta se necesitan entre sí, no se pueden separar); haz `pub` solo lo que el compilador pida del núcleo (lanzador de revisión de solo lectura, ayudas de sesión), anotándolo en el mensaje del commit.
- [ ] **Step 3:** actualiza la cuenta de herramientas del registro del proyecto en las pruebas del núcleo y comprueba que el binario sigue ofreciendo `gauntlet`.
- [ ] **Step 4:** `grep -ril gauntlet crates/loomcoding-core crates/loomcoding-tui` no devuelve fuentes; actualiza las rutas movidas en `docs/INVARIANTS.md` y `docs/CODE-MAP.md` y las entradas de `LISTA_BLANCA` que cambien de ruta; el número de pruebas del Gauntlet no baja; los cuatro chequeos; commit.

### Task 8: Documentación y cierre

**Files:** `docs/ARCHITECTURE.md`, `docs/CODE-MAP.md` (sección «Quiero tocar el Gauntlet» con las rutas nuevas), `docs/INVARIANTS.md`, `docs/INDEX.md`, `CHANGELOG`, estado de la spec.

- [ ] **Step 1:** actualiza los documentos con los nombres exactos del código.
- [ ] **Step 2:** comprueba uno por uno los criterios 1 a 6 de la spec con sus comandos y anótalos en la spec.
- [ ] **Step 3:** cambia el estado de la spec y del plan a «hecho»; commit.
