# Plan: respuesta final vacía de un subagente

Spec: [`docs/specs/2026-10-29-codeloom-subagent-empty-final-response.md`](../specs/2026-10-29-codeloom-subagent-empty-final-response.md)

## Cambios

1. Rechazar en el lanzador local un `TurnOutcome` sin texto final antes de
   persistir artefactos o devolver `JobResult`.
2. Incluir `steps/max_steps` en el error si el hijo agotó su límite.
3. Probar una llamada de herramienta en el último paso y la ruta normal con
   respuesta, sin red real.

## Estado

- [x] Rechazar el texto final vacío en el lanzador local y marcar el hijo como
  `failed`, informando el límite cuando se agotó.
- [x] Pruebas: 87 del módulo de subagentes pasadas; prueba del límite y respuesta
  normal pasadas; `cargo fmt --all -- --check`, Clippy (`-D warnings`) y
  `git diff --check` pasaron.
- [ ] Repetir el smoke en la TUI después de cerrar los daemons locales antiguos.
