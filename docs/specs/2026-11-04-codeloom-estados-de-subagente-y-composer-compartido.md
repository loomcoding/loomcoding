# Estados de subagente y composer compartido

- **Fecha:** 2026-11-04 (revisado 2026-11-05)
- **Estado:** Propuesta
- **Ámbito:** `codeloom-core`, `codeloom-tui`, `codeloom-cli`
- **Relacionado:** se alinea con `docs/specs/2026-11-05-codeloom-organizacion-y-navegabilidad.md`.
  Partir `app.rs` (10.769 líneas hoy) es requisito previo razonable para tocar el
  composer con seguridad: hoy el bug vive en un archivo que nadie puede abarcar.

## Problema

El subagente es hoy un "hueco" en la observabilidad del sistema:

1. **Sin latido ni última actividad.** No hay `last_activity_at` ni contadores por
   subagente; el padre no puede distinguir "trabajando" de "colgado".
2. **Sin detección de atasco.** No existe watchdog: un subagente que se queda
   esperando una herramienta o un turno del modelo no se detecta ni se recupera.
3. **Estados A2A demasiado gruesos para la UI.** `TaskState` (A2A) sí tiene ocho
   variantes (`submitted`/`working`/`input-required`/`auth-required`/`completed`/
   `failed`/`canceled`/`rejected`), pero **un turno completo se colapsa en un único
   `working`**: no se distingue generar texto, razonar, ejecutar una tool o esperar
   un permiso. Y al revés, falta granularidad de "asentado con éxito" frente a
   "asentado con error" más allá de `completed`/`failed`, y de "en cola" o "perdido".
4. **El padre no ve los estados.** `is_child_relevant()` (event/mod.rs) filtra al
   padre justo los eventos de estado de los hijos, así que el modelo que orquesta
   no recibe la información que necesita para decidir.
5. **La TUI infiere estado desde eventos** en vez de leer una proyección estable;
   cada evento de progreso se reinterpreta en el render, con riesgo de estados
   inconsistentes o parpadeo.
6. **`App` es un monolito** (app.rs, ~5.6k líneas) que mezcla layout, sesión e
   hijos; no hay un lugar claro donde viva el estado de los subagentes.
7. **No hay borrador por sesión ni destinatario de composer.** El composer escribe
   "al padre" siempre; no hay forma de dirigir un mensaje a un hijo concreto, ni de
   conservar un borrador entre sesiones.

## Auditoría del estado actual

| # | Hallazgo | Evidencia | Impacto | Esfuerzo |
|---|----------|-----------|---------|----------|
| 1 | `TaskStatus` A2A sin granularidad de asentamiento | `codeloom-a2a` (card/task) | Alto | S |
| 2 | `is_child_relevant()` filtra los eventos de estado de hijos | `event/mod.rs` | Alto | S |
| 3 | No hay `last_activity_at` ni contadores por subagente | `session/mod.rs` (`SubagentDescriptor` 21-40) | Alto | M |
| 4 | No hay watchdog ni detección de atasco | (ausente) | Alto | M |
| 5 | TUI infiere estado desde eventos, no desde proyección | `codeloom-tui` (render) | Medio | M |
| 6 | `App` monolítico (layout + sesión + hijos) | `app.rs` (~5.6k líneas) | Medio | L |
| 7 | Sin borrador por sesión ni destinatario de composer | `app.rs` / `composer` | Medio | M |
| 8 | `runtime_snapshot` no resume estado de subagentes | `runtime_snapshot` (1623-1639) | Bajo | S |

## Máquina de estados del subagente

### Enum fino (local, en `codeloom-core`)

