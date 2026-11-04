# Plan: estados de subagente y composer compartido

- **Fecha:** 2026-11-04 (revisado 2026-11-05)
- **Spec:** `docs/specs/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md`
- **Relacionado:** `docs/plans/2026-11-05-codeloom-organizacion-y-navegabilidad.md`
- **Prioridad:** A2A primero. Los mapeos del protocolo van antes que cualquier mejora de UI.

> **Nota de estructura.** `crates/codeloom-core/src/subagent.rs` es un **archivo de
> 7.510 líneas**, no un directorio. Por eso las fases crean módulos hermanos
> (`subagent_phase.rs`, `subagent_watchdog.rs`) en vez de asumir `subagent/mod.rs`.
> Convertirlo en directorio se decide en el plan de organización.

## Global Constraints

Cada fase termina en un commit que pasa los cuatro checks:

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

- Sin red real en tests (mock `Provider` o `wiremock`).
- `codeloom-core` sin `ratatui`/`crossterm`.
- Nada sin cota llega al modelo (decálogo de contexto).

## Fase 0 — Baseline y red de seguridad

**Qué cambia:** nada de código. Se fija el punto de partida para poder demostrar que
las fases siguientes no alteran comportamiento.

**Pasos:**
1. Ejecutar los cuatro checks y anotar verde/rojo al principio de este plan.
2. Anotar el recuento de tests: **2.137 atributos `#[test]`/`#[tokio::test]`** hoy.
3. Si algo ya está en rojo, arreglarlo antes de empezar y dejarlo anotado.

**Criterio de aceptación:** cuatro checks verdes y recuento anotado; sin baseline no se
puede atribuir una regresión a este plan.

**Estado: cumplido el 2026-11-05.** Commit de partida `3d23b60`. Los cuatro checks en
verde sobre ese commit: `cargo build` OK, `cargo test` OK (0 fallos), `cargo fmt --check`
OK, `cargo clippy --all-targets -- -D warnings` OK. Recuento verificado por script sobre
`crates/**/*.rs`: **2.137** atributos `#[test]`/`#[tokio::test]`, que coincide con la
cifra de referencia. Nada estaba en rojo, así que no hubo que arreglar nada previo.

## Fase 1 — `SubagentPhase` y mapeos A2A (core puro) — **prioridad A2A** (HECHA)

**Hecho:** `SubagentPhase` y sus mapeos viven en `subagent/phase.rs` (antes `subagent_phase.rs`; el alias
`subagent_phase` conserva la ruta pública). Los tests conservan la intención del plan, aunque no
siempre su nombre (p. ej. `todas_las_fases_proyectan`, `desconocido_es_unknown`).

**Qué cambia:** añadir `enum SubagentPhase` en `codeloom-core` (junto a `TaskStatus`,
sin tocar la wire de A2A), con la tabla de transiciones y **los dos mapeos**:
`SubagentPhase → TaskState` (las 8 variantes) y `TaskState → SubagentPhase` (inverso,
necesario para hijos remotos).

**Archivos:**
- `crates/codeloom-core/src/subagent_phase.rs` (nuevo, módulo hermano)
- `crates/codeloom-core/src/lib.rs` (declarar el módulo)

**Test concreto (puro, sin red):**
- `todas_las_fases_proyectan` — cada `SubagentPhase::ALL` tiene exactamente una proyección.
- `todos_los_task_state_tienen_fase` — recorre `TaskState::ALL` (son **8**) y verifica el
  inverso; **incluye explícitamente `AuthRequired` y `Rejected`**.
- `roundtrip_no_terminal_incompatible` — el inverso no produce fase terminal contradictoria.
- `transiciones_ilegal` — retroceso y salida de terminal se rechazan.
- `desconocido_es_unknown` — estado remoto no reconocido cae en `Unknown`, nunca en `panic!`.
- `working_es_fallback` — motor sin sub-fase produce `Working`.

**Criterio de aceptación:** `cargo test -p codeloom-core` verde; `match` exhaustivo (una
variante A2A nueva rompe compilación).

**Criterio de aceptación:** `cargo test -p codeloom-core` verde; `cargo clippy -- -D warnings` limpio.

## Fase 2 — Estado de observabilidad en `SubagentDescriptor` (HECHA)

**Hecho:** `SubagentDescriptor` lleva `phase`, `last_activity_at_ms`, `steps`, `tool_calls` y los
contadores de tokens (`session/mod.rs`), persistidos en SQLite con `DEFAULT` en cada columna nueva;
la fase y el latido se actualizan con `update_subagent_runtime`.

