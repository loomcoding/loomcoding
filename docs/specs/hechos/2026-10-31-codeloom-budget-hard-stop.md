# CodeLoom — Tope duro opcional de presupuesto

> Estado: **implementado** (spec escrita junto con el código, no antes; queda dicho).
>
> Fecha: 2026-10-31
>
> Origen: candidata D1 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Relacionado: `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` y el aviso de presupuesto (`budget.rs`).
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [Unbounded Agent Feedback Paths](https://agentpatterns.ai/patterns/anti-patterns/unbounded-agent-feedback-paths/).

## 1. Problema

`[budget]` solo avisa al 80 % y al 100 %. Un bucle del agente sigue gastando después del aviso. No es la reserva
por ejecución (descartada); es un corte simple y opcional.

## 2. Diseño

- `[budget] hard_stop = true` (por defecto `false`: nada cambia).
- `budget_exhausted(config, apuntes_de_la_familia)`, función pura: con `hard_stop`, devuelve el primer límite
  (tokens y luego coste) que el acumulado ya haya alcanzado.
- El bucle del agente lo consulta **antes de cada petición al modelo**; si se agotó, publica un `RuntimeNotice` y
  termina el turno. La familia (padre e hijos) suma al mismo total, de modo que también corta a los subagentes.
- Sin `hard_stop` no se lee el almacén: coste nulo.

## 3. Límites conocidos

- Corta **entre** peticiones, no durante una: una respuesta en curso termina y puede rebasar el límite.
- El tope de coste no cuenta los apuntes sin precio (se marcan como parciales); el de tokens no tiene ese defecto.
- Las herramientas de la respuesta que rebasó el límite sí se ejecutan antes de la parada.
- El corte termina el turno; no cancela subagentes que ya estén en marcha, que se detendrán al intentar su próxima
  petición.

## 4. Criterios de aceptación

1. Sin `hard_stop`, un presupuesto excedido solo avisa y el turno continúa.
2. Con `hard_stop`, agotado el límite no se pide otra respuesta.
3. `hard_stop` se lee de `[budget]` y es `false` por defecto.
4. Las cuatro comprobaciones del proyecto pasan.