```rust
/// Fase local del subagente. Vive junto a `TaskStatus` (A2A) pero NO lo sustituye:
/// es la vista rica que el runtime mantiene y que se proyecta a A2A al publicar/guardar.
pub enum SubagentPhase {
    /// Creado, aún no lanzado.
    Queued,
    /// Arrancando el turno: sesión/tarea creada, aún sin primer token.
    Starting,
    /// Turno del modelo en curso (esperando o recibiendo tokens).
    Streaming,
    /// El modelo está razonando (bloques de razonamiento explícitos).
    Reasoning,
    /// Ejecutando una herramienta (el subagente no consume tokens mientras tanto).
    ToolRunning,
    /// Bloqueado esperando aprobación de permiso del usuario.
    AwaitingPermission,
    /// Esperando entrada del padre (petición A2A `input-required`).
    AwaitingParentInput,
    /// Esperando credenciales de un agente remoto A2A (`auth-required`).
    AwaitingCredentials,
    /// En ejecución activa sin sub-fase conocida (fallback / motores que no la exponen).
    Working,
    /// Asentado con éxito (resumen disponible).
    SettledOk,
    /// Asentado con error (resumen/error disponible).
    SettledErr,
    /// El agente remoto rechazó la tarea (`rejected`).
    Rejected,
    /// Cancelado por el padre o el runtime.
    Canceled,
    /// Sin latido durante más del umbral: posible atasco.
    Stalled,
    /// Sin señal y sin forma de recuperar: perdido.
    Lost,
    /// Estado A2A remoto no reconocido por esta versión (compatibilidad futura).
    Unknown,
}
```

**Por qué tantas fases y no una sola `Working`.** El usuario pidió literalmente
"más estados para saber en todo momento en qué estado se encuentra". Un único
`Working` colapsa todo un turno (que puede durar minutos) y es justo lo que hoy
hace que un subagente parezca "perdido": se ve `working` durante 4 minutos y no se
distingue de un cuelgue. Las sub-fases de ejecución se justifican así:

| Fase | Qué la justifica en el código actual |
|------|--------------------------------------|
| `Starting` | Hay un intervalo real entre crear la tarea y el primer token (creación de sesión, carga de skills, resolución de agente). |
| `Streaming` | El provider emite deltas de texto; es el estado más frecuente y el que interesa mostrar. |
| `Reasoning` | Los providers exponen bloques de razonamiento separados del texto. |
| `ToolRunning` | Las tools son `Exclusive` (barrera) y pueden ser largas; hoy el hijo se ve `working` igual que si estuviera generando. |
| `AwaitingPermission` | Ya existe el flujo de permisos en `codeloom-tui/src/permission_flow.rs`; hoy el subagente queda mudo esperando al usuario. |
| `AwaitingCredentials` | A2A `auth-required`; necesario para agentes remotos (A8d-2). |

`Working` se conserva como **fallback** para motores/proveedores que no reportan
sub-fase: nunca se pierde información por no poder ser más preciso.

### Transiciones legales

| Origen | Destino | Quién la dispara |
|--------|---------|------------------|
| `Queued` | `Starting` | runtime al aceptar el encargo |
| `Starting` | `Streaming` | primer token del modelo |
| `Starting` | `ToolRunning` | el turno empieza directamente por una tool |
| `Streaming` | `Reasoning` | el provider emite bloque de razonamiento |
| `Reasoning` | `Streaming` | vuelve el texto visible |
| `Streaming`/`Reasoning` | `ToolRunning` | el modelo pide una herramienta |
| `ToolRunning` | `Streaming` | la herramienta termina y vuelve el turno |
| `ToolRunning` | `AwaitingPermission` | la tool requiere aprobación |
| `AwaitingPermission` | `ToolRunning` | el usuario aprueba |
| `AwaitingPermission` | `Streaming` | el usuario deniega (el modelo recibe el rechazo) |
| `Streaming`/`ToolRunning` | `AwaitingParentInput` | el subagente emite `input-required` |
| `AwaitingParentInput` | `Streaming` | el padre responde la petición |
| `Starting`/`Streaming`/`ToolRunning` | `AwaitingCredentials` | el agente remoto responde `auth-required` |
| `AwaitingCredentials` | `Streaming` | se resuelven las credenciales |
| `Streaming` | `SettledOk` | termina con éxito |
| `Streaming` | `SettledErr` | termina con error |
| `Streaming`/`ToolRunning` | `Rejected` | el agente remoto responde `rejected` |
| `*` (no terminal) | `Working` | fallback: motor que no reporta sub-fase |
| `Starting`/`Streaming`/`Reasoning`/`ToolRunning`/`AwaitingPermission`/`AwaitingParentInput`/`AwaitingCredentials`/`Working` | `Stalled` | watchdog al superar el umbral sin latido |
| `Stalled` | `Working` | el subagente emite latido de nuevo |
| `Stalled` | `Lost` | watchdog al superar el umbral de pérdida |
| `*` (no terminal) | `Canceled` | padre o runtime |
| `*` (no terminal) | `Unknown` | estado A2A remoto no reconocido |
| `SettledOk`/`SettledErr`/`Rejected`/`Canceled`/`Lost` | — | terminal |

