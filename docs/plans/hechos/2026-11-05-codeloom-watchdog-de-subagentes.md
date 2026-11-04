# Plan — Watchdog puro de latidos de subagentes (Fase 3)

- **Fecha:** 2026-11-05
- **Spec:** `docs/specs/hechos/2026-11-05-codeloom-watchdog-de-subagentes.md`
- **Plan madre:** `docs/plans/hechos/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md`
- **Ámbito:** solo Fase 3. NO Fase 4 (eventos / `is_child_relevant`), NO Fase 5
  (supervisor, `runtime_snapshot`, driver del tick), NO Fase 6 (composer).

## 0. Nota sobre la pregunta abierta 1 del plan madre

Este plan **cierra la pregunta abierta 1** del spec madre (qué hacer con subagentes sin
latidos): se resuelve con un watchdog de dos niveles (`Stalled` a los 120 s, `Lost` a
los 300 s por defecto) basado en `last_activity_at_ms`, con umbrales globales en
`[agents]` del `codeloom.toml` (`stall_threshold_secs` / `lost_threshold_secs`),
expresados en segundos en TOML y comparados en ms internamente. La pregunta ya está
tachada en el spec madre; al terminar la fase solo falta anotar "Fase 3 hecha" (§6).

## 1. Contexto ya verificado (no re-verificar)

- `crates/codeloom-core/src/subagent_phase.rs` existe (Fase 1): `enum SubagentPhase`
  con 16 variantes, `ALL`, `is_terminal()`, `can_transition_to(next) -> bool`.
  Reglas: de una fase terminal (`SettledOk|SettledErr|Rejected|Canceled|Lost`) no se
  sale; `Stalled` solo admite `Working | Lost | Canceled`; la recuperación
  `Stalled → Working` la dispara un latido real, NO el watchdog; toda fase no terminal
  admite `Canceled`/`Unknown`; `Unknown` no es terminal.
- **Quién puede ir a `Lost`** (líneas 206-264): solo `Working`, `Stalled` y `Unknown`.
  `Queued` no admite ni `Stalled` ni `Lost`; `Starting`/`Streaming`/`Reasoning`/
  `ToolRunning`/`Awaiting*` admiten `Stalled` pero no `Lost` directo. Consecuencia
  asumida (spec §3): las fases de turno llegan a `Lost` en **dos ticks**.
- `SubagentDescriptor` (`crates/codeloom-core/src/session/mod.rs`) ya tiene
  `phase: SubagentPhase` y `last_activity_at_ms: u64`; columnas ya migradas en sqlite.
  **No hay migración pendiente.**
- `crates/codeloom-core/src/lib.rs` ya declara `pub mod subagent_phase;` (línea 23).
- No existe `watchdog`, `stall_threshold` ni `lost_threshold` en el workspace.
- Patrón de configuración a copiar: `AgentsConfig` en
  `crates/codeloom-core/src/config/mod.rs` — `max_active_subagents: Option<usize>`
  (línea 163) + método con `clamp` + constantes `DEFAULT_*`/`MAX_*` (líneas 180-182).
  La clave del TOML **es el nombre del campo**; no hay `serde(rename)`.

## 2. Paso 1 — Configuración (`config/mod.rs`)

En `AgentsConfig`, siguiendo el patrón de `max_active_subagents`:

- Campos: `stall_threshold_secs: Option<u64>`, `lost_threshold_secs: Option<u64>`
  (deserialización tolerante a ausencia, igual que el campo existente). Las claves del
  TOML son exactamente esos nombres.
- Constantes: `DEFAULT_STALL_THRESHOLD_SECS: u64 = 120`,
  `DEFAULT_LOST_THRESHOLD_SECS: u64 = 300`,
  `MIN_STALL_THRESHOLD_SECS: u64 = 10`, `MAX_STALL_THRESHOLD_SECS: u64 = 3600`,
  `MIN_LOST_THRESHOLD_SECS: u64 = 30`, `MAX_LOST_THRESHOLD_SECS: u64 = 21600`,
  `LOST_MARGIN_MS: u64 = 30_000`.
