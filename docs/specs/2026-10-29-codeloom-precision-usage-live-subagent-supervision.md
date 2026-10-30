# Especificación: uso de tokens preciso y supervisión en vivo de subagentes

Estado: aprobada por la persona usuaria el 2026-10-29; implementación y validación automatizada completas. La prueba interactiva final queda pendiente en la terminal con provider configurado.

## Contrato observado por adapter

| Adapter | Disponibilidad y forma | Sin uso / cancelación |
|---|---|---|
| OpenAI Chat Completions y compatibles | Solicita `stream_options.include_usage`; el uso llega como snapshot final `Usage`. | El último chunk puede faltar si el stream se interrumpe. |
| Anthropic Messages SSE | `message_start` trae entrada inicial; `message_delta` trae salida final. El segundo se combina como snapshot acumulado, nunca se suma a la entrada repetida. | Sin `message_delta`, se conserva la estimación marcada como tal. |
| Gemini `generateContent` | Respuesta no streaming con `usageMetadata`, normalizada al snapshot final `Usage`. | Error antes de la respuesta: estimación local si se pudo formar la petición. |
| OpenAI Responses | Uso de la respuesta completa (`response.usage`) como snapshot `Usage`. | Sin respuesta final, se conserva la estimación marcada como tal. |

Referencias de protocolo: [OpenAI streaming events](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events), [Anthropic streaming](https://platform.claude.com/docs/en/build-with-claude/streaming). Los adaptadores sin un evento final verificable no deben etiquetar el estimador como dato del provider.

## Problema

La persona usuaria necesita saber cuánto consume cada ejecución mientras ocurre,
qué parte del conteo es confirmada por el proveedor y qué parte es estimada. El
registro actual se actualiza al cerrar la respuesta del proveedor; algunos
providers no emiten `usage`, y el acumulador puede sumar fragmentos que sean
fotografías acumuladas en vez de incrementos.

También debe poder supervisar cada subagente en ejecución desde el padre. Debe
ver su conversación en vivo, las herramientas que llama y el resultado o error
observable, y poder detener ese hijo sin depender de que el padre esté ejecutando
un turno del modelo. El estado del panel, el workflow y el subagente debe coincidir
con el estado terminal persistido.

## Decisiones de producto

- **Conteo confirmado:** los tokens informados por el provider son la cifra
  autoritativa. Se normalizan en cada adapter con semántica explícita (incremento
  o total acumulado), se muestran al recibirse y se guardan una sola vez por
  respuesta. Los fragmentos repetidos o acumulados no se suman dos veces.
- **Conteo provisional:** antes de que llegue una cifra del provider se puede
  mostrar una estimación incremental, identificada siempre como estimación. Al
  llegar el total confirmado, reemplaza esa estimación para la misma respuesta;
  nunca se suman entre sí. Si el provider no informa uso y no hay estimador
  compatible, se muestra «sin dato», no `0`.
- **Ámbitos separados:** petición activa, turno actual, sesión y familia
  (padre e hijos) son cifras diferenciadas. Cada registro identifica su sesión,
  modelo, origen (`provider`/`estimate`) y si la respuesta terminó. Los cambios
  de modelo y los hijos no mezclan precios ni contadores.
- **Coste:** el coste confirmado se calcula con el precio congelado de cada
  registro. Una estimación de tokens o precio queda rotulada como estimada; nunca
  se presenta como cargo confirmado.
- **Subagente visible mientras trabaja:** ningún hijo activo queda oculto. El
  padre muestra su estado A2A exacto, agente, paso/herramienta actual, texto del
  asistente conforme llega, llamadas a herramientas y resultado o error acotado.
  El nodo activo se abre por defecto; cada hijo se puede seleccionar y abrir como
  sesión propia. Plegar un nodo no oculta que sigue activo ni impide detenerlo.
- **Parada directa:** desde la TUI del padre existe una acción explícita para
  cancelar un hijo concreto, aunque el turno del padre esté inactivo. Se comunica
  inmediatamente `stopping` y se refleja el estado terminal real. No cancela
  hermanos. Si un remoto rechaza o no confirma la cancelación, se muestra ese
  resultado sin afirmar que se detuvo.
- **Contenido seguro y acotado:** se muestra el texto del agente y resúmenes
  legibles de argumentos, resultados y errores de herramientas, con límites de
  tamaño y redacción de secretos. No se expone `ReasoningDelta` como chat. La
  sesión completa del hijo continúa disponible para inspección detallada.
- **Estado coherente:** un hijo terminal no puede seguir describiéndose como
  «ejecutándose» en el dashboard o en `development_workflow status`. Si terminó
  sin resultado, el workflow lo presenta como fallo/bloqueo accionable y no como
  espera activa.

## Alcance

1. Auditar el contrato de uso de cada adapter: disponibilidad de conteo en
   streaming, si cada evento es delta o total acumulado, y qué ocurre al cancelar
   o fallar una respuesta.
2. Normalizar y emitir actualizaciones de uso durante el stream; mantener un
   registro temporal por respuesta y consolidar su apunte final sin duplicados.
3. Diferenciar cifra confirmada, estimación y ausencia de datos en la persistencia,
   contadores, coste y presentación de la TUI.
4. Reenviar deltas de texto visibles y eventos de herramientas del hijo al padre;
   mantener conversación, estado y métricas sincronizados en el nodo del hijo.
5. Proveer control directo de cancelación por hijo y reconciliar estado terminal,
   errores y workflows con el almacén.
6. Recuperar la vista tras seleccionar/abrir sesiones y tras pérdida de eventos
   del canal, sin mostrar un hijo activo como ausente o terminado.

## Fuera de alcance

- Presentar estimaciones como conteos exactos o reemplazar el uso confirmado del
  proveedor por un tokenizer local.
- Mostrar razonamiento privado del modelo.
- Replicar cuerpos ilimitados de herramientas en el transcript del padre; se
  mantienen vistas detalladas y límites de tamaño.
- Cambiar el protocolo A2A. La selección, presentación y control local son
  funciones de CodeLoom; la cancelación remota respeta la respuesta del peer.
- Cambiar permisos de herramientas, ejecución exclusiva o límites de
  concurrencia.

## Criterios de aceptación

1. Para cada adapter, tests de stream prueban eventos de uso parciales,
   acumulados, repetidos, ausentes y finales; la cifra resultante coincide con el
   total final del provider y nunca duplica tokens.
2. Durante un stream, la TUI actualiza el registro de la petición activa. Cada
   cifra indica claramente `confirmado`, `estimado` o `sin dato`; la confirmada
   reemplaza la provisional sin saltos por doble suma.
3. El cierre, error o cancelación consolida como máximo un apunte por respuesta,
   con modelo, sesión, fuente y estado; el histórico restaurado coincide con los
   registros confirmados y conserva la marca de estimación cuando corresponda.
4. Turno, sesión y familia muestran contadores independientes; un hijo se
   atribuye a su propio modelo/sesión y el total familiar coincide con la suma de
   padre e hijos.
5. Mientras un hijo esté activo, se puede ver desde el padre su chat conforme
   llega, el estado, herramienta y resumen de argumentos/resultado/error. La
  actividad continúa llegando aunque el nodo se pliegue o se abra otro hijo.
  Enter alterna entre plegado/desplegado; las flechas derecha/izquierda mantienen
  sus acciones direccionales.
6. La acción de detener cancela únicamente al hijo seleccionado, sin llamada al
   modelo padre. La UI muestra la petición de parada y después el resultado real;
   rechazos, errores y timeouts quedan visibles.
7. Tras completar un hijo, el dashboard y `development_workflow status` dejan de
   marcarlo como activo. Un hijo completado sin resultado produce un estado
   terminal accionable, nunca «sigue ejecutándose».
8. No hay red real en tests. Las pruebas cubren eventos perdidos/re-sincronización,
   cargas grandes acotadas, cambio de sesión/modelo, salida de herramientas,
   proveedor sin uso, error y cancelación.

## Estado conocido que motiva la tarea

La sesión de prueba del 2026-10-29 registró un hijo como `TASK_STATE_COMPLETED`
con artefacto vacío, mientras el workflow seguía en `specifying` con
`active_child` fijado. `status` devolvió esa fila sin comprobar el hijo y el
asistente afirmó que aún corría. La fila de uso del hijo no existía; sus mensajes
tenían `usage: null`. El ledger existente persiste una fila al cerrar la
respuesta, y la TUI recibe los contadores en `LiveEvent::Usage`; no representa
una estimación en curso ni separa en pantalla la falta de datos.

La especificación previa del árbol de subagentes (2026-10-22) ya define un
registro de chat acotado y nodo plegable, pero excluye los deltas y lo deja
plegado por defecto. Esta especificación reemplaza esas dos decisiones para los
hijos activos. Conserva el límite de memoria, las llamadas de herramienta
resumidas y los estados A2A exactos.
