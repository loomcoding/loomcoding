# Spec — Watchdog puro de latidos de subagentes (Fase 3)

- **Fecha:** 2026-11-05
- **Estado:** propuesta (pendiente de implementar)
- **Plan madre:** `docs/plans/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md` (Fase 3)
- **Ámbito:** solo Fase 3. Quedan fuera la Fase 4 (eventos / `is_child_relevant`), la Fase 5 (supervisor, `runtime_snapshot`, driver del tick) y la Fase 6 (composer).

## 1. Problema

Un subagente puede quedarse colgado (sin latidos) y hoy nadie lo detecta: el descriptor
lleva `phase: SubagentPhase` y `last_activity_at_ms: u64` (verificado en
`crates/codeloom-core/src/session/mod.rs`), pero no hay nada que compare el tiempo
transcurrido con umbrales y decida una transición. Esta fase añade esa decisión como
**función pura**, sin hilos, sin reloj y sin E/S, para que la Fase 5 (supervisor) solo
tenga que llamarla con `now_ms` y aplicar el resultado.

## 2. Decisiones cerradas (con el usuario)

1. **Umbrales globales**, en la sección `[agents]` del `codeloom.toml`. No por subagente.
2. **Defectos:** `stall_threshold` = **120 s**, `lost_threshold` = **300 s**.
3. **Unidades:** en TOML se expresan en **segundos** (legibles); internamente todas las
   comparaciones son en **milisegundos**, porque `last_activity_at_ms` es epoch en ms.
4. **Cotas** (ver §5).
5. Esta decisión cierra la **pregunta abierta 1** del spec madre (qué hacer con
   subagentes sin latidos): se resuelve con un watchdog de dos niveles (Stalled → Lost)
   basado en `last_activity_at_ms`. El plan lo anota explícitamente.

## 3. Semántica de los dos niveles

Dado `elapsed_ms = now_ms - last_activity_at_ms` (saturado a 0 si el reloj fue hacia
atrás, ver §6):

- `elapsed_ms >= lost_threshold` → transición a **`SubagentPhase::Lost`** (terminal).
- `elapsed_ms >= stall_threshold` (y no aplica Lost) → transición a
  **`SubagentPhase::Stalled`** (no terminal).
- En otro caso → `None` (no se hace nada).

**`Lost` en dos ticks para las fases de turno.** La tabla de `can_transition_to` solo
admite `→ Lost` desde `Working`, `Stalled` y `Unknown`; desde `Starting`, `Streaming`,
`Reasoning` y `ToolRunning` solo se llega a `Stalled`. Decisión cerrada: **no se toca la
tabla**. Un subagente en `Streaming` con 400 s de silencio recibe `Stall` en el primer
tick y `Lost` en el siguiente (ya desde `Stalled`, con el mismo `elapsed`). Es deseable:
`Lost` es terminal e irrecuperable, y pasar siempre por `Stalled` da una ventana en la
que un latido tardío aún puede rescatar al subagente.

`Stalled` es **recuperable**: un latido real (`last_activity_at_ms` fresco) permite
`Stalled → Working`; esa recuperación la dispara el latido, **nunca** el watchdog.
Esto ya lo garantiza `SubagentPhase::can_transition_to` en
`crates/codeloom-core/src/subagent_phase.rs` (Fase 1), que es la única fuente de
verdad de transiciones: el watchdog **reutiliza** `can_transition_to` y no duplica la
tabla.

## 4. Excepciones: esperar al humano no es atasco

Las fases `AwaitingPermission` y `AwaitingParentInput` **no** se marcan `Stalled` ni
`Lost` por el watchdog: son esperas al ritmo del humano, que puede tardar horas sin que
haya ningún fallo. Marcarlas sería un falso positivo sistemático. El padre siempre
puede cancelarlas manualmente (`Canceled`), que es la salida correcta para "lleva
demasiado esperando".

**`AwaitingCredentials`: decisión cerrada — también se excluye** del Stalled y del
Lost. Justificación: es, igual que las otras dos, una espera al ritmo del humano
(introducir una clave/credencial); el usuario puede tardar lo que sea en abrirla, y un
falso positivo aquí ensuciaría el estado justo cuando el subagente está pidiendo algo
al usuario. Coherencia: toda espera de humano queda fuera del watchdog.

**`Queued` tampoco se evalúa:** un subagente en cola espera un hueco de
`max_active_subagents`, no está colgado, y la tabla ya no le permite ir a `Stalled`/`Lost`.
El watchdog devuelve `None` por esa vía (hay test que lo fija).