`Unknown` **no es terminal**: si después llega un estado A2A reconocible, la fase se
actualiza con normalidad (un agente remoto puede estar en una versión más nueva del
protocolo y luego reportar algo que sí entendemos).

### Invariantes

- **Monotonía:** una vez en un estado terminal (`SettledOk`, `SettledErr`,
  `Canceled`, `Lost`) no hay transición de salida.
- **Terminalidad:** todo subagente alcanza un estado terminal o es cancelado.
- **No retroceso:** no se vuelve de `Settled*` a `Working`; `Stalled → Working` es
  la única "recuperación" permitida y solo por latido real del subagente.

### Mapeo `SubagentPhase` → `TaskState` (A2A)

`TaskState` tiene **ocho** variantes (`crates/codeloom-core/src/task.rs:18-27`):
`Submitted`, `Working`, `InputRequired`, `AuthRequired`, `Completed`, `Failed`,
`Canceled`, `Rejected`. La proyección **debe cubrirlas todas**.

| `SubagentPhase` | `TaskState` (A2A) |
|-----------------|-------------------|
| `Queued` | `Submitted` |
| `Starting` | `Submitted` |
| `Streaming` | `Working` |
| `Reasoning` | `Working` |
| `ToolRunning` | `Working` |
| `AwaitingPermission` | `Working` (con nota de permiso en metadatos) |
| `AwaitingParentInput` | `InputRequired` |
| `AwaitingCredentials` | `AuthRequired` |
| `Working` | `Working` |
| `SettledOk` | `Completed` |
| `SettledErr` | `Failed` |
| `Rejected` | `Rejected` |
| `Canceled` | `Canceled` |
| `Stalled` | `Working` (con nota de atasco en metadatos) |
| `Lost` | `Failed` (con nota de pérdida en metadatos) |
| `Unknown` | `Working` (conservador: no afirmamos ni éxito ni fallo) |

### Mapeo inverso `TaskState` (A2A) → `SubagentPhase`

Imprescindible para **hijos remotos**, donde la única información que llega es el
estado del protocolo. Sin esta tabla, un agente remoto en `auth-required` no tiene
a dónde proyectarse.

| `TaskState` (A2A) | `SubagentPhase` |
|-------------------|-----------------|
| `Submitted` | `Queued` |
| `Working` | `Working` |
| `InputRequired` | `AwaitingParentInput` |
| `AuthRequired` | `AwaitingCredentials` |
| `Completed` | `SettledOk` |
| `Failed` | `SettledErr` |
| `Canceled` | `Canceled` |
| `Rejected` | `Rejected` |
| *(otro / desconocido)* | `Unknown` |

Los dos mapeos son **la misma relación en dos direcciones**: el test de propiedad
sugerido en el plan verifica que para toda `SubagentPhase` existe proyección y que
el inverso de esa proyección no produce una fase terminal incompatible.

### El invariante de proyección

1. **Totalidad hacia A2A:** toda `SubagentPhase` proyecta a exactamente un
   `TaskState`. No hay fases locales sin representación en el protocolo.
2. **Totalidad desde A2A:** todo `TaskState` que llegue de un remoto tiene fase.
   Los valores no reconocidos caen en `Unknown`, nunca en `panic!` ni en descarte
   silencioso.
3. **Pérdida deliberada y declarada:** `Streaming`, `Reasoning`, `ToolRunning`,
   `AwaitingPermission`, `Stalled` y `Working` comparten `Working`. El detalle no se
   pierde porque viaja en **metadatos locales** (campo `phase` del descriptor), no
   en el protocolo.
4. **`Unknown` es conservador:** proyecta a `Working` (no a `Failed`), porque
   afirmar un fallo que no conocemos sería peor que no afirmar nada.
