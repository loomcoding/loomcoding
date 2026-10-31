# CodeLoom — Interfaz de la delegación recursiva (fase 2)

> Estado: **propuesta**, pendiente de aprobación (2026-11-01). Sin código hasta que spec y plan estén aprobados.
>
> Continúa `2026-10-31-codeloom-recursive-delegation.md` (fase 1, motor, ya implementada). Aquí se cierra lo que esa
> spec dejó abierto en su §4.5 y en el riesgo «Contexto y diario».
>
> Principios que rigen: A2A es la premisa (no se inventan estados; cada tarea sigue siendo tarea de su padre), la
> conversación es el diario (toda llamada a una herramienta o a un subagente es visible y su actor distinguible), el
> decálogo de contexto y coste (todo con cota) y la fluidez del compositor (nada puede hacerlo parpadear ni demorarse).

## 1. Problema

Con la recursión activada, el nieto existe, trabaja y queda guardado como tarea de su padre, pero la sesión raíz no se
entera: los eventos del nieto viajan por el bus del hijo y el reenvío filtrado del lanzador (`arrancar`, bucle de
`child_rx`) solo deja pasar los eventos «relevantes» del propio hijo, entre los que no figuran ni `SubagentChanged` ni
`ChildEvent`. En la interfaz, `LiveEvent::SubagentChanged` solo se atiende si `parent` es la sesión abierta, y
`LiveEvent::ChildEvent` igual. Consecuencia: el trabajo anidado es invisible en la raíz, que es justo el riesgo que la
fase 1 anotó («un nieto cuya salida no llega a la sesión raíz puede esconder trabajo»).

## 2. Objetivo

Que la persona usuaria vea, desde la sesión raíz, el árbol completo de subagentes: cada nieto (y bisnieto) con sangría, con
el camino completo hasta él y con el estado A2A exacto, y que sus llamadas a herramientas y su actividad aparezcan en el
diario igual que las de un hijo directo.

## 3. No objetivos

- No cambia el motor: ni profundidad, ni presupuesto, ni aprobación, ni cancelación (fase 1).
- No cambia el protocolo A2A ni los estados: la jerarquía es de CodeLoom y la interfaz la deriva de los campos `parent` y
  `child` que ya existen.
- Sin recursión activada (`[agents] max_depth = 1`, el valor por defecto), la interfaz es idéntica a la actual.
- `subagent_list` y `subagent_control` no cambian de alcance (siguen siendo por padre directo).
- No se mide nada con modelo real.

## 4. Diseño

### 4.1 Reenvío ascendente sin envolver

El reenvío filtrado del lanzador pasa a dejar subir también los eventos `SubagentChanged` y `ChildEvent` que le lleguen del
bus del hijo. Se reenvían **tal cual**, sin volver a envolverlos: ya llevan la identidad real (`parent` es el hijo, `child`
es el nieto), de modo que quien los recibe sabe de quién son sin necesidad de un camino acumulado en el evento. Solo los
emite el bus de un hijo que tiene lanzador anidado, así que con la profundidad por defecto no sube nada nuevo.

Cotas: la cantidad de eventos que sube por nivel es la de los eventos relevantes ya existentes (cada uno cruza como mucho
dos reenvíos con el techo en 3), y el bus de difusión conserva su comportamiento actual ante retraso (se pierden eventos,
no el reenvío). No hay bucles posibles: el árbol es finito y el techo es 3.

### 4.2 Qué acepta la interfaz

- `SubagentChanged(d)` se acepta si `d.parent` es la sesión abierta **o** un descendiente ya conocido (clave presente en
  `subagents`). El orden está garantizado: el descriptor del hijo se publica antes de que el hijo pueda lanzar a su propio
  hijo.
- `ChildEvent { parent, child, event }` se acepta con la misma regla sobre `parent`.
- La profundidad de un nodo se deriva recorriendo `parent` hasta la raíz sobre `subagents`, con cota igual al techo (3).
  Un `parent` desconocido se descarta, como hoy.
