# Plan — Desglose del contexto y caché en la interfaz

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/hechos/2026-10-31-codeloom-context-breakdown-ui.md`.

1. **Evento.** Prueba primero: el agente publica `ContextBreakdown` una vez por petición con el total de la estimación.
   Después, variante y publicación en el bucle.
2. **Líneas.** Pruebas de `context_breakdown_line` y `cache_usage_line` (con y sin datos). Después, implementarlas.
3. **Estado y panel.** Guardar el último desglose, descartarlo al cambiar de sesión, y ampliar la altura del panel una
   fila por línea con dato. Prueba de renderizado con y sin datos.
4. **Cierre.** Cuatro comprobaciones, inventario y commit.

## Verificación

Al quitar la publicación del evento o la suma de caché, la prueba correspondiente falla.