**Qué cambia:** en `SubagentDescriptor`: `phase: SubagentPhase` (**campo tipado**
propio), `last_activity_at_ms: u64`, `steps`, `tool_calls`, `tokens_in`,
`tokens_out` (saturados).

**Dos decisiones no negociables:**
- **Nunca `Instant`**: el descriptor deriva `Serialize, Deserialize, PartialEq, Eq` y
  `Instant` no implementa ninguno de los cuatro (no compila ni persiste).
- **La fase no viaja en `TaskStatus.message`**: es texto libre y
  `update_subagent_status` puede pisarlo.

**Archivos:**
- `crates/codeloom-core/src/session/mod.rs`
- `crates/codeloom-core/src/session/sqlite.rs` — `save_subagent` (~314) escribe
  columnas explícitas, así que hay que migrar: `ALTER TABLE subagents ADD COLUMN
  phase TEXT NOT NULL DEFAULT 'queued'`, `last_activity_at_ms INTEGER NOT NULL
  DEFAULT 0`, y `DEFAULT 0` en los contadores.
- Todos los literales `SubagentDescriptor { .. }` del workspace (romperán: es
  intencionado, obliga a revisar cada sitio).

**Test concreto:**
- `contadores_saturan` — no desbordan en `u64::MAX`.
- `migracion_fila_antigua` — fila sin `phase` rehidrata como `Queued` sin error.
- `fase_no_viaja_en_message` — cambiar fase no sobrescribe `TaskStatus.message`.

**Criterio de aceptación:** tests verdes; base existente migra sin pérdida de filas.

**Criterio de aceptación:** tests verdes; sin cambios de API pública que rompan la TUI.

## Fase 3 — Watchdog puro (HECHA 2026-11-05)

Implementada en `subagent_watchdog.rs` y `AgentsConfig`; ver `docs/plans/2026-11-05-codeloom-watchdog-de-subagentes.md`.

**Qué cambia:** módulo puro que, dado `now_ms: u64`, devuelve `Stalled`/`Lost` o
vuelta a la fase previa. Sin reloj real, sin hilos.

**Archivos:**
- `crates/codeloom-core/src/subagent_watchdog.rs` (nuevo)
- `crates/codeloom-core/src/lib.rs`

**Test concreto:**
- `stall_y_recuperacion` — superado `stall_threshold` → `Stalled`; latido → `Working`
  (no "fase previa": `can_transition_to` solo admite `Stalled → Working`; ver
  `docs/specs/2026-11-05-codeloom-watchdog-de-subagentes.md` §8).
- `lost_terminal` — superado `lost_threshold` → `Lost`.
- `no_falso_positivo` — dentro del umbral no transiciona.
- `esperar_al_humano_no_es_atasco` — un hijo en `AwaitingParentInput` o
  `AwaitingPermission` **no** se marca `Stalled`: esperar al humano no es un cuelgue.
  Es el test que evita el falso positivo más probable en producción.

**Criterio de aceptación:** tests puros verdes; sin dependencia de `std::thread`/reloj.

## Fase 4 — Eventos y lista blanca de `is_child_relevant()` (HECHA 2026-11-05, con matices)

**Hecho:** `LiveEvent::SubagentPhaseChanged { parent, child, phase }`, publicado en el bus
del padre por el reenvío de eventos del hijo (`subagent/local_watch.rs`) a partir de un
**observador puro** (`subagent/phase_observer.rs`): traduce `TurnStarted`, `Delta`,
`ReasoningDelta`, `ToolStarted/Finished` en transiciones **legales** (`can_transition_to`),
con salto por `Streaming` si hace falta y una ventana de 500 ms para el vaivén
`Streaming ↔ Reasoning` (a lo sumo 2 eventos/s por hijo). La TUI guarda la fase en el
descriptor sin tocar `status.state` (A2A manda). Tests: `phase_observer` (7),
`local_watch` (2) y `child_tree` (2).

**Matices respecto al texto original (comprobados en el código):**
- `is_child_relevant()` **ya era una enumeración exhaustiva** (sin comodín): añadir una
  variante sin clasificarla no compila, que es más fuerte que `lista_blanca_completa`.
- «El padre se queda ciego» era falso: `SubagentChanged`, `SubagentSettled` y
  `SubagentInputRequested/Resolved` los publica el lanzador **en el bus del padre**; no son
  eventos del hijo y reenviarlos los duplicaría. Lo que faltaba era la fase fina.
