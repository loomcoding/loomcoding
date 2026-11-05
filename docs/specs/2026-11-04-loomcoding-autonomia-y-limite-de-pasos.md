# Loomcoding — Autonomía y límite de pasos

> Estado: **implementada** (2026-11-04 a 2026-11-05; véase el plan y `docs/INDEX.md`). Primero de tres proyectos acordados
> (1: este; 2: ajustes desde la TUI y `/model`; 3: selector de agentes). Cada uno con su spec y su plan.
>
> Lo que dijo la persona usuaria va como decisión; lo que decide quien escribe va marcado **(mío)**.

## 1. Por qué

Un agente que explora gasta un paso por cada ronda de lectura y llega a `max_steps` (30 por defecto) antes
de escribir nada. Al llegar:

- **El agente principal** rompe el bucle en `agent/turn.rs` (`should_stop`) sin aviso ni forma de seguir.
- **Un subagente** devuelve un `Err` en `subagent/local_body.rs` («reached its step limit») y su job queda
  `failed`: **se pierde todo el trabajo hecho**, que es lo que se ve como «termina el job y no hace nada».

Otros hechos del código que importan:

- Un paso es una ronda de modelo con sus herramientas (`steps += 1` en `agent/turn.rs`), sea `view`,
  `bash` o una búsqueda.
- Las herramientas `Parallel` de una misma respuesta ya se ejecutan juntas, pero con un tope **fijo** de 8
  (`buffer_unordered(8)` en `agent/dispatch.rs`) y solo si el modelo las pide en la misma respuesta.
- Ya existe `max_active_subagents` en `[agents]`.
- `Esc` cancela el turno por token cooperativo, pero antes limpia selecciones y cierra paneles, y su doble
  pulsación **cierra la aplicación** (`run/keys.rs`). No sirve de parada segura.

Evidencia de uso (`E:\workspace\personal\llm-local-lab`, `G:\temp\loom-qwen`): con `llama-server` y un
Qwen de 9B (`-np 2`, 32k de contexto repartidos), una tarea acotada tarda de 4 a 7 minutos. Es **un caso de
prueba**, no el diseño: todo lo de abajo debe valer para cualquier proveedor.

## 2. Objetivos, no objetivos y antiobjetivos

**Objetivos**

1. Que un agente que avanza no muera por contar rondas.
2. Que llegar al tope no destruya el trabajo hecho.
3. Que se pueda ejecutar más cosas a la vez, con un techo configurable y razonable.
4. Una parada de emergencia que corte en seco el turno principal y todo lo que tenga en vuelo.
5. Que el principal se apoye en especialistas disponibles cuando los haya.

**No objetivos**

- Un motor de paralelismo nuevo: se parte de `dispatch.rs`, de `max_active_subagents` y del lanzador actual.
- Adivinar la capacidad del servidor preguntándole (cada proveedor lo expone distinto o no lo expone).
- Un tope de coste nuevo: el que hay (`hard_stop`) se queda como está; con modelos locales no es el freno.
- Parar un subagente concreto con el modo Dios (aparcado a petición de la persona usuaria).
- Cambiar lo que hace `Esc`.

**Antiobjetivos** (fracaso aunque funcione)

- Un agente sin ningún freno (cada bucle conserva una cota; decálogo de contexto y coste).
- Un modo Dios que deja procesos (`bash`, hijos) vivos tras pararse.
- Que la solución dependa de un proveedor o de un modelo pequeño concreto.

Los cambios no vigilados no son aquí un riesgo: el trabajo va guiado por spec y plan, y el Gauntlet cubre
la revisión.

## 3. Diseño

### 3.1 Pasos con progreso (mío, salvo que el paso deja de ser el freno principal)

- Se separan dos contadores del turno: **pasos** (rondas con alguna herramienta que escribe o `Exclusive`, y
  rondas sin herramientas) contra `max_steps`; y **pasos de lectura** (rondas cuyas llamadas son todas
  `Parallel`) contra un tope propio `max_read_steps`, por defecto un múltiplo generoso de `max_steps`
  (propuesta: 4×). Así leer no consume el presupuesto de trabajo, pero ningún bucle queda sin cota.
- La detección de repetición (`RepeatedToolCalls`) **no** se extiende a las lecturas: `agent/stop.rs`
  documenta como decisión deliberada que releer lo mismo es legítimo (una salida truncada obliga a volver
  a mirar). La cota de las lecturas es `max_read_steps`.

### 3.2 Al llegar al tope (mío)

- **Principal.** `StopReason::StepBudget` deja de ser un corte mudo: se publica un evento visible y se pregunta
  a la persona (`ask_user`, como la aceptación del Gauntlet) si seguir y cuántos pasos más (opciones
  acotadas). Sin quien responda (modo no interactivo) se para como hoy, pero con el motivo dicho.
- **Subagente.** En vez de `Err`, el hijo cierra con un **informe de traspaso**: una última petición al
  modelo, sin herramientas y con salida acotada, pide qué se hizo y qué falta. El job termina `completed`
  con ese informe como artefacto, rotulado como parcial; el padre decide si lo relanza. No se inventa ningún
  estado A2A nuevo. Si el informe no se puede obtener, entonces sí falla, con el motivo.

### 3.3 Paralelismo con techo (decisión; los valores son míos)

- **Herramientas.** El tope de 8 de `dispatch.rs` pasa a ser configurable (`max_parallel_tools`, defecto 8)
  y es un techo: el modelo decide cuántas pide en una ronda y las que sobren esperan en cola, no se rechazan.
