# Plan — Autonomía y límite de pasos

> **Para quien ejecute:** cada tarea es independiente y se ejecuta de una en una, con la prueba en rojo
> primero. La ejecuta Loomcoding con el Qwen3.5-9B local (ver «Ejecución»), y la verifica y revisa Claude.

**Objetivo:** que un agente que avanza no muera por contar rondas, que llegar al tope no destruya el trabajo,
que haya un techo de paralelismo configurable y una parada de emergencia.

**Arquitectura:** contadores separados de pasos de trabajo y de lectura en `agent/turn.rs`; ajustes en una
sección `[limits]` con cargador propio (patrón de `UiConfig`); envoltorio de `Provider` con semáforo y
retroceso; informe de traspaso del subagente; comando `KillAll` que cancela turno, jobs e hijos.

**Spec:** `docs/specs/2026-11-04-loomcoding-autonomia-y-limite-de-pasos.md`.

## Restricciones globales

- Los cuatro checks del repo en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`,
  `cargo clippy -- -D warnings`.
- `loomcoding-core` no depende de `ratatui` ni `crossterm`. Sin red real en tests.
- Ningún archivo supera 1000 líneas (guardarraíl `crates/loomcoding-e2e/tests/guardarrailes.rs`).
- **No se añaden campos a `LoomcodingConfig` ni a `ProviderConfig`** (se construyen en unos 100 sitios): los
  ajustes nuevos van en `LimitsConfig`, con su propio cargador.
- Valores por defecto: `max_parallel_tools = 8`, `max_concurrent_requests = 4`,
  `max_read_steps = 4 × max_steps`. Todo bucle nuevo lleva cota (decálogo de contexto y coste).
- Una tarea = un commit. Mensajes de commit en español pijo (convención del repo).

## Revisión: casos que ninguna tarea cubre sola

- `max_steps = 1` con un turno de solo lecturas: debe terminar por `max_read_steps`, no colgarse.
- Ronda con una lectura y una escritura a la vez: cuenta como trabajo, no como lectura.
- Subagente que llega al tope sin ningún texto del asistente: sigue fallando, con el motivo (T5).
- `max_concurrent_requests = 0` en el TOML: se trata como el valor por defecto, no como bloqueo eterno (T1).
- Cancelar con peticiones en cola del semáforo: las esperas se cancelan, no se quedan colgadas (T7).

## Estructura de archivos

| Archivo | Responsabilidad |
|---|---|
| `crates/loomcoding-core/src/config/limits.rs` (nuevo) | `LimitsConfig` y su cargador |
| `crates/loomcoding-core/src/agent/stop.rs` | clasificar rondas y los dos presupuestos |
| `crates/loomcoding-core/src/agent/turn.rs` | contadores y pregunta al tope |
| `crates/loomcoding-core/src/agent/handoff.rs` (nuevo) | informe de traspaso |
| `crates/loomcoding-core/src/agent/dispatch.rs` | tope de herramientas en paralelo |
| `crates/loomcoding-core/src/provider/limited.rs` (nuevo) | semáforo y retroceso ante 429 |
| `crates/loomcoding-core/src/agent/kill.rs` (nuevo) | parada de emergencia |

---

### Tarea 1: `LimitsConfig`

**Files:** Create `crates/loomcoding-core/src/config/limits.rs`; Modify `config/mod.rs` (`mod limits; pub use limits::LimitsConfig;`).

**Interfaces:**
- Produces: `pub struct LimitsConfig { pub max_read_steps: Option<usize>, pub max_parallel_tools: usize, pub max_concurrent_requests: usize }` con `Default` (`None`, `8`, `4`), `LimitsConfig::from_toml(&str) -> Result<LimitsConfig>`, `LimitsConfig::load(&Path) -> Result<LimitsConfig>` y `fn read_step_cap(&self, max_steps: usize) -> usize` (el `max_read_steps` o `4 * max_steps`, mínimo 1).
- Sección TOML `[limits]`; sin ella, todo por defecto. Un `0` en `max_parallel_tools` o `max_concurrent_requests` equivale al valor por defecto.

- [ ] Prueba `limits_default_when_section_missing`: `from_toml("model = \"x\"\n")` da `8`, `4`, `None`.
- [ ] Prueba `limits_read_values_from_section`: con `[limits]\nmax_parallel_tools = 2\nmax_concurrent_requests = 3\nmax_read_steps = 50` se leen tal cual.
- [ ] Prueba `zero_limits_fall_back_to_defaults`: `max_concurrent_requests = 0` da `4`; `max_parallel_tools = 0` da `8`.
- [ ] Prueba `read_step_cap_defaults_to_four_times_max_steps`: `read_step_cap(30) == 120`, `read_step_cap(0) == 1`, y con `Some(7)` da `7`.
- [ ] Ejecutar `cargo test -p loomcoding-core limits`: deben fallar (no compila). Implementar siguiendo `UiConfig::from_toml`/`load` de `config/mod.rs` (struct privado `FicheroConLimites { #[serde(default)] limits: LimitsConfig }`). Ejecutar de nuevo: pasan.
- [ ] Commit.

### Tarea 2: clasificar rondas y presupuestos en `stop.rs`

**Files:** Modify `crates/loomcoding-core/src/agent/stop.rs`.

**Interfaces:**
- Produces: `pub fn is_read_round(message: &Message) -> bool` (mensaje del asistente con **al menos una** llamada a herramienta y **ninguna** en `WRITE_TOOLS`); `StopReason::ReadBudget`; `pub fn stop_reason_budgets(history: &[Message], steps: usize, max_steps: usize, read_steps: usize, max_read_steps: usize) -> Option<StopReason>` (`ReadBudget` si `read_steps >= max_read_steps`; si no, lo que dé `stop_reason`). `stop_reason` y `should_stop` no cambian de firma.

- [ ] Pruebas: `a_round_of_only_reads_is_a_read_round` (`view` + `grep`), `a_round_with_a_write_is_not_a_read_round` (`view` + `edit`), `a_round_with_bash_is_not_a_read_round`, `a_message_without_tool_calls_is_not_a_read_round`, `read_budget_stops_at_the_cap` (`stop_reason_budgets(&[], 0, 30, 5, 5) == Some(ReadBudget)`), `work_budget_still_wins_first` (`steps == max_steps` da `StepBudget`).
- [ ] Ejecutar `cargo test -p loomcoding-core stop`: fallan. Implementar. Pasan. Commit.

### Tarea 3: contadores de pasos en el turno

**Files:** Modify `crates/loomcoding-core/src/agent/turn.rs`, `crates/loomcoding-core/src/agent/mod.rs` (campo `limits: LimitsConfig` y `pub fn with_limits(mut self, limits: LimitsConfig) -> Self`, por defecto `LimitsConfig::default()`); Test: `agent/tests/turn.rs`.

**Interfaces:**
- Consumes: T1 (`LimitsConfig::read_step_cap`), T2 (`is_read_round`, `stop_reason_budgets`).
- Produces: `AgentLoop::with_limits`. En el bucle del turno existe `read_steps: usize`; al reunir las llamadas de la ronda, si `is_read_round` del mensaje del asistente es cierto, `steps -= 1` y `read_steps += 1`; la comprobación de parada usa `stop_reason_budgets` con `config.max_steps` y `limits.read_step_cap(config.max_steps)`. `StepStarted` sigue publicando `step` y `max_steps`.

- [ ] Prueba `read_only_rounds_do_not_spend_the_work_budget`: `max_steps = 2`, guion del proveedor con 5 rondas de `view` y una respuesta final de texto; el turno termina con el texto final (antes se cortaba).
- [ ] Prueba `read_rounds_are_capped_by_max_read_steps`: `with_limits(max_read_steps = Some(3))`, guion con 10 rondas de `view`; el turno se detiene tras 3 lecturas sin texto final.
- [ ] Prueba `a_write_round_spends_the_work_budget`: `max_steps = 1`, guion con una ronda `edit`, luego texto; el turno se detiene tras la escritura.
- [ ] Ejecutar `cargo test -p loomcoding-core agent::tests::turn`: fallan. Implementar. Pasan. Commit.

### Tarea 4: pregunta al llegar al tope (agente principal)

**Files:** Modify `crates/loomcoding-core/src/agent/turn.rs`; Test: `agent/tests/turn.rs`.

**Interfaces:**
- Consumes: T3. `ctx.permission.ask_user(&[Question]) -> Option<Vec<Answer>>` (ejemplo: `tools/gauntlet/decide.rs`).
- Produces: al darse `StopReason::StepBudget` o `ReadBudget` en el **agente principal** (no en un hijo), se publica `RuntimeNotice` con el motivo y se pregunta con la cabecera `"Step limit"` y las opciones `"Continue"` y `"Stop"`. Con `"Continue"` el tope de ese contador sube en `config.max_steps` y el bucle sigue; como máximo `MAX_STEP_EXTENSIONS = 3` veces por turno. Sin respuesta, con `"Stop"` o con cancelación, se para como hoy.

- [ ] Pruebas: `step_limit_asks_and_continues_when_the_person_agrees` (responder `Continue` completa el turno con el texto final), `step_limit_stops_when_the_person_declines`, `step_limit_stops_without_anyone_to_ask`, `step_limit_extensions_are_capped` (responder siempre `Continue` con un guion infinito de escrituras termina tras 3 prórrogas).
- [ ] Ejecutar: fallan. Implementar. Pasan. Commit.

### Tarea 5: informe de traspaso del subagente

**Files:** Create `crates/loomcoding-core/src/agent/handoff.rs` (declarar en `agent/mod.rs`); Modify `crates/loomcoding-core/src/subagent/local_body.rs`; Test: `subagent/tests/ciclo_de_vida.rs`.

**Interfaces:**
- Produces: `pub const MAX_HANDOFF_BYTES: usize = 4096;` y `impl AgentLoop { pub async fn handoff_report(&self, session: SessionId, cancel: &CancellationToken) -> Result<String> }`: una única petición al proveedor con el historial reciente, **sin herramientas**, `max_tokens = 1024`, sistema fijo (qué se hizo y qué falta), salida truncada a `MAX_HANDOFF_BYTES` en frontera de carácter.
- En `local_body.rs`, cuando el texto final está vacío **y** `steps >= child_max_steps`, antes del `Err` actual: si `handoff_report` da texto no vacío, el job se completa con ese texto precedido de `[PARTIAL: step limit {steps}/{child_max_steps}]`. Si falla o viene vacío, se usa el último texto del asistente del historial; si tampoco hay, sigue el `Err` actual con su mensaje.

- [ ] Pruebas: `handoff_report_is_bounded` (proveedor que devuelve 10 000 bytes: el resultado ≤ `MAX_HANDOFF_BYTES`), `handoff_request_offers_no_tools`, `a_subagent_at_the_step_limit_completes_with_a_partial_report` (estado `completed`, artefacto con el prefijo `[PARTIAL`), `a_subagent_at_the_limit_without_any_text_still_fails_with_the_reason`.
- [ ] Ejecutar: fallan. Implementar. Pasan. Commit.

### Tarea 6: tope de herramientas en paralelo

**Files:** Modify `crates/loomcoding-core/src/agent/dispatch.rs` (y sus llamadores en `agent/turn.rs`).

**Interfaces:**
- Consumes: T1.
- Produces: `execute_tools` recibe un parámetro más, `max_parallel: usize`, antes de `bus`; sustituye `buffer_unordered(8)` por `buffer_unordered(max_parallel.max(1))`. El turno pasa `self.limits.max_parallel_tools`. Las llamadas de las pruebas existentes pasan `8`.

- [ ] Prueba `parallel_batch_never_exceeds_the_cap`: herramienta `Parallel` de prueba que cuenta cuántas se ejecutan a la vez con un contador atómico; 6 llamadas y `max_parallel = 2` dan un máximo observado de 2 y las 6 resultados.
- [ ] Ejecutar: falla (no compila). Implementar. Pasan todas las de `dispatch`. Commit.

### Tarea 7: envoltorio de proveedor con semáforo y retroceso ante 429

**Files:** Create `crates/loomcoding-core/src/provider/limited.rs` (declarar en `provider/mod.rs`); Modify `crates/loomcoding-cli/src/server/agent_build.rs` (envolver el proveedor construido).

**Interfaces:**
- Consumes: T1 (`max_concurrent_requests`). `trait Provider`, `ProviderStream` de `provider/mod.rs`.
- Produces: `pub struct LimitedProvider { .. }` con `pub fn new(inner: Arc<dyn Provider>, max_concurrent: usize) -> Self` que implementa `Provider`: `capabilities` delega; `stream` espera un permiso del semáforo (cancelable con `cancel`), llama al interior y devuelve un flujo que **conserva el permiso hasta que termina o se suelta**. Si el interior responde `CoreError::Provider` con el texto `http 429` (en minúsculas, como `is_context_overflow`), reintenta hasta `MAX_RATE_LIMIT_RETRIES = 3` con espera `500 ms × 2^intento`, cancelable, y publica nada (el aviso en pantalla llega por el `tracing::warn!`).

- [ ] Pruebas: `never_more_than_the_limit_run_at_once` (proveedor de prueba con contador, 6 peticiones simultáneas, límite 2: máximo observado 2), `the_permit_is_held_until_the_stream_ends`, `a_429_is_retried_then_succeeds`, `a_429_gives_up_after_the_retry_cap`, `a_cancelled_waiter_does_not_hang` (límite 1, la segunda petición se cancela mientras espera y devuelve `CoreError::Cancelled`).
- [ ] Ejecutar: fallan. Implementar. Pasan. En `agent_build.rs`, envolver con `LimitedProvider::new(provider, limits.max_concurrent_requests)` leyendo `LimitsConfig::load` (mismo estilo que `ProviderCacheConfig::load` en ese archivo). `cargo test -p loomcoding-cli`. Commit.

### Tarea 8: guía en el prompt de sistema

**Files:** Modify el módulo que construye el prompt de sistema (buscar con `grep -rn "fn build_system" crates/loomcoding-core/src/agent`); Test: `agent/tests/system_prompt.rs`.

**Interfaces:** una sección acotada (≤ 400 bytes) con dos frases: agrupar en una sola respuesta las lecturas independientes, y delegar en un subagente especialista cuando haya uno disponible. Va solo si el agente puede delegar (`delegation` no es `off`).

- [ ] Pruebas: `prompt_tells_the_model_to_batch_independent_reads`, `delegation_hint_is_absent_when_delegation_is_off`, `parallel_guidance_stays_under_its_cap`.
- [ ] Ejecutar: fallan. Implementar. Pasan. Commit.

### Tarea 9: modo Dios (`Ctrl+X`)

**Files:** Create `crates/loomcoding-core/src/agent/kill.rs`; Modify `crates/loomcoding-tui/src/run/keys.rs`, `crates/loomcoding-tui/src/run/mod.rs` (`SessionCommand::KillAll`), `crates/loomcoding-tui/src/app/mod.rs` (`kill_all_requested: bool`), `crates/loomcoding-cli/src/server/worker.rs`; Test: `agent/tests/` y `crates/loomcoding-tui/src/app/tests/`.

**Interfaces:**
- Produces: `impl AgentLoop { pub async fn kill_everything(&self, session: SessionId) -> KillReport }` con `KillReport { children: usize, jobs: usize }`: cancela los hijos no terminales (`subagents.list` + `cancel`) y los jobs en marcha de la sesión (API de cancelación de `LocalJobRegistry`); el worker además cancela el token del turno activo (`ActiveTurn`). `Ctrl+X` fuera del panel de la cola activa `app.kill_all_requested`; dentro del panel se conserva el comportamiento actual. Publica `RuntimeNotice` con «emergency stop: N children, M jobs».
- `bash` ya usa `kill_on_drop`; la prueba de abajo comprueba que, al soltarse el futuro, el proceso muere.

- [ ] Pruebas: `kill_everything_cancels_running_children_and_jobs`; `ctrl_x_outside_the_queue_requests_a_kill_all`; `ctrl_x_inside_the_queue_keeps_pausing_the_queue`; `cancelling_a_bash_call_leaves_no_process_alive` (lanzar un `bash` con `sleep 30`, cancelar, comprobar que el proceso ya no existe en menos de 2 s).
- [ ] Ejecutar: fallan. Implementar. Pasan. `cargo test -p loomcoding-cli`. Commit.

### Tarea 10: documentación

**Files:** `docs/ARCHITECTURE.md` (fila de límites), `docs/CODE-MAP.md` (sección «Quiero tocar los límites del agente»), `crates/loomcoding-core/README.md`, `docs/INDEX.md` (estado del spec y de este plan).

- [ ] Rellenar cada archivo con las rutas reales de T1 a T9 y ejecutar `cargo test -p loomcoding-e2e --test guardarrailes`. Commit.

---

## Ejecución con Loomcoding

- **Dónde:** un worktree aparte (`G:\temp\loom-pilot`) con `LOOMCODING_HOME` aislado y `CARGO_TARGET_DIR` propio; nunca el árbol principal. Servidor: `G:\temp\loom-qwen\lanzar-llama.ps1`; lanzador: `lanzar-piloto.ps1`.
- **Cómo:** cada tarea se entrega como un fichero `G:\temp\loom-qwen\tarea-N.md` (formato de `tarea-piloto.md`: un único objetivo, pruebas primero, comandos a ejecutar y resultado literal). El Gauntlet se usa cuando la barra es inspeccionable (por ejemplo, las pruebas ya escritas y un fichero de referencia); si no, ejecución directa.
- **Qué mide Claude por tarea:** pasos gastados y de lectura (`StepStarted`), tiempo, si llegó al tope, si las pruebas pasan **en el árbol limpio** (no solo lo que el agente afirma), y la calidad del diff. Lo traslada al `INDEX` como informe y a la conversación como opinión.
- **Aviso:** estas tareas se ejecutan con el código **actual**, que aún corta al llegar a `max_steps`. Se espera que el 9B tropiece con ese tope en varias; es parte de la medición, y las tareas T2 a T4 lo arreglan.