- Métodos (devuelven **ms**, listos para comparar con `last_activity_at_ms`):
  - `stall_threshold_ms() -> u64`:
    `self.stall_threshold_secs.unwrap_or(DEFAULT).clamp(MIN, MAX) * 1000`.
  - `lost_threshold_ms() -> u64`:
    `self.lost_threshold_secs.unwrap_or(DEFAULT).clamp(MIN, MAX) * 1000`, y después
    **normalización estricta**: `max(lost, stall_threshold_ms() + LOST_MARGIN_MS)`
    (un subagente no puede perderse antes de atascarse, y con empate `Stalled` jamás
    sería observable).
- Comentarios en español explicando el porqué de las cotas (ruido por debajo; por
  encima de 1 h/6 h una cancelación manual lo cubre) y del margen.
- Tests en `config/mod.rs`:
  - `umbrales_por_defecto` (120 000 / 300 000 ms).
  - `umbrales_acotados_por_ambos_lados` (por debajo del mínimo y por encima del máximo).
  - `lost_siempre_supera_a_stall`: `stall=3600` con `lost` por defecto →
    `lost_ms == 3_600_000 + LOST_MARGIN_MS`; y `lost <= stall` configurado → idem.
  - `toml_lee_las_claves_del_spec`: parsea `[agents]` con `stall_threshold_secs = 60` y
    `lost_threshold_secs = 200` y comprueba 60 000 / 200 000 (fija que la clave no se
    ignora en silencio). Usar el helper de parseo que ya usan los tests de
    `max_active_subagents` (línea ~690).

## 3. Paso 2 — Módulo puro `subagent_watchdog.rs`

Fichero nuevo `crates/codeloom-core/src/subagent_watchdog.rs`, hermano de
`subagent_phase.rs`; añadir `pub mod subagent_watchdog;` en `lib.rs` (junto a
`subagent_phase`).

**Pureza estricta:** sin `std::thread`, sin `Instant`, sin `SystemTime::now()`, sin
E/S, sin tokio. El "ahora" entra como parámetro.

API:

```rust
/// Qué transición debe aplicar el supervisor a un subagente, o `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogDecision {
    /// Marcar `Stalled` (recuperable con un latido real).
    Stall,
    /// Marcar `Lost` (terminal).
    Lost,
}

pub fn evaluate(
    phase: SubagentPhase,
    last_activity_at_ms: u64,
    now_ms: u64,
    stall_threshold_ms: u64,
    lost_threshold_ms: u64,
) -> Option<WatchdogDecision>
```

Lógica (en este orden):

1. Si `phase.is_terminal()` → `None` (sin mirar tiempos).
2. Si `phase` es `Queued` o `AwaitingPermission | AwaitingParentInput |
   AwaitingCredentials` → `None` (en cola o esperando al humano no es atasco; spec §4).
3. `elapsed_ms = now_ms.saturating_sub(last_activity_at_ms)` — si el reloj fue hacia
   atrás, elapsed = 0 y no hay falso positivo.
4. Si `elapsed_ms >= lost_threshold_ms` y
   `phase.can_transition_to(SubagentPhase::Lost)` → `Some(WatchdogDecision::Lost)`.
5. Si no, si `elapsed_ms >= stall_threshold_ms` y
   `phase.can_transition_to(SubagentPhase::Stalled)` →
   `Some(WatchdogDecision::Stall)`. **Ojo:** desde `Streaming` con `elapsed ≥ lost` se
   cae aquí (no puede ir a `Lost` directo) y sale `Stall`; el `Lost` llega en el tick
   siguiente, ya desde `Stalled`. Está documentado en el spec §3 y se comenta en el
   código.
6. Si no → `None`.

**Reutilización obligatoria de `can_transition_to`** (pasos 4 y 5): el watchdog no
duplica la tabla de transiciones. La recuperación `Stalled → Working` queda fuera de
este módulo: la dispara el latido real (Fase 5; precondiciones en spec §8).

Comentarios en español explicando el POR QUÉ de cada regla, con el estilo de
`subagent_phase.rs` como modelo.

## 4. Paso 3 — Tests (en `subagent_watchdog.rs`, `#[cfg(test)]`)

