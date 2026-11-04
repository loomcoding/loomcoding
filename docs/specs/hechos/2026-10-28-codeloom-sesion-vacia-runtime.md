# Sesiones vacías en el runtime local

> Estado: **hecho** (2026-11-04). Implementado: `session_belongs_to_project` y su regresión; los cuatro checks pasan. La prueba manual de tool en la TUI con OpenRouter queda como opcional, sin bloquear nada.

## Objetivo

Permitir cargar y reanudar una sesión vacía del agente local sin debilitar el aislamiento entre agentes.

## Contrato

- `list_sessions(project)` conserva su función actual de listado reciente y devuelve solo sesiones con mensajes.
- La pertenencia de una sesión se comprueba por su fila en `sessions` y su `project`, no por aparecer en el listado.
- El runtime puede cargar una sesión vacía que pertenezca al agente.
- La conexión inicial carga la sesión actual sin pedir antes un listado vacío que dispare otra sesión nueva.
- Una sesión de otro proyecto/agente sigue siendo rechazada.

## Aceptación

- Prueba que una sesión vacía pertenece a su proyecto y no a otro.
- El listado sigue excluyendo sesiones vacías.
- El runtime acepta cargar la sesión vacía inicial y rechaza una sesión ajena.
- Pasan `cargo build`, `cargo test`, `cargo fmt --check` y `cargo clippy -- -D warnings`.