- Las acciones que dependen de «ser hijo directo» no cambian: `SubagentSettled` sigue despertando solo al padre inmediato
  (el hijo), nunca a la raíz por un nieto; `wake_causes` no se toca.

### 4.3 Presentación

Cada descendiente tiene su propio nodo, con la misma maquinaria que un hijo directo (despliegue, resumen de progreso,
herramientas colgadas, registro propio), con dos diferencias:

1. **Sangría** proporcional a la profundidad (dos columnas por nivel) en la cabecera del nodo.
2. **Camino** en la etiqueta: `hijo › nieto` (etiquetas de los ascendientes separadas por `›`), truncado por la izquierda
   con `…` si no cabe, de forma que el nieto, que es el actor, siempre sea legible.

El estado A2A exacto sigue entre paréntesis tras la etiqueta, como en la regla de trazabilidad vigente. Un nodo cuyo padre
llega a un estado terminal no cambia de estado por ello: cada tarea conserva el suyo.

### 4.4 Diario

Las llamadas a herramientas del nieto (`ToolStarted`/`ToolFinished`) y su actividad entran en el diario de la raíz por la
misma vía que las de un hijo directo, colgadas de su nodo. La llamada `subagent` del hijo que lanzó al nieto cuelga del
nodo del hijo y el nieto aparece como nodo propio: el actor queda distinguible por el camino (§4.3).

### 4.5 Fluidez y cotas

- Los eventos de descendientes pasan por el mismo `apply_child_event`, que ya descarta lo no relevante y no repinta si las
  líneas no cambian (`sync_activity`).
- Número de nodos: acotado por `max_active_subagents` (el semáforo cuenta todo el árbol) y por la profundidad máxima 3.
- Un nodo plegado no construye sus filas (comportamiento actual).

## 5. Criterios de aceptación

1. Con profundidad 2, un nieto lanzado por un hijo genera en el bus de la raíz un `SubagentChanged` y `ChildEvent`
   observables (prueba de núcleo con proveedor simulado).
2. Con la profundidad por defecto, no sube ningún evento nuevo al bus de la raíz (prueba de no regresión).
3. La interfaz crea el nodo del nieto, con sangría de un nivel y etiqueta `hijo › nieto`, y conserva el estado A2A exacto
   entre paréntesis.
4. Las herramientas del nieto cuelgan de su nodo en el diario de la raíz.
5. Un `SubagentChanged` con padre desconocido se descarta; un bisnieto muestra dos niveles de sangría y el camino completo.
6. Un nieto que termina no despierta a la raíz (solo a su padre).
7. Sin recursión, el árbol, las líneas y las pruebas existentes de la interfaz no cambian.
8. Las cuatro comprobaciones del proyecto pasan. Todo con proveedor simulado, sin red.

## 6. Riesgos

- **Duplicidad de eventos:** un evento de nieto lo recibe la raíz una sola vez porque se reenvía tal cual y no se vuelve a
  envolver; una prueba lo fija (criterio 1).
- **Ruido:** un árbol ancho puede llenar el diario. Se mitiga con las cotas del semáforo y el plegado por defecto de los
  nodos terminales ya existente.
- **Orden de llegada:** si algún día un descriptor llegara antes que el de su padre, se descartaría. Hoy no ocurre (§4.2);
  se deja una prueba que lo documenta.
- **Etiquetas largas:** el camino puede desbordar el ancho; de ahí el truncado por la izquierda.

## 7. Preguntas abiertas

- ¿Hace falta que `subagent_list` de la raíz muestre el árbol entero con una bandera opcional? Queda para otra spec.
- ¿Debe el despliegue de un nodo mostrar a sus hijos como filas, además de tener nodo propio? Esta spec propone solo nodo
  propio con sangría, para no duplicar información.
