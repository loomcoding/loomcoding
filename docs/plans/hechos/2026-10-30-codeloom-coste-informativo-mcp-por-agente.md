# Plan: coste informativo de MCP por agente

> Estado: **hecho**. Verificado el 2026-10-30: `cargo fmt --check`, `clippy -D warnings`, `build` y `test` completos en verde con `CARGO_TARGET_DIR=target/e2e`.

Spec: [`docs/specs/hechos/2026-10-30-codeloom-coste-informativo-mcp-por-agente.md`](../../specs/hechos/2026-10-30-codeloom-coste-informativo-mcp-por-agente.md)

## Fases

1. Trazar el camino de `ProviderRequest`, nombres MCP namespaced, precios,
   `UsageRecord` y sesiones hijas; confirmar cómo asociar cada petición al
   agente y sesión propietarios.
2. Añadir un ledger acotado de estimación por petición, servidor MCP y agente,
   contando únicamente las definiciones realmente enviadas. Repetir el conteo
   cuando se vuelvan a enviar; no contar inicialización del servidor.
3. Persistir el desglose con el uso de la sesión para que sobreviva al cierre y
   a la recarga, con una migración aditiva compatible con sesiones existentes.
4. Mostrar tokens por petición y acumulado/coste estimado en el widget MCP,
   separado del total agregado que informa el provider; coste desconocido si no hay
   precio catalogado.
5. Conservar la medición como informativa, sin conectarla a budget ni sumar el
   componente al total agregado. Skills quedan fuera hasta que se cargue texto
   real de `SKILL.md`.
6. Verificar con Provider falso las definiciones, repetición por petición,
   atribución entre dos agentes, persistencia y ausencia de doble conteo.
   Ejecutar `cargo build`, `cargo test`, `cargo fmt --check` y
   `cargo clippy -- -D warnings`.

## Estado

- [x] Preparar spec y plan para revisión.
- [x] Aprobación e implementación solicitadas por el usuario.
- [x] Ledger estimado por petición, servidor y agente.
- [x] Persistencia aditiva y widget MCP informativo.
- [x] Roundtrip SQLite de componentes de uso y estimación core.
- [x] `cargo build`, `cargo fmt --all -- --check` y
  `cargo clippy -- -D warnings`.
- [x] `cargo test --workspace`. Resuelto el 2026-10-30: `cargo fmt --check`, `clippy -D warnings`, `build` y `test` completos en verde (0 fallos, `codeloom-cli` incluido) con `CARGO_TARGET_DIR=target/e2e`; los dos fallos del composer y del slash menu ya no se reproducen.
