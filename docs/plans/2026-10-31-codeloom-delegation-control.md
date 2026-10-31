# Plan — Control de la delegación

> Estado: en ejecución.
> Spec: `docs/specs/2026-10-31-codeloom-delegation-control.md`.

1. **C3.** Pruebas del lanzador con respuestas Approve, Cancel y sin usuario, y con otros modos; luego campo
   `confirm`, `with_delegation_confirm` y la pregunta en `start`; el agente lo pasa siempre al lanzador.
2. **C1.** Pruebas de lectura de `delegation` en `[agents]` y de `delegation_rule`; luego el enum, la regla del prompt y
   el filtrado de herramientas con `off`. Cableado en `codeloom-cli`.
3. Cuatro comprobaciones, inventario y commit (uno por parte).
