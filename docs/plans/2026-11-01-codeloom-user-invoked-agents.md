# Plan — Invocación directa de agentes por la persona usuaria

> Estado: propuesta, pendiente de aprobación (2026-11-01).
> Spec: `docs/specs/2026-11-01-codeloom-user-invoked-agents.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado.
- `codeloom-core` no depende de `ratatui` ni `crossterm`.
- El compositor nunca hace trabajo por pulsación; el análisis es al enviar.
- Sin agentes invocados, comportamiento y pruebas existentes idénticos.

## Tareas

1. **Selección de skills en el lanzador (núcleo).** Pruebas primero (criterios 3 y 6): `SubagentRequest` y
   `start_requested`; el subconjunto se valida contra `scope.skills` antes de crear la tarea (no declarada, repetida, más de 16,
   agente sin skills); sin selección, idéntico a `start`. Luego el código en `subagent.rs` (el filtro entra donde hoy se llama
   a `load_scoped_skills`).
2. **Análisis de la orden (TUI, función pura).** Pruebas primero (criterios 1 y 2): `parse_agent_command(texto, agentes)`
   devuelve `RunAgent` para `/agent …` y para `@nombre …` solo si el nombre es un agente usable; `--skills` con lista; errores
   de sintaxis legibles. Luego la función.
3. **Orden de cliente a servidor.** Pruebas primero: `ClientCommand::RunAgent` viaja y el servidor la ejecuta contra la sesión
   abierta, sin turno del modelo principal (criterios 1, 4 y 5). Luego el código en `server.rs` y la ruta de envío de la TUI.
4. **Compositor y autocompletado.** Pruebas primero (criterio 7): `/agent` aparece en las órdenes con barra; el nombre se
   completa con la lista en memoria; ningún acceso a disco por pulsación. Luego `commands.rs` y el envío.
5. **Cierre.** Cuatro comprobaciones, actualizar spec y plan, inventario y la guía del estudio de plugins. Commit.