- **Hecho:** `phase` y `last_activity_at_ms` se persisten (`SessionStore::update_subagent_runtime`,
  que no toca a un hijo asentado). Los escribe el driver del supervisor: al cambiar la fase y,
  con la fase quieta, un latido como mucho cada 10 s por hijo.
- **Hecho:** `SubagentSettled` lleva `summary: Option<String>`, un extracto de 500 bytes como
  máximo: el motivo si el hijo falló o el texto de su primer artefacto. Lo calcula `local_watch`
  releyendo el hijo ya asentado.
- **Hecho:** `AwaitingPermission`. No hacía falta que el bus del hijo lo expusiera: el padre ya
  sabe que el hijo espera un permiso, porque `ChildResponder::decide` lo pasa a `auth-required`.
  El supervisor lo traduce a `AwaitingPermission` (antes lo marcaba `AwaitingCredentials`, que
  solo aplica a remotos; ahora que estos también se vigilan, el supervisor distingue el origen).
- **Descartado:** `SubagentProgress`, por redundante: los contadores y los artefactos de
  progreso ya suben con `SubagentChanged`.
- **Hecho:** hijos remotos. El supervisor los vigila como a los locales. Su espejo (`remote.rs`)
  publica un `SubagentPhaseChanged` (latido) por cada sondeo bueno, cada evento de flujo y, con
  el flujo abierto y mudo, uno por periodo de sondeo. Sin latidos hay `Stalled` y luego `Lost`;
  el `auth-required` remoto se muestra como `AwaitingCredentials`. El `lost contact` propio del
  espejo (tres sondeos fallidos seguidos) sigue vigente y suele llegar antes.
- **Probado de extremo a extremo (PTY, daemon real y perfil aislado):** un hijo pregunta,
  `Ctrl+G` apunta el composer a él y la respuesta le llega. La prueba destapó un fallo de 6b: el
  look `Crush` (el de por defecto) ignoraba la etiqueta del destinatario y solo `Classic` la
  pintaba. Ahora una fila reservada sobre el composer nombra al hijo en cualquier look.
- **Límite conocido:** con el flujo abierto, la conexión viva cuenta como contacto aunque el
  remoto no diga nada; solo `max_follow` (30 min) acota un remoto mudo con la conexión viva.


**Qué cambia:** `SubagentPhaseChanged` y `SubagentProgress` (coalescido) en
`LiveEvent`; `SubagentSettled` gana `summary` acotado. `is_child_relevant()`
(`event/mod.rs:244`) pasa de lista negra a **lista blanca** explícita.

**Por qué:** hoy deja **fuera** justo los eventos de estado (`JobChanged`,
`SubagentChanged`, `SubagentInputRequested/Resolved`, `SubagentSettled`,
`ChildEvent`). El padre se queda ciego: ese es el origen del "agente perdido".

**Archivos:**
- `crates/codeloom-core/src/event/mod.rs`
- Puntos de emisión en `crates/codeloom-core/src/subagent/` (`local_run.rs`, `remote.rs`, `control.rs`)

**Test concreto:**
- `lista_blanca_completa` — todo evento de estado está clasificado; falla si se añade
  una variante y se olvida clasificarla.
- `estado_de_hijo_llega_al_padre` — `SubagentPhaseChanged` reenviado.
- `progreso_coalescido` — N latidos en ventana corta producen 1 evento.
- `padre_no_se_inunda` — la cola del padre está acotada.

**Criterio de aceptación:** tests verdes; el padre recibe fases; sin inundación.

## Fase 5 — Supervisor del runtime (HECHA 2026-11-05, con matices)

**Hecho:** `subagent/supervisor.rs` (puro: `on_event` + `tick(now_ms)` → `Mark`/`Notify`/
`Cancel`, reutiliza `watchdog::evaluate`; 14 tests), `subagent/supervisor_driver.rs`
(`spawn_supervisor`: alimenta al supervisor desde el bus del padre, tick de 5 s, publica
`SubagentPhaseChanged` y `RuntimeNotice`; 2 tests) y su arranque por agente en
`codeloom-cli` con los umbrales del `[agents]`. `runtime_snapshot` lista cada hijo vivo con
fase y `health=ok|stalled|lost`.

**Decisiones y matices:**
- **`ToolRunning` queda fuera del tick** (alternativa admitida en §8.2 del spec del
  watchdog): una herramienta larga no emite latidos y no hay latido periódico del ejecutor.
