# Plan — Mando de delegación en los hijos que delegan

> Estado: hecho (2026-11-01).
> Spec: `docs/specs/hechos/2026-11-01-codeloom-delegation-mode-for-nested-children.md`.

## Restricciones

- Las cuatro comprobaciones pasan en cada commit.
- Sin red real: proveedor simulado.
- Sin configuración de modo, comportamiento idéntico al actual.

## Tareas

1. **Pruebas primero** (`subagent.rs`): el proveedor simulado registra el prompt de sistema de cada conversación; pruebas de
   los criterios 1, 2 y 3.
2. **Código.** Campo `delegation` en `InProcessSubagentLauncher` con `with_delegation_mode`; `anidado` lo copia; el agente
   raíz se lo pasa al construir el lanzador (`agent/mod.rs`); el agente hijo con lanzador anidado lo recibe.
3. **Cierre.** Cuatro comprobaciones, quitar el límite de la spec de delegación recursiva, commit.
