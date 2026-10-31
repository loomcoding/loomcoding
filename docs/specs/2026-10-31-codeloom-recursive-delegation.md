# CodeLoom — Delegación recursiva acotada por profundidad

> Estado: **fase 1 (motor) implementada** (2026-10-31). **Fase 2 (interfaz) implementada** (2026-11-01): ver `2026-11-01-codeloom-recursive-delegation-ui.md`.
>
> Origen: decisión futura anotada en `2026-10-31-codeloom-delegation-limits.md` §4 («aplicar `max_depth` de verdad»).
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [Recursive Sub-Agent Delegation](https://agentpatterns.ai/patterns/multi-agent/recursive-sub-agent-delegation-depth/).
> Relacionado: `2026-10-31-codeloom-delegation-control.md` (aprobación y mando de delegación),
> `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` (todo bucle lleva cota).

## 1. Problema

La profundidad real de delegación es 1: el hijo se construye sin la herramienta `subagent` y sin lanzador, de modo que no
puede delegar. El campo `max_depth` del perfil se lee y, si vale algo distinto de 1, solo se avisa («declarado y no
aplicado»). Un perfil que declare una cadena de especialistas (por ejemplo, un coordinador que reparte entre revisores) no
puede expresarla.

## 2. Objetivo

Permitir que un subagente delegue en otros, hasta una profundidad fijada por **dos candados** que deben abrirse a la vez, y
sin que el coste, la concurrencia ni la cancelación escapen al control actual.

## 3. No objetivos

- El comportamiento por defecto no cambia: sin configuración, la profundidad sigue siendo 1.
- No se permite la recursión sin cota: hay un techo duro de profundidad.
- No se cambia el protocolo A2A: cada tarea sigue siendo una tarea de su padre; la jerarquía es de CodeLoom.
- No se diseña aquí la presentación en la interfaz de más de un nivel (fase 2).
- No se mide ahorro ni calidad: eso exigiría una batería y gasto real, con su propia autorización.

## 4. Diseño

### 4.1 Los dos candados

1. **Techo del árbol:** `[agents] max_depth` (entero). Sin valor, **1** (recursión apagada). Se acota a `1..=3`.
2. **Permiso del perfil:** `max_depth` del perfil de agente, ahora con significado propio: *niveles del subárbol que ese
   agente encabeza, contándose a sí mismo*. `1` o ausente es una hoja; `2` puede lanzar hijos que son hojas; etc.

### 4.2 Regla

Se definen los «niveles restantes» de una sesión:

- Raíz (la sesión de la persona usuaria): `techo + 1`.
- Hijo: `min(restantes(padre) - 1, max_depth del perfil del hijo o 1)`.

Una sesión **puede delegar solo si tiene al menos 2 niveles restantes**. Con el techo por defecto (1) la raíz tiene 2 y
los hijos como máximo 1: igual que hoy. Para que exista un nieto hacen falta techo ≥ 2 **y** un perfil con
`max_depth` ≥ 2 en el hijo. Un perfil que pida más de lo que el techo permite queda recortado, y el recorte se avisa
una vez como aviso de ejecución, no como «declarado y no aplicado».

### 4.3 Qué recibe el hijo que puede delegar

- Las herramientas `subagent`, `subagent_control` y `subagent_list`, con un lanzador anidado cuyo «padre» es el hijo.
- **Nunca** `development_workflow`: el flujo de desarrollo solo lo gobierna la sesión raíz.
- El mando `[agents] delegation = "off"` sigue mandando: con él nadie recibe esas herramientas, a ningún nivel.

### 4.4 Lo que se comparte en todo el árbol

- **Concurrencia:** el lanzador anidado comparte el semáforo del lanzador raíz, de modo que `max_active_subagents` cuenta
  el árbol entero, no cada nivel.
- **Presupuesto:** los nietos suman a la familia de la **raíz**, no a la de su padre inmediato, de modo que el tope
  duro (`[budget] hard_stop`) los alcanza.
- **Cancelación:** el token de cancelación de un hijo es el del trabajo que lo ejecuta; cancelar al padre baja por todo el
  subárbol.
- **Aprobación:** cada delegación, a cualquier nivel, pasa por la aprobación de `confirm_delegation = always`; con
  `--yolo` o sin usuario se deniega (la regla de C3 no cambia).
- **Pasos:** cada sesión conserva su `max_steps`; un nieto no tiene más pasos que los que su perfil le da.

### 4.5 Lo que ve la persona usuaria (fase 1)

El nieto queda guardado como tarea de su padre (`list_subagents(padre)`), con su identidad y sus artefactos, y emite
eventos etiquetados como cualquier hijo. Sin presentación específica por niveles: se verá como una tarea más. Mostrarlo
bajo su padre, con sangría y el camino completo, es la fase 2, ya hecha.

## 5. Criterios de aceptación

1. Con el techo por defecto, el hijo no recibe `subagent` aunque su perfil diga `max_depth: 3`; se avisa el recorte.
2. Con techo 2 y perfil con `max_depth: 2`, el hijo recibe `subagent` y puede lanzar un nieto; el nieto no recibe
   `subagent` (es una hoja).
3. Con techo 3 y perfiles con `max_depth` 3 y 2, la cadena raíz → hijo → nieto → bisnieto existe y el bisnieto es hoja.
4. Con techo 2 pero un perfil sin `max_depth`, el hijo no delega.
5. `development_workflow` no llega a ningún hijo; con `delegation = "off"` no llegan las herramientas de subagentes.
6. El tope `max_active_subagents` cuenta padres e hijos: con tope 2, una tercera tarea en cualquier nivel se rechaza.
7. Cancelar al padre cancela al nieto.
8. El gasto de un nieto suma a la familia de la raíz y puede activar `hard_stop`.
9. La aprobación por código se pide para la delegación de un hijo igual que para la de la raíz.
10. Las cuatro comprobaciones del proyecto pasan. Todo con proveedor simulado, sin red.

## 6. Riesgos

- **Coste:** cada nivel multiplica llamadas. Por eso hay dos candados, techo duro de 3, semáforo y presupuesto del árbol.
  Aun así, queda como riesgo operativo; se documenta que activarlo exige conocer el gasto.
- **Contexto y diario:** un nieto cuya salida no llega a la sesión raíz puede esconder trabajo. La fase 1 lo deja guardado y
  visible como tarea del hijo, pero no en el diario de la raíz; la fase 2 lo cierra: la interfaz muestra a los descendientes con sangría y camino.
- **Bloqueo por capacidad:** si padres e hijos agotan el semáforo esperando unos a otros, no avanza nadie. Se mitiga
  rechazando (no esperando) cuando no hay hueco, que es el comportamiento actual.
- **Perfiles remotos A2A:** un agente remoto no recibe la recursión (decide por sí mismo); el techo solo rige los locales.

## 7. Nota de implementación

- `session_usage(sesión)` solo sumaba los hijos **directos** (una consulta de un nivel), de modo que el gasto de un
  nieto no aparecía en el total de la raíz ni en el presupuesto. Ahora es una consulta recursiva sobre todo el subárbol
  (con `UNION`, que deduplica y protege de ciclos). Esto cambia también el total que ve cualquier sesión con
  descendientes: incluye ya a los nietos.
- Un hijo que no puede delegar sigue sin ver `subagent`, `subagent_control`, `subagent_list` ni `development_workflow`
  (el constructor del lanzador ya se las quitaba). El mando `[agents] delegation = "on_request" | "proactive"` solo
  rige en la sesión raíz: los hijos que delegan no reciben su regla.
- Sin cubrir en pruebas: el `hard_stop` de un nieto de extremo a extremo. Está cubierto que comparte la familia de la raíz
  (`familia`) y que el total de la raíz suma el subárbol.