- El eco de `Stalled`/`Lost` que el propio supervisor publica **no** cuenta como latido
  (test `lo_que_publica_el_propio_supervisor_no_cuenta_como_latido`).
- `Stalled → Working` lo dispara un evento real del hijo (`ChildEvent`), nunca el tick.
- **Resuelto (2026-11-05): un hijo perdido puede pasar a `failed` y detenerse su trabajo.**
  `[agents] lost_action = "notify"` (defecto desde 2026-11-04 con los remotos ya vigilados: solo
  marcar y avisar) o `"fail"` (un valor desconocido es un error de configuración). Con `fail`, `SupervisorAction::Fail` llega al
  agente (`LostHandler` sobre `Weak<AgentLoop>`, para no mantenerlo vivo) y el lanzador
  (`fail_lost` → `dar_por_perdido`) **primero persiste `failed` con el motivo y luego mata
  el trabajo**: así el asiento lee `failed` y no `canceled`, y el padre recibe
  `SubagentSettled`. Repetirlo sobre un hijo ya asentado no es un error. Se eligió `failed`
  y no `canceled` porque nadie lo canceló: dejó de dar señales.
- El driver vive en el daemon (`codeloom-cli`), no en la TUI: la TUI cliente recibe sus
  eventos por el bus.
- La fase y el latido sí se persisten (ver Fase 4, «Hecho»).
- **Probado de extremo a extremo con un remoto (2026-11-05)**, `codeloom-a2a/tests/supervised_remote.rs`
  (lanzador → espejo → remoto `wiremock` → supervisor): un remoto que sigue contestando al sondeo
  **nunca** se marca `stalled` ni `lost`; uno cuyo `GetTask` se cuelga pasa a `stalled` y luego a
  `lost` con sus avisos y sin tocar la tarea (`MarkAndNotify`); con `FailAndStop` el hijo queda
  `failed` («lost: …», no `canceled`) y el espejo manda `CancelTask` al remoto. Queda por probar,
  con un servidor A2A de otra implementación, solo lo que ya cubre `live_remote.rs` a mano.

**Contenido original de la fase:**

**Qué cambia:** `SubagentSupervisor` en core, **puro** (`on_event(&LiveEvent)` +
`tick(now_ms) -> Vec<SupervisorAction>`), y un driver que inyecta el tick en TUI y
CLI. `runtime_snapshot` (`codeloom-cli/src/server/manager.rs`) incluye fase y salud por
subagente.

**No se inventa `SubagentId`:** el identificador real es `SessionId`
(`session/mod.rs:11`); el descriptor usa `parent: SessionId` / `child: SessionId`.

**Archivos:**
- `crates/codeloom-core/src/supervisor.rs` (nuevo)
- `crates/codeloom-tui/src/run.rs` (driver del tick)
- `crates/codeloom-cli/src/server/manager.rs` (`runtime_snapshot`)

**Test concreto:**
- `agrega_por_arbol` — estado agregado correcto por padre.
- `lost_produce_accion` — `Lost` → `Mark` y `Notify`; `Cancel` **solo si la política lo
  permite** (decisión abierta 2 del spec).
- `supervisor_sin_io` — compila y se testea sin tokio runtime ni red.

**Criterio de aceptación:** tests verdes; `runtime_snapshot` muestra fase y salud, no
solo una lista de ids.

## Fase 6 — Composer compartido (destinatario + borrador por sesión)

**Qué cambia:** destinatario activo, **un borrador por sesión**, despacho de
`submit()` según destino.

**Orden decidido (2026-11-05):** primero **6a** (borrador por sesión), después el troceo
de `app.rs` (Fase 5 del plan de organización) y por último **6b** (destinatario,
`Ctrl+G`, `/focus`).

**6a — HECHA.** `load_session` aparta el borrador (composer + `draft_images`) de la
sesión que se deja y restaura el de la que se abre, en `App::composer_drafts`
(`BTreeMap<SessionId, ComposerDraft>`). El indicador de trabajo y su estilo pasan al
composer entrante porque son del agente, no del borrador. Un borrador vacío no deja
entrada, y la primera carga no toca nada. Seis tests en `app/tests/session.rs`
(`load_session_preserva_borrador_del_padre` y compañeros).

