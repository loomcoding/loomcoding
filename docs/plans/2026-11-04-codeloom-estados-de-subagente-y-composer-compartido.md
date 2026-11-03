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

## Fase 1 — `SubagentPhase` y mapeos A2A (core puro) — **prioridad A2A**

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

## Fase 2 — Estado de observabilidad en `SubagentDescriptor`

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

## Fase 3 — Watchdog puro

**Qué cambia:** módulo puro que, dado `now_ms: u64`, devuelve `Stalled`/`Lost` o
vuelta a la fase previa. Sin reloj real, sin hilos.

**Archivos:**
- `crates/codeloom-core/src/subagent_watchdog.rs` (nuevo)
- `crates/codeloom-core/src/lib.rs`

**Test concreto:**
- `stall_y_recuperacion` — superado `stall_threshold` → `Stalled`; latido → fase previa.
- `lost_terminal` — superado `lost_threshold` → `Lost`.
- `no_falso_positivo` — dentro del umbral no transiciona.
- `esperar_al_humano_no_es_atasco` — un hijo en `AwaitingParentInput` o
  `AwaitingPermission` **no** se marca `Stalled`: esperar al humano no es un cuelgue.
  Es el test que evita el falso positivo más probable en producción.

**Criterio de aceptación:** tests puros verdes; sin dependencia de `std::thread`/reloj.

## Fase 4 — Eventos y lista blanca de `is_child_relevant()`

**Qué cambia:** `SubagentPhaseChanged` y `SubagentProgress` (coalescido) en
`LiveEvent`; `SubagentSettled` gana `summary` acotado. `is_child_relevant()`
(`event/mod.rs:244`) pasa de lista negra a **lista blanca** explícita.

**Por qué:** hoy deja **fuera** justo los eventos de estado (`JobChanged`,
`SubagentChanged`, `SubagentInputRequested/Resolved`, `SubagentSettled`,
`ChildEvent`). El padre se queda ciego: ese es el origen del "agente perdido".

**Archivos:**
- `crates/codeloom-core/src/event/mod.rs`
- Puntos de emisión en `crates/codeloom-core/src/subagent.rs`

**Test concreto:**
- `lista_blanca_completa` — todo evento de estado está clasificado; falla si se añade
  una variante y se olvida clasificarla.
- `estado_de_hijo_llega_al_padre` — `SubagentPhaseChanged` reenviado.
- `progreso_coalescido` — N latidos en ventana corta producen 1 evento.
- `padre_no_se_inunda` — la cola del padre está acotada.

**Criterio de aceptación:** tests verdes; el padre recibe fases; sin inundación.

## Fase 5 — Supervisor del runtime

**Qué cambia:** `SubagentSupervisor` en core, **puro** (`on_event(&LiveEvent)` +
`tick(now_ms) -> Vec<SupervisorAction>`), y un driver que inyecta el tick en TUI y
CLI. `runtime_snapshot` (`codeloom-cli/src/server.rs:1623`) incluye fase y salud por
subagente.

**No se inventa `SubagentId`:** el identificador real es `SessionId`
(`session/mod.rs:11`); el descriptor usa `parent: SessionId` / `child: SessionId`.

**Archivos:**
- `crates/codeloom-core/src/supervisor.rs` (nuevo)
- `crates/codeloom-tui/src/run.rs` (driver del tick)
- `crates/codeloom-cli/src/server.rs` (`runtime_snapshot`)

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

**El bug que arregla** (verificado): `load_session`
(`crates/codeloom-tui/src/app.rs:2802`) **no toca `self.composer`** y abrir un hijo
**cambia de sesión**. Escribes para el padre, abres el hijo y el texto aparece dentro
del hijo.

**Atajos:**
- `Ctrl+T` — alternar destinatario. **No `Tab`**: ya lo usan el foco del rail
  (`run.rs`) y los diálogos de permiso (`permission_flow.rs`).
- `/focus <id>` — fijar destinatario.
- `/parent` — volver al padre: **ya existe** (`commands.rs:18` y `:56`); no se añade
  `/back`.

**Archivos:**
- `crates/codeloom-tui/src/app.rs` (`load_session` guarda/restaura borrador;
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