Las fases terminales (`SettledOk | SettledErr | Rejected | Canceled | Lost`) **no se
evalúan**: `can_transition_to` ya impide salir de ellas, y el watchdog lo respeta
devolviendo `None` sin llegar a comparar tiempos.

## 5. Configuración y cotas

Se sigue el patrón existente de `AgentsConfig` en `crates/codeloom-core/src/config/mod.rs`
(`max_active_subagents: Option<usize>` + método con `clamp` + constantes `DEFAULT_*`):

```toml
[agents]
stall_threshold_secs = 120   # segundos
lost_threshold_secs  = 300   # segundos
lost_action          = "fail" # "fail" (defecto: asienta `failed` y detiene el trabajo) | "notify"
```

La clave del TOML es **idéntica** al campo (`_secs` incluido), como `max_active_subagents`:
sin `#[serde(rename)]`, una clave distinta se ignoraría en silencio.

| Campo | Tipo en `AgentsConfig` | Defecto | Cota |
|---|---|---|---|
| `stall_threshold_secs` | `Option<u64>` | 120 | `clamp(10, 3600)` |
| `lost_threshold_secs` | `Option<u64>` | 300 | `clamp(30, 21600)` |

Justificación de las cotas (una línea): por debajo de 10 s/30 s el watchdog dispara por
ruido de planificación o una pausa de red normal; por encima de 1 h/6 h ya no aporta
nada que una cancelación manual no cubra.

**Invariantes:**

- `lost_threshold > stall_threshold` **estrictamente**: si tras aplicar cotas el `lost`
  resultante no supera al `stall` en al menos `LOST_MARGIN_MS` (30 000 ms), se normaliza
  a `stall + LOST_MARGIN_MS`. Con un empate, `Lost` ganaría en el mismo instante y
  `Stalled` jamás sería observable; el margen garantiza la ventana de recuperación.
- Los métodos de acceso (`stall_threshold_ms()`, `lost_threshold_ms()`) devuelven ya
  **milisegundos** listos para comparar con `last_activity_at_ms`.

## 6. Casos de borde

- **Bordes exactos del umbral:** `elapsed_ms == stall_threshold` SÍ marca `Stalled`;
  `elapsed_ms == lost_threshold` SÍ marca `Lost` (comparación `>=`).
- **Reloj hacia atrás:** `now_ms < last_activity_at_ms` (NTP, suspensión, VM restaurada)
  → elapsed se satura a 0 → `None`. Nunca un falso positivo por retroceso de reloj.
- **Fase terminal:** `None` inmediato, sin mirar tiempos.
- **Sin latidos nunca:** un subagente sin actividad supera primero `stall` y después
  `lost`; la evaluación es sin estado, así que cada tick recalcula desde cero.

## 7. Módulo y restricciones

- Fichero nuevo: `crates/codeloom-core/src/subagent_watchdog.rs`, hermano de
  `subagent_phase.rs`, declarado en `crates/codeloom-core/src/lib.rs`.
- **Pureza estricta:** sin `std::thread`, sin `Instant`, sin `SystemTime::now()`, sin
  E/S, sin tokio. El "ahora" entra como parámetro `now_ms: u64`.
- `codeloom-core` sigue sin depender de `ratatui` ni `crossterm`.
- Sin red en tests; comentarios en español explicando el POR QUÉ, con el estilo de
  `subagent_phase.rs` como modelo.

## 8. Precondiciones para la Fase 5 (supervisor)

El watchdog solo es tan bueno como `last_activity_at_ms`. La Fase 5 debe garantizar:

1. **Todo evento observable del subagente refresca `last_activity_at_ms`**: tokens,
   razonamiento, inicio y fin de herramienta, y cambios de fase.
2. **`ToolRunning` con herramientas largas** (un `cargo build` de minutos no emite
   tokens): mientras una herramienta corre, el ejecutor refresca la marca con un latido
   periódico (≤ `stall_threshold / 2`). Sin esto, una herramienta legítima larga acabaría
   en `Lost`. Si no es viable, la Fase 5 debe excluir `ToolRunning` del tick.
3. **`Stalled → Working`** la dispara ese latido, nunca el watchdog.
4. **Recuperación a `Working`, no a la fase previa:** el plan madre decía "latido → fase
   previa", pero `can_transition_to` solo admite `Stalled → Working`; el madre se corrige
   en consecuencia.

## 9. Preguntas abiertas

Ninguna. Las que había (umbrales, unidades, cotas, `AwaitingCredentials`) están
cerradas en §2, §4 y §5.
