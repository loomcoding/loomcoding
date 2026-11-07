# Desarrollo orgánico (ODD) como modo ligero por defecto — spec

Estado: **propuesta** (sin plan ni código). Las decisiones marcadas como «decisión provisional» las tomó quien redacta para
poder cerrar el texto; la persona usuaria puede cambiarlas. Nace de `gentle-shell`
(`docs/readme-reference.md#organic-driven-development`, clonado en `G:\loom-references\gentle-shell`), cuyo flujo por
defecto es *Organic Driven Development*. Relacionadas: `2026-10-25-codeloom-agent-development-workflow.md` (el flujo
pesado, que se conserva), `2026-11-04-loomcoding-gauntlet-minimo.md`,
`2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.

## Problema

Hoy Loomcoding tiene dos extremos. Sin flujo, el padre improvisa: no hay noción de «trabajo grande», una interrupción
pierde el hilo y nada impide dar por terminado un trabajo a medias. Con `development_workflow` (estilo SDD: spec, plan,
aprobación, implementación, revisión) hay disciplina, pero es **pesado y solo a petición**, así que no sirve para el
trabajo corriente. Falta el término medio: un modo ligero que se ajuste solo al tamaño de lo pedido.

## Qué toma Loomcoding de ODD (y qué no)

Fuente: lectura del repositorio de referencia. Es un protocolo de comportamiento del agente **más** algunas
comprobaciones de ejecución; aquí interesan sobre todo las segundas, porque una disciplina que vive solo en el prompt
no se garantiza.

**Se toma**

1. **Autorizar y explorar primero.** Una pregunta, una revisión o una propuesta se quedan en solo lectura; no se escribe
   hasta que se pida un cambio. Ante una intención ambigua, una sola aclaración.
2. **La prueba del resumen para clasificar.** Un trabajo es *pequeño* si, comprendido, con riesgo acotado, puede
   retomarse desde la petición más `git diff`; es *grande* solo si eso falla. **Nunca** clasifican los recuentos
   (ficheros, comandos, líneas). Si lo descubierto durante el trabajo cambia la respuesta, se reclasifica.
3. **Un documento de feature vivo, solo para lo grande.** Se crea automáticamente tras explorar, sin pedir permiso, y se
   avisa en una línea. Lo pequeño no deja artefactos duraderos.
4. **Casillas con prueba.** Una tarea se marca hecha solo con evidencia observada (salida de un test, un diff), nunca
   por afirmación del agente.
5. **Cierre bloqueante.** No se da por terminado un trabajo con tareas pendientes, salvo que la persona haya ordenado
   parar o el resultado sea un «Necesita tu decisión» que nombre los bloqueos.
6. **Fase inferida de la actividad.** Leer = explorando; editar = implementando; correr tests, compilar o lint =
   comprobando; preguntar = decidiendo; tocar el documento de feature = planificando. Sin llamar al modelo.
7. **TDD cuando aplica.** Si hay una prueba ejecutable y un resultado esperado claro, primero el rojo observado.

**No se toma en este corte**: entrega en cadena de PR y estrategias de reparto por tamaño, revisión nativa por nivel de
riesgo tras cada commit, espejo de memoria externa (Engram) y el comando de fase que declara el propio agente.

## Diseño

### Cuatro piezas

1. **Sección de prompt corta** (la prueba del resumen, autorizar-explorar, el cierre). Con tope de bytes propio según la
   regla 6 del decálogo; **no** se copia el protocolo entero de la referencia a cada petición.
2. **Documento de feature.** Un fichero de Markdown por feature con objetivo, motivo, alcance, restricciones, tareas con
   identificador estable y criterio de aceptación, evidencia, registro y siguiente paso. *Decisión provisional:* vive en
   el proyecto, en `.loomcoding/odd/<feature>.md`, y la sesión guarda el localizador; el recuperador lo lee entero al
   retomar y reconcilia contra el código real antes de seguir. Acotado en bytes.
3. **Cierre bloqueante impuesto por el runtime.** Una comprobación determinista al acabar el turno del padre: si hay
   tareas pendientes en el documento, el turno no se da por cerrado y se devuelve el control al modelo con la lista. Es
   **duro pero acotado**: reutiliza el presupuesto de turnos automáticos (`MAX_AUTO_TURNS` en la TUI,
   `MAX_WAKE_TURNS` sin ella; regla 7 del decálogo) y se levanta con la cancelación del usuario o con un resultado
   «Necesita tu decisión». Nunca puede entrar en un bucle sin techo.
4. **Fase visible.** Una etiqueta calculada de la actividad de las herramientas, mostrada en la cabecera y en el panel
   del agente. Es información **propia de Loomcoding, no un estado A2A**, y se rotula así.

### Cómo se relaciona con lo que ya hay

- **`development_workflow` se conserva** como el flujo pesado y explícito (spec y plan aprobados, revisor
  independiente). ODD no lo sustituye: cubre el trabajo corriente. *Decisión provisional:* convive.
- **La regla del propio repositorio «toda tarea tiene su spec y plan antes de código»** (`AGENTS.md`) es una convención
  de *este* proyecto, no del producto; no cambia. ODD es una función que ofrece Loomcoding a quien lo use.
- **El documento de feature no es el plan.** Es más ligero y cambia mientras se trabaja. *Decisión provisional:* son
  cosas distintas; un plan aprobado puede servir de punto de partida del documento, no al revés.
- **La lista de tareas de la TUI** (`TodosChanged`) pasa a ser una proyección del documento, no otra fuente de verdad.
- **Subagentes**: siguen siendo tareas A2A. Quien delega es el padre, solo con razón (paralelismo o contexto), nunca por
  tamaño; cada trabajador recibe el localizador del documento y los identificadores de sus tareas.

## Reglas

- Núcleo sin `ratatui`; ficheros nuevos ≤ 400 líneas; módulo propio (`odd/`), no dentro de `agent/`.
- **Todo acotado** (decálogo): bytes del documento, número de tareas, tamaño de la sección de prompt, reintentos del
  cierre, líneas del registro.
- **La clasificación grande/pequeña es del modelo, la comprobación es del runtime.** El runtime no adivina el tamaño; solo
  impone lo que ya está escrito (tareas pendientes, formato del documento).
- **Nunca en silencio**: crear el documento, reabrir una tarea o impedir un cierre se anuncian en el transcript, con el
  actor distinguible.
- Un documento ilegible o con formato roto **no bloquea** el cierre: se avisa y se pasa (principio de «en la duda, no
  se impide», para que un fallo del propio mecanismo no encierre al usuario).
- Sin red real en las pruebas; tiempos con reloj simulado.

## Qué se mide antes de dejarlo por defecto

Como en el resto de ideas de este proyecto, no se activa por defecto sin números propios:
- **Falsos «grande»**: trabajos pequeños que generaron documento (ruido) y falsos «pequeño» (se perdió el hilo).
- **Coste de prompt** de la sección frente a su beneficio.
- **Cierres bloqueados**: cuántas veces impidió un cierre prematuro de verdad y cuántas fue un falso bloqueo.
- **Tareas marcadas sin evidencia**, comparando con y sin la regla de casillas con prueba.

## Plan de trabajo propuesto (a detallar en el plan)

1. Documento de feature: formato, lectura, escritura, validación y cotas.
2. Cierre bloqueante en el bucle del padre, con las pruebas del peor caso (documento enorme, roto, presupuesto agotado).
3. Fase inferida de las herramientas y su aparición en la TUI.
4. Sección de prompt y la clasificación; proyección a la lista de tareas.
5. Medición en sesiones reales y decisión de activarlo por defecto.

## No objetivos

Entrega en cadena de PR, revisión nativa por riesgo, memoria externa, un agente o CLI especializado de ODD (la
referencia tampoco lo introduce) y cualquier cambio en `development_workflow`.

## Riesgos

1. **Ruido**: documentos para trabajos que no los necesitaban. Lo cubre la medición de falsos «grande».
2. **Bloqueo molesto**: el cierre impide terminar cuando la persona ya lo daba por bueno. Lo mitigan el escape de
   cancelación, el «Necesita tu decisión» y el techo de turnos.
3. **Casillas falsas**: el modelo marca sin prueba. Lo mitiga exigir evidencia en la propia marca y la comprobación de
   formato; no se puede verificar la veracidad de la evidencia por completo.
4. **Prompt caro**: la referencia gasta mucho prompt. Aquí la sección es corta y con tope.
5. **Dos fuentes de verdad** (documento y lista de tareas): se resuelve haciendo de la lista una proyección.
6. **Confundir fases con estados A2A**: se rotulan siempre como etiquetas propias.

## Decisiones de la persona usuaria (2026-11-07)

Activo **por defecto**, con interruptor (`[odd] enabled`); la medición pasa de condición previa a seguimiento posterior. Cuando
se active un modo o flujo (ODD, `development_workflow`, `gauntlet`) debe **reflejarse en algún lugar visible** (se prevé
la cabecera; tarea 5 del plan). El resto de decisiones provisionales de esta spec (evidencia, sección de prompt solo para
el padre) siguen a la espera de su revisión. Plan: `docs/plans/2026-11-07-loomcoding-odd-desarrollo-organico.md`.

## Preguntas abiertas

1. ¿Dónde vive el documento: `.loomcoding/odd/` del proyecto (provisional), `docs/` o solo en la sesión?
2. ¿Cierre bloqueante por defecto o solo si la persona activa ODD? (Aquí se supone duro con techo, tras la medición.)
3. ¿Qué cuenta como «evidencia» válida para una casilla, y quién la comprueba (formato solo, o también que el comando
   citado exista en el historial)?
4. ¿La sección de prompt se activa para todos los agentes o solo para el padre?
5. ¿Se reaprovecha la lista `TodosChanged` tal cual o hace falta un evento propio del documento?

## Estado tras la implementación (2026-11-07)

Implementado en las tareas T1–T8 del plan. Cómo quedaron las preguntas abiertas:

1. **Dónde vive el documento:** `.loomcoding/odd/<feature>.md` del proyecto, un fichero por feature (resuelta).
2. **Cierre bloqueante por defecto:** sí, activo por defecto con interruptor `[odd] enabled`; es duro pero acotado: 3
   reinyecciones por turno, ninguna si no hay progreso, plazo de 2 s, cancelación primero. **Desviación** de la
   redacción original: no reutiliza el presupuesto de turnos automáticos (resuelta).
3. **Evidencia válida:** el runtime comprueba que `call_id` es una llamada de `bash`, `job_start` o `job_output` de los
   últimos 500 mensajes de la sesión; **no** comprueba que el texto sea cierto (**provisional**: una evidencia más fuerte
   queda para después).
4. **Sección de prompt:** solo para el padre (resuelta); los subagentes no ven `odd_track` ni heredan el cierre.
5. **Lista de tareas:** se reaprovecha `TodosChanged` como proyección del documento; si el modelo usa `todo_write` a la
   vez, gana el último en escribir en pantalla (resuelta, con esa limitación documentada).

Además: la TUI es un cliente separado del daemon, así que `/odd on|off` guarda `[odd] enabled` y aplica al reiniciar. La
medición de la spec es un seguimiento posterior a la activación, con contadores por `tracing` (destino `odd`).
