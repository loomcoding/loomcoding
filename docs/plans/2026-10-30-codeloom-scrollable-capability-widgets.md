# Plan: widgets desplazables de capacidades

> Estado (anotado en la higiene del 2026-11-01): implementado; cabo suelto: revisión visual en terminal real.

Spec: [`docs/specs/2026-10-30-codeloom-scrollable-capability-widgets.md`](../specs/2026-10-30-codeloom-scrollable-capability-widgets.md)

## Fases

1. Reutilizar el snapshot real MCP/LSP/Skills y el panel Activity existente.
2. Partir el espacio Activity en herramientas recientes y tres widgets de
   capacidades con nombres y estado visible.
3. Dar a cada widget su propio offset; enfocar con clic y desplazar con rueda o
   PageUp/PageDown sin tocar el transcript ni otros widgets.
4. Mantener el resumen superior compacto y el rail Crush existente.
5. Ejecutar `cargo fmt --all --check`, `cargo build` y
   `cargo clippy -- -D warnings`; registrar la prueba visual como pendiente
   hasta abrir el binario en un terminal real.

El cálculo informativo MCP y la atribución de skills se muestran en los widgets
correspondientes; ambos se etiquetan como estimaciones y no cambian el total.

## Estado

- [x] Crear esta spec y este plan antes del código.
- [x] Renderizar Activity compacto y widgets MCP/LSP/Skills con datos reales.
- [x] Añadir foco y scroll independiente con mouse/teclado.
- [x] `cargo fmt --all --check`, `cargo clippy -p loomcoding-cli -- -D warnings` y build del binario `loomcoding`.
- [x] Test de geometría del dashboard y scroll independiente de widgets MCP/LSP/Skills.
- [x] Build workspace, `cargo fmt --all -- --check` y `cargo clippy -- -D warnings`.
- [x] `cargo test --workspace`: dos tests del composer/slash menu fallan en `loomcoding-tui`. Resuelto el 2026-10-30: `cargo fmt --check`, `clippy -D warnings`, `build` y `test` completos en verde (0 fallos, `loomcoding-cli` incluido) con `CARGO_TARGET_DIR=target/e2e`; los dos fallos ya no se reproducen.
- [ ] Revisión visual en terminal real.
