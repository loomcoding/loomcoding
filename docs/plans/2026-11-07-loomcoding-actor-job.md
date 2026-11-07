# El actor «job» en el transcript — plan

> **Para quien ejecute:** una tarea cada vez; prueba que falla (rojo visible), código mínimo, los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`), un commit por
> tarea. Cargo desde PowerShell. Rama `actor-job`.

**Spec:** `docs/specs/2026-11-07-loomcoding-actor-job.md`.

## Restricciones globales

- Núcleo sin `ratatui`; ficheros nuevos ≤ 400 líneas.
- **A2A se respeta al máximo:** el job no es un agente A2A; ningún estado A2A nuevo ni tocado; el origen no llega al proveedor
  ni a mensajes A2A.
- El texto que ve el modelo no cambia.
- Sin red real en las pruebas. Binario real solo con `LOOMCODING_HOME` temporal.

## Tarea 1 — El origen del mensaje lleva el job

**Ficheros:** `crates/loomcoding-core/src/message/mod.rs` (`MessageOrigin::Job(String)`; quita `Copy`; `#[serde(rename_all =
"snake_case")]` se conserva), `crates/loomcoding-core/src/agent/subagents.rs` (`flush_subagent_notices`: el aviso de
vigilancia y el de job terminado crean el mensaje con `origin = MessageOrigin::Job(job.id.0.clone())`), y lo que obligue a
corregir la pérdida de `Copy`.
- [ ] Pruebas: serde ida y vuelta de `Job("3")`; un mensaje antiguo sin la variante (`"origin":"internal"` y sin campo) se
  lee igual que antes; el aviso de vigilancia y el de job terminado salen con `Job(id)`; el mensaje que llega al proveedor
  (conversión a petición) no incluye el origen y su rol sigue siendo `user`; ninguna conversión A2A lo incluye; el almacén
  de sesiones guarda y relee el origen.

## Tarea 2 — `cloth`: tipo de línea del actor job

**Ficheros:** `crates/loomcoding-cloth/src/state/transcript.rs` (`LineKind::Job`; `StyleToken::Job` si no basta con uno
existente), `widgets/transcript/lines.rs` (prefijo y token), el trazo de los seis carriles (`tree`, `dots`, `bar`, `box`,
`tint`, `none`) y todos los `match` que enumeran `LineKind` (el compilador los señala).
- Prefijo propuesto `job: ` (sin chocar con `⚙` de herramientas ni `◆` de actividad); token que reutilice un color del tema.
- [ ] Pruebas: la línea se pinta con su prefijo en cada estilo de carril; por debajo de 40 columnas no se reserva carril
  como hasta ahora; copiar o exportar el transcript conserva el prefijo; los temas con y sin color no se rompen.

## Tarea 3 — La TUI pinta el actor, en vivo y al reabrir

**Ficheros:** `crates/loomcoding-tui/src/app/events.rs` (`apply_message_added`, `SessionMessageAdded`), `app/session.rs`
(reapertura), `app/activity.rs` (`internal_user_message`: el origen `Job` va a `LineKind::Job`; `parse_notice` se conserva
para mensajes antiguos), `crates/loomcoding-cli/tests/tui_pty_vigilancia.rs` y `docs/mockups/vigilancia-jobs-e2e.md`
(el volcado real pasa de `sys: [Job …]` al actor job).
- [ ] Pruebas: un aviso de vigilancia en vivo sale como actor job; un aviso de job terminado también; reabrir la sesión los
  pinta igual; un mensaje antiguo con origen `Internal` y texto `[Job N …]` se sigue reconociendo; un aviso de subagente o
  de ODD sigue saliendo como `sys:`; con el modo `tool_inline` el aviso sigue funcionando; el volcado e2e se regenera con
  `LOOMCODING_HOME` temporal.
- Documentación: `docs/CODE-MAP.md` (sección de jobs), `CHANGELOG.md` (incluida la nota de compatibilidad de sesiones),
  `docs/INDEX.md`.