- **Peticiones al proveedor.** Un envoltorio sobre `trait Provider` limita las peticiones simultáneas por
  proveedor (`max_concurrent_requests`, valor por defecto sensato; propuesta: 4). Es transversal: no conoce
  ni llama a ningún servidor concreto. Con una respuesta 429 o de servidor ocupado se baja el ritmo con
  reintentos acotados y se avisa en pantalla.
- **Subagentes.** Se usa `max_active_subagents`; no se duplica.
- **Guía al modelo.** Una frase en el prompt de sistema para agrupar lecturas independientes en una sola
  respuesta y para delegar en especialistas disponibles. Sin más maquinaria de delegación.

### 3.4 Modo Dios (decisión; tecla y detalles abiertos)

Una tecla propia, sin otro significado, que **sin confirmación**:

1. cancela el token del turno principal;
2. mata las herramientas en vuelo, incluido el árbol de procesos de `bash`;
3. cancela todos los hijos no terminales de la sesión;
4. deja un aviso visible de lo que se paró.

Cada hijo cancelado acaba en `canceled`, no en `completed` (ya lo garantiza la cancelación del padre en
`local_body.rs`). La prueba exige que no quede ningún proceso hijo vivo.

## 4. Medición (mío)

Los criterios de éxito se miden con las tareas de `G:\temp\loom-qwen`, antes y después, sobre cualquier
proveedor: pasos gastados por tarea, pasos de lectura, tiempo total, jobs fallidos por tope (objetivo: cero)
y trabajo recuperable de un traspaso. `LiveEvent::StepStarted` ya da la serie; el spec del plan fija cómo
se recoge.

## 5. Preguntas abiertas (no tratar como resueltas)

1. ¿Qué tecla es el modo Dios? Propuesta **(mía)**: `Ctrl+X`. Está libre fuera del panel de la cola, donde ya
   significa «pausar la cola y cancelar el turno»; ahí se mantiene. `Ctrl+Mayús` no es fiable en la consola de
   Windows. Se confirma al implementar.
2. ¿El subagente pregunta a la persona o solo informa al padre cuando llega al tope? (Aquí: informa; la
   pregunta queda para el principal.)
3. ¿Los valores por defecto (4× de lectura, 4 peticiones, 8 herramientas) son los correctos? Se fijan con la
   medición.
4. ~~¿Los proveedores ya reintentan ante 429?~~ Resuelta: `stream_with_retry` (`provider/mod.rs`) existe pero
   solo lo usa una prueba; en producción no hay reintentos. El retroceso ante 429 es trabajo nuevo y va dentro
   del envoltorio de §3.3.
5. ¿Dónde viven los ajustes nuevos: `[agents]`, `[provider]` o una sección propia? Se decide en el proyecto 2
   (ajustes desde la TUI), que tocará los mismos campos.
6. ¿Una petición extra de traspaso cabe en el presupuesto de contexto de un modelo con ventana corta? Debe
   medirse; si no cabe, el traspaso usa el último texto del asistente sin llamar al modelo.

## 6. Enmiendas tras el piloto con Qwen3.5-9B (2026-11-04)

Cabe señalar que el piloto (T1–T3 y tres Gauntlets contra `llama-server`) contradijo o completó el diseño en
cuatro puntos. Lo medido está en `G:\temp\loom-qwen\autonomia\logs\`; lo de aquí es lo que se deduce.

1. **Respuesta final vacía: causa hallada (hecho medido).** Un modelo con razonamiento puede cerrar el turno
   con `finish_reason: stop` habiendo escrito su respuesta *dentro* de `reasoning_content`, sin cerrar el
   bloque de pensamiento: `content` llega vacío y el JSON o el resumen exigidos quedan en el razonamiento
   (petición del crítico reproducida con un proxy de registro: `content` 0 caracteres, 809 de razonamiento
   terminando en el JSON pedido). `compat.rs` y `turn.rs` lo leen bien; el hueco está en que el turno descarta
   el razonamiento. Enmienda: el turno acumula el razonamiento y, si termina con `Stop`, sin llamadas a
   herramientas y sin texto, el mensaje final **se rotula como texto procedente del razonamiento** y se usa
   como respuesta (trazable en la UI: «sin texto, recuperado del razonamiento»). Transversal a cualquier
   proveedor que emita `reasoning_content`. Prueba primero: guion con solo `ReasoningDelta` y `Done(Stop)`.
2. **Exploración atascada.** Más presupuesto no la arregla (T3: 19 lecturas de 30, 49 de 60). Además del tope
   de lecturas de §3.1 se añade una guarda: tras N rondas de lectura seguidas sin escritura ni texto nuevo,
   se inyecta un aviso al modelo («escribe lo que llevas o declara qué te falta») antes de agotar el tope.
3. **Informe de traspaso también para el crítico del Gauntlet.** Un crítico que llega al tope sin veredicto
   hoy cuenta como fallo del Gauntlet; debe pasar por el mismo traspaso de §3.2 (una petición sin
   herramientas que fuerce el JSON).
4. **Barra de calidad no legible por el constructor** cuando el artefacto es código: el constructor copió la
   barra byte a byte. La barra se entrega solo al coordinador y al crítico. Es cambio del Gauntlet, no de
   este proyecto; se anota aquí para no perderlo.

## 7. Verificación

Los cuatro checks del repo, más pruebas por tema: cuenta de lecturas frente a escrituras; cota de lecturas
alcanzada; el principal pregunta al tope y obedece la respuesta; el subagente deja informe parcial y no
falla; tope de herramientas y de peticiones respetados con cola; modo Dios sin procesos vivos. Sin red real.