5. **Ningún estado A2A se queda fuera:** si el protocolo añade una variante nueva,
   el `match` de proyección rompe la compilación (exhaustividad de Rust), que es
   exactamente el recordatorio que queremos.

La vista rica (`Stalled`, `Lost`, `Streaming`, `ToolRunning`, `SettledOk` vs
`SettledErr`...) es **local**; al publicar/guardar se proyecta a A2A conservando el
detalle en metadatos para no perder información al rehidratar.

## Observabilidad y watchdog

### Estado por subagente (en `SubagentDescriptor`)

- `phase: SubagentPhase` — **campo tipado nuevo**, con columna propia en
  `session/sqlite.rs`. No se transporta la fase dentro de `TaskStatus.message`: ese
  campo es texto libre, no está tipado y `update_subagent_status` puede pisarlo.
- `last_activity_at_ms: u64` — último latido o evento de progreso, en **epoch
  milisegundos**.
- `steps: u64` — pasos (turnos de modelo) ejecutados.
- `tool_calls: u64` — invocaciones de herramienta.
- `tokens_in: u64`, `tokens_out: u64` — tokens consumidos (con cota).

**Por qué `u64` y no `Instant`.** `SubagentDescriptor` deriva
`Serialize, Deserialize, PartialEq, Eq` (`crates/codeloom-core/src/session/mod.rs`),
y `std::time::Instant` **no implementa ninguno de los cuatro**: es opaco, no se
serializa y ni siquiera tiene representación estable entre procesos. Con
`Option<Instant>` el descriptor no compila ni se puede persistir. Un epoch en
milisegundos es serializable, comparable, persiste en SQLite y sobrevive a un
reinicio del daemon (el watchdog calcula `now_ms - last_activity_at_ms` con un reloj
inyectado, así que los tests siguen siendo puros).

### Persistencia: migración

`save_subagent` (`crates/codeloom-core/src/session/sqlite.rs:314`) escribe
**columnas explícitas**, no serializa el struct entero. Por tanto añadir la fase
requiere:

- `ALTER TABLE subagents ADD COLUMN phase TEXT NOT NULL DEFAULT 'queued'`;
- `ALTER TABLE subagents ADD COLUMN last_activity_at_ms INTEGER NOT NULL DEFAULT 0`;
- columnas para `steps`, `tool_calls`, `tokens_in`, `tokens_out` (con `DEFAULT 0`).

Los `DEFAULT` son obligatorios para no romper filas ya existentes; una fila antigua
sin fase se rehidrata como `Queued` y el watchdog la saca de ahí con el primer
latido. La migración se aplica al abrir la base (patrón ya usado en ese archivo).

### Umbral de atasco

- `stall_threshold`: tiempo sin latido tras el cual `Working → Stalled`.
- `lost_threshold`: tiempo sin latido tras el cual `Stalled → Lost`.

Ambos configurables, con valores por defecto conservadores (p. ej. 60 s / 300 s) y
**cotas** para que el watchdog no dispare en falso con modelos lentos.

### Quién evalúa el watchdog

Un módulo **puro de `codeloom-core`** (sin `ratatui`/`crossterm`): recibe el
`Instant` actual y el estado del subagente, y devuelve la transición. El *tick* lo
inyecta el runtime (TUI o CLI) con un temporizador; el core no depende del reloj
del sistema ni de hilos.

### Recuperación

- `Stalled`: se notifica al padre (evento) y se ofrece cancelar o esperar.
- `Lost`: se marca terminal y se libera el slot; el padre decide reintentar o
  fallar la tarea.

### Cotas

- Contadores con `u64` y saturación (no overflow).
- Un solo evento de `Stalled`/`Lost` por transición (no por tick).

## Propagación de estado

### Eventos nuevos/ampliados

- `SubagentPhaseChanged { id, from, to }` — transición de fase.
- `SubagentProgress { id, steps, tool_calls, tokens_in, tokens_out }` — heartbeat
  con contadores (coalescible).
- `SubagentSettled { id, phase: SettledOk|SettledErr, summary }` — resumen final
  con cota de tamaño.

### `is_child_relevant()` → lista blanca