Constantes de test: `STALL = 120_000`, `LOST = 300_000`, `T0 = 1_000_000`.

Nombres **exactos** exigidos por el plan madre:

- `stall_y_recuperacion` — fase `Working`, pasado `stall` → `Some(Stall)`; y con fase
  `Stalled` + latido fresco se comprueba que `Stalled.can_transition_to(Working)` es
  `true`, documentando que la recuperación la dispara el latido, no el watchdog (el
  madre decía "fase previa"; es `Working`).
- `lost_terminal` — usa fase **`Working`** (o `Stalled`), pasado `lost` →
  `Some(Lost)`. No usar `Streaming`: ahí sale `Stall` (ver el test de dos ticks).
- `no_falso_positivo` — `elapsed < stall` en `Working` y en `Streaming` → `None`.
- `esperar_al_humano_no_es_atasco` — `AwaitingPermission`, `AwaitingParentInput` y
  `AwaitingCredentials` con tiempo muy por encima de ambos umbrales → `None`.

Tests adicionales:

- `bordes_exactos_del_umbral` — en `Working`: `elapsed == STALL` → `Stall`;
  `elapsed == LOST` → `Lost` (comparación `>=`); un ms menos → la decisión inferior.
- `fases_terminales_no_se_evaluan` — las cinco terminales con tiempo enorme → `None`.
- `reloj_hacia_atras_no_da_positivo` — `now_ms < last_activity_at_ms` → `None`.
- `stalled_con_tiempo_enorme_da_lost` — fase `Stalled` → `Lost`, nunca `Stall` repetido.
- `lost_en_dos_ticks_desde_streaming` — `Streaming` con `elapsed > LOST` → `Stall`;
  aplicando esa fase (`Stalled`) con el mismo `elapsed` → `Lost`.
- `cola_no_se_evalua` — `Queued` con tiempo enorme → `None`.
- `sin_latidos_recorre_los_dos_niveles` — barrido de `elapsed` en `Working`: `None` →
  `Stall` → `Lost`, en ese orden y sin saltos hacia atrás.

## 5. Paso 4 — Verificación y commit

**No usar `cargo test` directo** (falla en esta máquina: `Acceso denegado` sobre
`codeloom.exe`, hay daemons vivos con el binario tomado). Usar siempre:

```
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\checks.ps1 -TargetDir .verify-target
```

(`-TargetDir` existe en el script y `.verify-target` está en `.gitignore`.) Los cuatro
checks (build, test, fmt, clippy) deben salir verdes.

Commit único en español, p. ej.:
`watchdog: fase 3 — evaluador puro de latidos de subagentes con umbrales configurables`.

## 6. Estado en el plan madre

Al cerrar la fase, anotar en
`docs/plans/hechos/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md`:
Fase 3 **hecha**, con fecha. (La pregunta abierta 1 ya está marcada como resuelta en el
spec madre y el test `stall_y_recuperacion` ya está corregido a `Working`.)

## 7. Entrega a la Fase 5

Antes de implementar el supervisor hay que cumplir las precondiciones del spec §8:
latido en todo evento, latido periódico durante `ToolRunning` (o excluirlo del tick),
y recuperación a `Working`.

## 8. Fuera de alcance

- Fase 4: eventos de transición, `is_child_relevant`.
- Fase 5: supervisor, `runtime_snapshot`, driver del tick que llama a `evaluate`.
- Fase 6: composer compartido.
- Cualquier cambio en `subagent_phase.rs` (solo se consume) o migraciones sqlite.
