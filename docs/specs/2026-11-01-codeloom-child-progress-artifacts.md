# CodeLoom — Artefactos de progreso publicados por el hijo

> Estado: **implementada** (2026-11-01) con las respuestas de §9; véase §10 para lo que cambió al construirla.
>
> Origen: fase B de `2026-10-21-codeloom-agent-autonomy-design.md`, que quedó a medias. La persona usuaria lo
> quiere para **saber cómo va el hijo** mientras trabaja, no solo cuando termina.
>
> Relacionado: `hechos/2026-10-23-codeloom-a2a-follow-up-design.md`, `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`,
> `2026-10-29-codeloom-precision-usage-live-subagent-supervision.md`.

## 1. Problema

Mientras un hijo trabaja, la persona usuaria y el padre solo ven su **estado** A2A, un mensaje de estado corto y, si el
hijo usa `todo_write`, su lista de tareas dentro de su propia sesión. No hay un lugar donde el hijo deje resultados
parciales con nombre (un plan, hallazgos hasta ahora, una decisión tomada) que se puedan leer sin entrar en su sesión
ni esperar a que termine. Hoy el único artefacto de un hijo es su respuesta final, que se crea al asentarse.

## 2. Lo que ya existe (verificado en el código)

- `Artifact` de A2A (`a2a.rs`: `artifact_id`, `name`, `description`, `parts`, `metadata`), guardado en
  `subagents.artifacts_json` del descriptor.
- `eventos_de_cambio` (`task_events.rs`) emite un `TaskArtifactUpdateEvent` por cada artefacto **nuevo** que aparece en el
  descriptor entre dos `SubagentChanged`. Por tanto, si un artefacto entra en el descriptor con la tarea en marcha,
  el streaming A2A (SSE) ya lo transmite sin tocar el servidor.
- Hoy `complete_subagent` fija los artefactos **al asentarse**; no hay operación para añadir uno antes.

## 3. Objetivo

Que el hijo publique, mientras trabaja, artefactos **con nombre y acotados**; que el padre y la persona usuaria los vean
al instante y puedan leerlos; y que A2A los transmita como los artefactos de cualquier tarea.

## 4. No objetivos

- No sustituye a `todo_write` (lista de tareas del propio hijo) ni al mensaje de estado.
- Sin ficheros ni binarios: solo partes de texto (`Part::text`).
- Sin *chunks*: no se usa `append` de A2A; publicar de nuevo con el mismo nombre **reemplaza** (`append = false`).
- No cambia el estado A2A de la tarea ni el protocolo; el nodo sigue en `working`.
- Sin hijos, no cambia nada.

## 5. Diseño

### 5.1 Herramienta del hijo: `artifact_publish`

`artifact_publish { name, content, description? }`, `ExecutionMode::Exclusive` (escribe). Solo se ofrece a los
**subagentes**, nunca a la sesión raíz (que no tiene tarea a la que adjuntar). Publicar con un `name` ya usado por el mismo
hijo reemplaza ese artefacto y conserva su `artifact_id`; con uno nuevo crea otro.

Cotas (decálogo): `name` 1–64 bytes; `description` ≤ 200; `content` ≤ 8 KiB por artefacto; ≤ 8 artefactos de progreso por
tarea y ≤ 32 KiB en total. Pasarse de cualquiera es un error de la herramienta, no un recorte silencioso.

### 5.2 Cómo se distingue lo parcial de lo final

A2A no tiene «artefacto final». Los de progreso llevan `metadata = {"codeloom": {"kind": "progress"}}`; el resto (los de
agentes remotos y la respuesta final del hijo) no llevan marca y son el **resultado**. Consecuencias:

- `SubagentDescriptor::result_text` y el aviso de finalización al padre usan **solo el resultado**, de modo que un hijo que
  publicó seis artefactos no inunda al padre con ellos (decálogo). El aviso añade una línea con nombre y tamaño de cada
  artefacto de progreso.
- `complete_subagent` **conserva** los de progreso y añade el resultado; hoy reemplazaría la columna entera.
- Un artefacto de progreso no cuenta como resultado para el caso «hijo sin respuesta final»: sigue fallando por vacío.

### 5.3 Lectura por el padre

- `subagent_list` incluye, por hijo, `artifacts: [{name, bytes}]` (listado, sin contenido).
- `artifact_read { child_id, name }` devuelve el contenido, acotado con `limit_bytes` (≤ 8 KiB) y solo de **hijos
  del llamador**, como `subagent_control`. Solo lectura, `Parallel`.

### 5.4 Qué ve la persona usuaria

- Cada publicación emite el `SubagentChanged` habitual, así que el árbol de la TUI se refresca.
- El nodo del hijo muestra, bajo su etiqueta, **de 1 a 3 líneas** del último artefacto de progreso publicado (su nombre
  y el arranque de su contenido, truncado por ancho y sin saltos), más cuántos lleva. Es una vista previa acotada: el
  contenido completo no se pinta en el árbol.
