# Plan: diagnóstico de fallos en subagentes locales

> Estado: implementado y verificado el 2026-10-27.

Spec: `docs/specs/hechos/2026-10-27-codeloom-subagent-failure-diagnostics.md`.

1. [x] Propagar el detalle terminal del `JobSnapshot` al estado fallido local solo
   cuando el almacén no tenga ya un estado terminal específico.
2. [x] Cambiar `subagent_list` para emitir `state` como texto estable y `message`
   como detalle opcional acotado.
3. [x] Añadir una prueba de fallo local que compruebe persistencia del error y una
   prueba pequeña del formato de `subagent_list`.
4. [x] Ejecutar `cargo test -p codeloom-core` (507 pasaron, 1 ignorado),
   `cargo check -p codeloom-cli` y `git diff --check`.

`cargo fmt --all --check` sigue señalando dos líneas preexistentes en
`crates/codeloom-tui/src/render.rs`; los dos archivos Rust de este cambio se
formatearon directamente con `rustfmt --edition 2021`.
