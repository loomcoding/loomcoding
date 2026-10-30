# Plan: error HTTP de Anthropic y resultado duplicado de subagente

Spec: [`docs/specs/2026-10-29-codeloom-anthropic-400-subagent-result-duplication.md`](../specs/2026-10-29-codeloom-anthropic-400-subagent-result-duplication.md)

## Cambios

1. [x] Corregir la serialización de `Role::System` para Anthropic y conservar
   en el error HTTP un campo `error.message` limitado en longitud.
2. [x] Mantener avisos de finalización de subagente en el historial enviado al
   modelo, pero quitar el cuerpo duplicado del transcript del padre en vivo y
   restaurado.
3. [x] Añadir pruebas pequeñas para ambos comportamientos.

## Validación

- [x] Pruebas Anthropic: 20 pasadas, 1 ignorada; pruebas de restauración TUI:
  2 pasadas.
- [x] `cargo fmt --all -- --check`, `git diff --check` y Clippy de los dos
  crates afectados con `-D warnings`.
- [ ] Reprobar con el proveedor Anthropic configurado; el mensaje HTTP debe mostrar
  la causa que devuelva el endpoint.
