# CodeLoom — Límites de delegación: profundidad declarada y concurrencia configurable

> Estado: **implementado** (spec escrita junto con el código, no antes; queda dicho).
>
> Fecha: 2026-10-31
>
> Origen: candidata C4 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [Recursive Sub-Agent Delegation](https://agentpatterns.ai/patterns/multi-agent/recursive-sub-agent-delegation-depth/).

## 1. Problema

- `max_depth` del perfil de agente se lee y no se aplica sin aviso. La profundidad real es 1: los hijos no reciben la
  herramienta `subagent`. Un perfil con `max_depth: 3` declara una profundidad que no existe.
- El máximo de subagentes activos es un `8` fijo en el código.

## 2. Decisión

- **Profundidad:** no se implementa la recursión (sería una función nueva, con coste y riesgo propios). Se avisa: un
  `max_depth` distinto de 1 se registra como «declarado y no aplicado», igual que `scope` o `delegation`. El valor 1
  coincide con el comportamiento real y no avisa.
- **Concurrencia:** `[agents] max_active_subagents` (entero). Sin valor, 8. Se acota a `1..=64`. El mensaje de
  capacidad agotada nombra el tope vigente.

## 3. Criterios de aceptación

1. `max_depth` distinto de 1 produce el aviso y consta en `declared_not_applied`; 1 o ausente, no.
2. Sin configurar, el tope sigue siendo 8; un valor fuera de rango se acota.
3. Con el tope configurado, la tarea que lo excede se rechaza con el número vigente en el mensaje.
4. Las cuatro comprobaciones del proyecto pasan.

## 4. Riesgos

- El tope se aplica por lanzador; un cambio de configuración exige reconstruirlo, como con el resto de opciones.
- Aplicar `max_depth` de verdad queda como decisión futura, con su spec.
