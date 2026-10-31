# Plan — Límites de delegación

> Estado: implementado (2026-10-31).
> Spec: `docs/specs/2026-10-31-codeloom-delegation-limits.md`.

1. Pruebas y aviso de `max_depth` en `registry/profile.rs`.
2. `max_active_subagents` en `AgentsConfig`, `with_max_active` en el lanzador, `with_max_active_subagents` en el
   agente y cableado en `codeloom-cli`.
3. Cuatro comprobaciones y commit. Verificación: al ignorar el tope configurado, la prueba de capacidad falla.
