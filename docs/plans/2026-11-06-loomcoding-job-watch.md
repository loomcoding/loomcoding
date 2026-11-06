# Vigilancia de jobs (`watch`) — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla, código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`), un commit por tarea.

**Spec:** `docs/specs/2026-11-06-loomcoding-job-watch.md`. Trabajo en una rama propia.

> Estado: **hecho** (T1–T5). La observabilidad de la TUI, el ajuste de `WRITE_TOOLS`, la republicación del recuento
> de un job ya terminado y el E2E PTY (`crates/loomcoding-cli/tests/tui_pty_vigilancia.rs`) cierran la tarea 5.

## Restricciones globales

- Reutilizar `JobRegistry`, `JobHandle::append`, `track_job`, `flush_subagent_notices` y el bus (`LiveEvent`). No
  crear otro canal de avisos ni dependencias nuevas (`regex` ya está en el core).
- `job.rs` tiene 463 líneas: no crece; la lógica nueva va en `crates/loomcoding-core/src/job_watch.rs`.
- Todo acotado según la spec; tiempos con `tokio::time::pause`; sin red real.

## Revisión (casos que ninguna tarea cubre por sí sola)

- Patrón inválido: `job_start` falla antes de arrancar el comando.
- Salida sin saltos de línea de más de 4 KiB: se corta sin pánico ni crecimiento sin límite.
- Salida con bytes UTF-8 partidos entre dos lecturas: no se pierde ni se duplica ninguna línea.
- Job que termina con un resto sin salto: el resto se prueba contra el patrón al cerrar.
- Dos jobs vigilados a la vez: sus avisos no se mezclan y cada uno respeta su propio tope.
- `kill` de un job vigilado: no quedan tareas de vigilancia vivas.

## Tarea 1 — Troceo, emparejado y cola de eventos

**Ficheros:** nuevo `crates/loomcoding-core/src/job_watch.rs` (`WatchSpec { pattern: Regex, max_events: usize }`
con `WatchSpec::parse(pattern, max_events) -> Result<WatchSpec, CoreError>`; `LineMatcher::push(&mut self, chunk:
&str) -> Vec<String>` que devuelve las líneas que casan, y `finish(&mut self) -> Vec<String>`; `EventQueue` con
`push`, `drain_batch(now) -> Option<WatchBatch { lines, suppressed }>`), `lib.rs`.
- [ ] Pruebas: líneas completas y troceadas en varias lecturas; resto sin salto >4 KiB; UTF-8 partido; patrón
  inválido o de más de 256 bytes; `max_events` por defecto 20 y tope 100; agrupación de 10 líneas con «+N
  suprimidas»; un lote cada 5 s con reloj simulado.

## Tarea 2 — `job_start` con `watch` y registro en el job

**Ficheros:** `tools/job_start.rs` (esquema y descripción con `watch`; el cuerpo del job alimenta el `LineMatcher`
a la vez que `handle.append`), `job.rs` (`JobSnapshot.watch: Option<WatchInfo>`, y el hueco para la cola de
eventos del dueño), `registry/snapshot.rs` si serializa jobs.
- [ ] Pruebas: patrón válido arranca y registra `WatchInfo`; patrón inválido no arranca el comando; sin `watch`
  todo igual que hoy; `kill` cancela la vigilancia; `job_list` muestra la vigilancia.

## Tarea 2b — `job_watch` sobre un job en marcha

**Ficheros:** nuevo `crates/loomcoding-core/src/tools/job_watch.rs` (`JobWatchTool`, nombre `job_watch`, esquema
`{job_id, pattern?, max_events?}`, `ExecutionMode::Exclusive`), registro de la herramienta junto a `job_start`, y en
`job.rs` el método para sustituir o quitar la vigilancia de un job vivo (la trait `JobRegistry` gana
`set_watch(owner, id, Option<WatchSpec>) -> Result<()>`).
- [ ] Pruebas: añadir vigilancia a un job en marcha; solo ve líneas posteriores; cambiar el patrón; quitarla con
  patrón vacío; job terminado o de otra sesión → error; patrón inválido → error sin cambiar nada.

## Tarea 3 — Entrega como mensaje de la conversación

**Ficheros:** `agent/subagents.rs` (`flush_subagent_notices` entrega los lotes pendientes como
`[Job N watch] …` con `limit_bytes(MAX_NOTICE_BYTES)`), `event/mod.rs` si hace falta un `LiveEvent` propio para
rotular el actor «job».
- [ ] Pruebas: un lote se entrega una sola vez; queda persistido y publicado; el aviso final de fin de job sigue
  llegando; agotar `max_events` añade «watch limit reached».

## Tarea 4 — Despertar del padre

**Ficheros:** `crates/loomcoding-tui/src/app/turn.rs` (`take_wake`: nueva causa), `app/child_types.rs`
(presupuesto compartido `MAX_AUTO_TURNS`), `crates/loomcoding-core/src/agent/settle.rs` (`MAX_WAKE_TURNS` sin TUI).
- [ ] Pruebas: un aviso con el padre ocioso despierta; con el presupuesto agotado no despierta pero se entrega
  en el siguiente turno; sin TUI (`run_until_settled`) respeta `MAX_WAKE_TURNS`.

## Tarea 5 — Observabilidad, documentación y comprobación

La tarjeta del agente y el panel de jobs muestran «vigilado: N avisos». `docs/INDEX.md`, `docs/CODE-MAP.md`
(sección de jobs), CHANGELOG; volcado e2e con perfil aislado (`docs/visual-e2e.md`): un job que imprime una línea
que casa despierta al modelo mock y el aviso sale en el transcript con el actor «job».
