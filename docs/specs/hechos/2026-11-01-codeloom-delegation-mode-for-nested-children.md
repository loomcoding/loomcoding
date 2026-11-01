# CodeLoom — Mando de delegación en los hijos que delegan

> Estado: **implementada** (2026-11-01), aprobada por la persona usuaria en la conversación («sí, hagamos eso»).
>
> Cierra un límite anotado en `2026-10-31-codeloom-recursive-delegation.md` §7: «el mando
> `[agents] delegation = "on_request" | "proactive"` solo rige en la sesión raíz; los hijos que delegan no reciben su regla».
> Relacionado: `2026-10-31-codeloom-delegation-control.md`.

## 1. Problema

Con la recursión activada, un hijo recibe las herramientas de subagentes y un lanzador anidado, pero su prompt de sistema
no lleva la regla del mando de delegación. Con `delegation = "on_request"` la raíz solo delega si se le pide, y sin
embargo un hijo podría delegar por su cuenta; con `proactive`, la regla de delegar pronto no llega al hijo.

## 2. Objetivo

Que la regla de prompt del mando de delegación rija en todos los niveles que pueden delegar, con el mismo texto que en la
raíz.

## 3. No objetivos

- No cambia el comportamiento sin recursión: un hijo hoja no tiene herramientas de subagentes ni regla.
- `delegation = "off"` no cambia: la raíz no recibe las herramientas, así que no existe hijo que delegue.
- La regla sigue siendo persuasión del prompt, no una garantía; la garantía son los dos candados de profundidad y la
  aprobación por código (`confirm_delegation`).

## 4. Diseño

El lanzador guarda el modo (`with_delegation_mode`) que el agente raíz le pasa al construirlo. El lanzador anidado lo
hereda. El agente hijo que recibe lanzador anidado se construye con ese modo, de modo que `system_prompt` añade la misma
regla (la condición actual «hay lanzador de subagentes» ya lo permite). Un hijo sin lanzador anidado no recibe modo.

## 5. Criterios de aceptación

1. Con `on_request` y recursión, el prompt de un hijo que delega contiene la regla de `on_request`; con `proactive`, la de
   `proactive`.
2. Sin modo (`None`), el prompt del hijo no lleva regla alguna (comportamiento actual).
3. Un hijo hoja (sin lanzador anidado) no recibe la regla aunque la raíz tenga modo.
4. Las cuatro comprobaciones pasan. Todo con proveedor simulado, sin red.

## 6. Riesgos

- Persuasión, no garantía: un modelo puede ignorar la regla. Se documenta como en la raíz.