- La vista de alternancia que ya existe para entrar en el chat del hijo (abrir el hijo desde el carril y volver) es la
  forma de leerlo entero y de ver su conversación; esta spec no crea otra.
- Por A2A (servicio y SSE), cada publicación es un `TaskArtifactUpdateEvent` con `append = false` y `lastChunk = true`
  (`lastChunk` en A2A marca el último trozo de un artefacto, no el final de la tarea).

### 5.5 Persistencia y reinicios

Se guardan en `artifacts_json` del descriptor, sin tabla nueva. Un hijo `Unfinished` tras un reinicio conserva los que
publicó (es justo lo que permite saber hasta dónde llegó).

## 6. Decisiones nuestras (rotuladas, no de A2A)

- La marca `kind: progress` en `metadata` y la regla de que el resultado es lo que no la lleva.
- Reemplazo por nombre en vez de `append`.
- Las cotas numéricas de §5.1.
- Que `artifact_publish` solo exista para hijos.

## 7. Riesgos

- **Coste en el prompt del hijo:** una herramienta más gasta tokens de esquema en cada petición. Se mantiene un esquema
  mínimo y se mide el aumento de la sección de herramientas antes de darla por buena.
- **Uso excesivo:** un hijo puede publicar a cada paso. Las cotas (8 y 32 KiB) lo acotan; el modelo decide qué merece la pena.
- **Confianza:** lo que el hijo publica es texto del modelo, no una verificación; el padre debe tratarlo como tal.

## 8. Criterios de aceptación

1. Un hijo que llama a `artifact_publish` hace que aparezca un `SubagentChanged` con el artefacto y la tarea sigue `working`.
2. Publicar dos veces con el mismo nombre deja uno solo, con el mismo `artifact_id` y el contenido nuevo.
3. Superar cualquier cota falla con un error claro y no cambia nada.
4. `complete_subagent` conserva los artefactos de progreso; `result_text` y el aviso al padre solo traen el resultado y listan
   nombre y tamaño de los de progreso.
5. `artifact_read` solo lee artefactos de hijos del llamador y devuelve contenido acotado.
6. `eventos_de_cambio` emite el `TaskArtifactUpdateEvent` de uno publicado en marcha (prueba sobre el flujo existente).
7. La sesión raíz no ofrece `artifact_publish`; un hijo sin respuesta final sigue fallando aunque haya publicado.
8. Un hijo reiniciado (`Unfinished`) conserva sus artefactos de progreso.
9. Las cuatro comprobaciones pasan; todo con proveedor simulado.

## 9. Preguntas abiertas

Resueltas el 2026-11-01 por la persona usuaria:

1. **Marca en `metadata`.** `Artifact.metadata` existe en A2A y `codeloom-a2a` ya lo lleva al cable (`wire.rs`).
2. **`artifact_publish` se ofrece a todos los hijos** (con esquema mínimo), no solo a los perfiles que lo declaren en
   `scope.tools`; se revisa si el coste en tokens molesta (tarea 6 del plan).
3. **Vista previa de 1 a 3 líneas en el nodo del hijo**, y la alternancia existente para entrar en su chat (§5.4).

## 10. Notas de implementación (2026-11-01)

- **Reemplazo y orden:** republicar un nombre conserva el `artifact_id` pero **pasa el avance al final** de la lista, de
  modo que «el último» es siempre el más reciente. Es decisión nuestra.
- **Fallo hallado y corregido (tarea 4):** `eventos_de_cambio` solo emitía artefactos con `artifact_id` desconocido, así que
  una segunda publicación con el mismo nombre no se habría transmitido por A2A. Ahora emite cuando el artefacto es nuevo
  **o difiere** del ya visto.
- **Subida al padre:** el bus de cada hijo es propio y solo reenvía eventos «relevantes» envueltos en `ChildEvent`. El
  `SubagentChanged` de la propia tarea del hijo se reenvía **sin envolver** al bus del padre, igual que los cambios de tarea
  que publica el lanzador.
- **Vista previa (§5.4):** de 1 a 3 líneas, solo mientras la tarea sigue en marcha (en una asentada manda su resultado),
  truncadas a 100 columnas, con `n notes` en el resumen de la cabecera. Es el campo `preview` de `NodeView` en `batten`.
- **Quién ve qué:** `artifact_publish` se registra al lanzar cada hijo (también los de solo lectura) y nunca en la raíz;
  `artifact_read` viaja con las herramientas de delegación, así que `delegation = off` la retira y un hijo que delega la recibe.
- **Coste en el prompt (riesgo §7):** la definición de `artifact_publish` pesa 349 bytes (≈ 90 tokens) por petición del hijo. Una
  prueba (`the_tool_definition_stays_small…`) fija el tope en 450 bytes. `artifact_read` solo viaja al padre.
- **Sin medir con un modelo real:** que los modelos usen bien la herramienta (cuándo publicar, cuánto) no se ha probado; eso
  exigiría una llamada real y su autorización.
