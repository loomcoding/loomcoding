# Ajustes del modelo local — plan

Spec: `docs/specs/2026-11-05-loomcoding-ajustes-modelo-local.md`. Cada tarea: prueba que falla, código mínimo,
los cuatro chequeos, un commit.

## Tarea 1 — `[limits] temperature` y herencia en subagentes
- `config/agent_limits.rs`: campo `temperature: f32` (defecto 0.0), validación 0..=2 en `from_toml`
  (pierde `Eq`; quitar donde estorbe). Pruebas: defecto, lectura, rechazo de 3.0 y -1.
- `agent/turn.rs`, `agent/compaction.rs`, `agent/handoff.rs`: usar `limits.temperature`.
- `subagent/launcher.rs` + `agent/subagents.rs` + `subagent/local_body.rs`: el lanzador recibe `with_limits` y
  el hijo `.with_limits(...)`. Prueba: el proveedor simulado del hijo ve `max_tokens` y `temperature` del padre.
- CLI: nada que cablear (ya llama a `with_limits`).

## Tarea 2 — `[provider] efforts`
- `config/limits.rs` (`ProviderLimitsConfig`): `efforts: Vec<String>` (pierde `Copy`); validación.
- `loomcoding-tui`: `effort_options()` usa los declarados cuando el catálogo no sabe. Pruebas en ambos crates.

## Tarea 3 — `StopReason::RepeatedOutput`
- `agent/stop.rs`: detección en `stop_reason`; `label()`; `step_limit.rs` lo trata como no prorrogable.
- `agent/turn.rs`: aviso `RuntimeNotice` al cortar. Pruebas: 3 idénticas cortan, 2 no, cortas no, rearme con usuario.

## Tarea 4 — documentación
- `INVARIANTS.md`/`CODE-MAP.md` si procede, `docs/INDEX.md`, CHANGELOG.
