# Plan: ratón y scroll del composer

Spec: `docs/specs/hechos/2026-10-27-codeloom-composer-scroll-and-mouse.md`.

Estado: implementado.

- [x] La rueda sobre el composer mueve el cursor por filas visibles, incluidas
  las filas envueltas; fuera de él conserva el scroll del transcript.
- [x] Barra de una columna, visible solo con overflow, con el ancho descontado
  del texto.
- [x] Preferencia guardada en `mouse = true`; `App` ya iniciaba con ratón activo.
- [x] Pruebas: `cargo test -p batten` (420) y `cargo test -p codeloom-tui`
  (452 pasaron; 2 ignoradas).
- [x] `git diff --check` pasó.
- [ ] `cargo fmt --all --check`: señala dos líneas largas preexistentes en
  `crates/codeloom-tui/src/render.rs` (1476 y 1569), fuera de este cambio.
