# Plan — Señal de recorte estructurada

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/2026-10-31-codeloom-truncation-signal.md`.

## Tareas

1. Pruebas en `agent/mod.rs`: cabecera con y sin volcado, total dentro del tope, resultado corto intacto.
2. `limit_bytes_with_header` y `cut_points` en `tools/mod.rs`; `contenido_de_herramienta` con reserva de 512.
3. Cuatro comprobaciones, inventario y commit.

## Verificación

Al anular la cabecera, la prueba de cabecera falla.