Cambiar el filtro de "excluir eventos de estado" a una **lista blanca explícita**:
el padre recibe `SubagentPhaseChanged`, `SubagentSettled` y (coalescido)
`SubagentProgress`, y NO recibe el ruido interno (herramientas, logs). La lista
blanca se documenta en el propio `event/mod.rs`.

### Throttle / coalescing

- `SubagentProgress` se coalesce: como máximo uno por subagente por ventana
  (p. ej. 1 s), conservando el último valor.
- `SubagentPhaseChanged` y `SubagentSettled` no se coalescen (son transiciones).

### Qué ve el padre (modelo) vs. qué ve la UI (humano)

- **Padre (modelo):** solo la lista blanca, con resúmenes acotados. Suficiente para
  decidir (esperar, cancelar, responder, reintentar).
- **UI (humano):** la proyección completa (`SubagentPhase`, contadores, latido),
  renderizada desde el estado, no desde eventos.

## Supervisor del runtime

### Qué rol es

Un componente **del runtime** (no un subagente): observa el árbol de subagentes y
actúa sobre él. No ejecuta tareas ni consume turnos del modelo.

### Dónde vive

En `codeloom-core` (lógica pura) con un *driver* en `codeloom-tui`/`codeloom-cli`
que le inyecta el tick y las acciones.

### Estado que mantiene

Una agregación por árbol: por cada subagente, su `SubagentPhase`, `last_activity_at`
y contadores; más un resumen global (nº activos, asentados, atascados, perdidos).

### Cómo se le informa

Suscripción al bus de eventos + agregación por árbol (padre → hijos). No sondea
estado; reacciona a eventos y al tick del watchdog.

### Acciones

- **Avisar:** emitir `SubagentPhaseChanged`/`SubagentSettled` al padre.
- **Cancelar por atasco:** al pasar a `Lost`, cancelar el subagente y liberar el slot.
- **Resumen en `runtime_snapshot`:** incluir el estado agregado de subagentes.

### Qué NO hace

- No decide la política de reintento (eso es del padre/modelo).
- No ejecuta herramientas ni turnos de modelo.
- No duplica el motor: solo observa y dispara transiciones de watchdog.

### Interfaz mínima

```rust
// Nota: NO existe un tipo `SubagentId` en el código; el identificador real es
// `SessionId` (crates/codeloom-core/src/session/mod.rs:11), y el descriptor usa
// `parent: SessionId` / `child: SessionId`.

/// Lógica pura: sin reloj, sin hilos, sin I/O. El "ahora" se inyecta.
pub struct SubagentSupervisor { /* último latido por hijo, umbrales */ }

impl SubagentSupervisor {
    /// Observa el bus sin consumirlo.
    pub fn on_event(&mut self, ev: &LiveEvent);

    /// El runtime llama a tick() con su propio reloj; devuelve acciones a ejecutar.
    pub fn tick(&mut self, now_ms: u64) -> Vec<SupervisorAction>;
}

pub enum SupervisorAction {
    /// Avisar al padre (nota acotada, no un turno nuevo).
    Notify { parent: SessionId, child: SessionId, phase: SubagentPhase },
    /// Marcar el hijo en una fase derivada del watchdog.
    Mark { child: SessionId, phase: SubagentPhase },
    /// Cancelar el hijo.
    Cancel { child: SessionId },
}
```

`on_event` recibe `&LiveEvent` (el enum real del bus,
`crates/codeloom-core/src/event/mod.rs:64`) y `tick` recibe `now_ms: u64` en vez de
`Instant`, para que el core no dependa de un reloj y los tests sean deterministas.

## Composer compartido padre/hijo

### Destinatario activo

El composer tiene un **destinatario activo** (`Option<SessionId>`: `None` = padre,
`Some(id)` = ese hijo).
El header/rail muestra siempre a quién va dirigido el mensaje para evitar ambigüedad.

### Un borrador por sesión

Un único borrador persistente por sesión (no por destinatario): al cambiar de
destinatario el borrador se conserva, evitando pérdida de texto.

### Atajos

- **`Ctrl+T`** — alternar destinatario (padre ↔ hijo activo).
- `/focus <id>` — fijar destinatario a un subagente concreto.
- `/parent` — volver al padre (**ya existe**, `codeloom-tui/src/commands.rs:18` y
  `:56`; no se añade `/back`).