**6b — HECHA (2026-11-05).** Destinatario del composer (`ComposerTarget::{Open, Child}`),
`/focus [id]` y `Ctrl+G` (alterna sesión → hijos vivos → sesión). El despacho es una
función pura sobre el `TaskState` A2A (`child_dispatch`): `input-required` → responde
(`SessionCommand::AnswerSubagent` → `AgentLoop::answer_subagent` → `InputBroker::answer`);
`submitted`/`working` → cola en la TUI (máx. 4 por hijo y 4.000 bytes por mensaje), que se
entrega al pedir entrada y se descarta con aviso si el hijo termina; `auth-required` y
terminales → rechazo con aviso, sin perder el borrador. El composer nombra el destino.
Tests: `app/tests/composer_target.rs`.

**Correcciones al plan original (comprobadas en el código):**
- La **Fase 4 no era precondición**: `TaskTracker::transition` ya publica `SubagentChanged`
  con el `TaskState` en cada cambio y la TUI lo guarda; el despacho solo necesita ese estado.
  La Fase 4 se hizo después, por su propio valor (padre ciego ante eventos del hijo).
- «Asentado → nuevo turno» **no** se hace: en A2A una tarea terminal no se reinicia. Se
  rechaza con aviso y se continúa abriendo el hijo (rail → Enter), que ya es una sesión.
- No hay prueba del extremo daemon (`worker.rs`): es un reenvío fino como `cancel_subagent`.

**El bug que arreglaba** (verificado): `load_session` (`app/session.rs`, `App::load_session`)
**no tocaba `self.composer`** y abrir un hijo **cambia de sesión**. Escribías para el
padre, abrías el hijo y el texto aparecía dentro del hijo.

**Atajos:**
- ~~`Ctrl+T` — alternar destinatario.~~ **Falso (comprobado 2026-11-05):** `Ctrl+T` ya abre
  el panel de herramientas (`run.rs`, `tools_open`), y `Tab` lo usan el foco del rail y los
  diálogos de permiso (`permission_flow.rs`). La tecla del destinatario quedó decidida: **`Ctrl+G`** (ver 6b).
- `/focus <id>` — fijar destinatario.
- `/parent` — volver al padre: **ya existe** (`commands.rs:18` y `:56`); no se añade
  `/back`.

**Archivos:**
- `crates/codeloom-tui/src/app/` (`session.rs`: `load_session` guarda/restaura borrador;
  `composer_target`)
- `crates/codeloom-tui/src/commands.rs` (comando `/focus`)
- `crates/batten/src/state/composer.rs` (borradores, si hace falta)

**Test concreto (TUI, sin red):**
- `load_session_preserva_borrador_del_padre` — **reproduce el bug reportado**.
- `borrador_unico_por_sesion` — conmutar conserva el texto de cada uno.
- `despacho_por_destino` — hijo en curso → cola; `AwaitingParentInput` → responde;
  `AwaitingPermission` → rechaza con aviso; asentado → nuevo turno.
- `focus_cambia_destinatario`.

**Criterio de aceptación:** tests verdes; header y rail muestran el destinatario sin
ambigüedad.

## Dependencias entre fases

```
Fase 0 → Fase 1 → Fase 2 → Fase 3 → Fase 4 → Fase 5
                                      └────→ Fase 6 (requiere Fase 4)
```

La Fase 6 necesita las fases de la Fase 4 para decidir el despacho, y es más segura
**después** de partir `app.rs` según el plan de organización.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Regresión de `is_child_relevant()`: ampliar la lista blanca puede inundar al padre | Coalescing + cota de cola, con test dedicado |
| Falsos positivos del watchdog con modelos lentos | Umbrales conservadores/configurables y `Awaiting*` excluidos (test propio) |
| Migración SQLite rompe bases existentes | `DEFAULT` en toda columna nueva + test de fila antigua |
| Romper los 2.137 tests al tocar `SubagentDescriptor` | Los literales rotos son la señal: se actualizan uno a uno |
| `Unknown` enmascara un fallo real | Proyecta a `Working` (conservador) y se registra en metadatos |
| `App` monolítico (10.769 líneas) al tocar el composer | Cambios mínimos y localizados; partir `app.rs` va en el plan de organización |

## Fuera de alcance

- **Reescritura de `App`** para separar layout/sesión/hijos: va en
  `docs/plans/2026-11-05-codeloom-organizacion-y-navegabilidad.md`. Aquí solo se tocan
  `load_session` y `composer_target` de forma localizada.
- Política de reintento automático (queda en el padre/modelo).
- Memoria persistente alimentada por `SubagentSettled.summary`.
- Cola de mensajes con persistencia entre reinicios.
- Convertir `subagent.rs` en directorio: se decide en el plan de organización.