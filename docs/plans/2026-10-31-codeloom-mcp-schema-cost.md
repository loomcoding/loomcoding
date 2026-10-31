# Plan — Coste de los esquemas MCP al conectar

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/2026-10-31-codeloom-mcp-schema-cost.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real en las pruebas.
- El núcleo no depende de `ratatui` ni `crossterm`.

## Tareas

1. **Núcleo.** Prueba primero: `tool_schema_tokens` y el agregado por servidor, que ignora herramientas sin servidor y
   coincide con `mcp_usage_components`. Después implementar y hacer que `mcp_usage_components` use la función común.
2. **Captura.** `McpServerCapability.schema_tokens` con `#[serde(default)]`; prueba de deserialización sin el campo.
   Rellenarlo en `server.rs` desde `mcp.tools()`.
3. **Panel.** Funciones puras para el texto de la línea y para el aviso; pruebas de ambas, incluidos los dos umbrales.
   Usarlas en `render.rs`.
4. **Cierre.** Cuatro comprobaciones, inventario y commit.

## Verificación

Se comprueba a mano que las pruebas «muerden»: al alterar la suma o un umbral, la prueba correspondiente falla.