**Por qué `Ctrl+T` y no `Tab`.** `Tab` **ya está ocupado** en dos sitios:
el foco del rail (`crates/codeloom-tui/src/run.rs`) y los diálogos de permiso
(`crates/codeloom-tui/src/permission_flow.rs`). Reutilizarlo obligaría a un
prefijo o a un modo, y rompería el reflejo de "Tab mueve el foco". `Ctrl+T` está
libre y se lee como "target".

### El bug real que arregla el borrador por sesión

No es solo una mejora de comodidad. Hoy `load_session`
(`crates/codeloom-tui/src/app.rs:2802`) **no toca `self.composer`**, y abrir un hijo
**cambia de sesión** (`open_child` → `load_session`). Consecuencia verificada:

1. Escribes un párrafo para el padre.
2. Abres un hijo: el composer **conserva el texto** y ahora parece dirigido al hijo.
3. Si envías, ese texto va al hijo; si vuelves al padre, arrastras lo que quedara.

Es exactamente el "no sabemos dónde estoy escribiendo" que reportó el usuario. Un
borrador por sesión (guardado y restaurado en `load_session`) lo resuelve de raíz, y
además convierte la conmutación en algo sin pérdida de texto.

### Semántica de envío según destino

| Destino | Acción al enviar |
|---------|------------------|
| Hijo en curso (`Streaming`/`ToolRunning`/`Working`) | Encolar mensaje (cola del hijo) |
| Hijo en `AwaitingParentInput` | Responder la petición pendiente |
| Hijo en `AwaitingPermission` | No se puede: el envío se rechaza con aviso (el hijo espera al usuario, no al padre) |
| Hijo en `AwaitingCredentials` | No se puede: hay que resolver credenciales primero |
| Hijo en `Stalled` | Advertir; permitir enviar como nuevo turno o cancelar |
| Hijo asentado (`SettledOk`/`SettledErr`/`Rejected`) | Nuevo turno (relanzar) |
| Hijo en `Lost`/`Canceled` | No se puede: hay que relanzar explícitamente |
| Hijo remoto A2A | Mensaje A2A (`message/send`) |
| Padre (`None`) | Turno normal del padre |

### Relación con `enum Focus` y `submit()`

El destinatario del composer se modela como una extensión del `enum Focus`
existente (o un campo paralelo `composer_target`), y `submit()` despacha según la
tabla anterior. No se reescribe el flujo de envío del padre; se añade el despacho
por destino.

## Alternativas consideradas

### Máquina de estados

- **A) Extender `TaskStatus` (A2A) con fases finas.** Se descarta: rompe la
  compatibilidad A2A y mezcla la vista local con el protocolo.
- **B) `SubagentPhase` local + proyección a A2A (elegida).** Mantiene A2A intacto y
  da granularidad local.
- **C) Solo eventos, sin enum.** Se descarta: sin estado canónico la TUI sigue
  infiriendo y el watchdog no tiene qué evaluar.

### Watchdog

- **A) Hilo/timer en el core.** Se descarta: introduce dependencia de reloj/hilos
  en el core puro.
- **B) Tick inyectado + lógica pura (elegida).** Testeable sin red ni reloj real.
- **C) Watchdog en la TUI.** Se descarta: duplica lógica y no cubre el modo CLI.

### Composer

- **A) Un borrador por destinatario.** Se descarta: más estado y riesgo de texto
  "perdido" al cambiar de hijo.
- **B) Un borrador por sesión (elegido).** Simple y sin pérdida.
- **C) Composer separado por hijo.** Se descarta: multiplica widgets y rompe el
  flujo único.

## Preguntas abiertas

1. ¿Valores por defecto de `stall_threshold`/`lost_threshold`? ¿Deben ser
   configurables por subagente o globales?
2. ¿El `SubagentSettled.summary` debe alimentar la memoria del padre o solo el
   `runtime_snapshot`?
3. ¿La cola de mensajes de un hijo en `Working` tiene cota? ¿Qué pasa si se llena?
4. ¿`/focus` debe aceptar hijos remotos A2A o solo locales?
5. ¿El supervisor debe poder cancelar por atasco sin confirmación del padre, o
   siempre avisar primero?