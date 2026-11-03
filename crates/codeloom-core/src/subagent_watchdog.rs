//! Vigilante puro de latidos de subagentes: decide si un subagente sin señal debe
//! marcarse `Stalled` o `Lost`.
//!
//! Cabe señalar que el módulo es una función pura: sin hilos, sin reloj y sin E/S. El
//! «ahora» entra como parámetro, de modo que el supervisor (Fase 5) solo tiene que
//! llamar a [`evaluate`] en cada tick y aplicar el resultado. La recuperación
//! `Stalled → Working` NO vive aquí: la dispara un latido real, nunca el vigilante.

use crate::subagent_phase::SubagentPhase;

/// Transición que el supervisor debe aplicar a un subagente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogDecision {
    /// Marcar `Stalled`: recuperable con un latido real.
    Stall,
    /// Marcar `Lost`: terminal.
    Lost,
}

/// Evalúa un subagente y devuelve la transición a aplicar, o `None` si no hay nada que hacer.
///
/// Los umbrales llegan ya en milisegundos (`AgentsConfig::stall_threshold_ms` y
/// `lost_threshold_ms`), la misma unidad que `last_activity_at_ms`.
pub fn evaluate(
    phase: SubagentPhase,
    last_activity_at_ms: u64,
    now_ms: u64,
    stall_threshold_ms: u64,
    lost_threshold_ms: u64,
) -> Option<WatchdogDecision> {
    // De una fase terminal no se sale; salir antes evita comparar tiempos en balde.
    if phase.is_terminal() {
        return None;
    }
    // Dícese de las esperas que no son atasco: en cola se aguarda un hueco, y las
    // peticiones al humano (permiso, entrada del padre, credenciales) van a su ritmo y
    // pueden tardar horas sin fallo alguno. Marcarlas sería un falso positivo sistemático.
    if matches!(
        phase,
        SubagentPhase::Queued
            | SubagentPhase::AwaitingPermission
            | SubagentPhase::AwaitingParentInput
            | SubagentPhase::AwaitingCredentials
    ) {
        return None;
    }
    // Saturación: si el reloj fue hacia atrás (NTP, suspensión, VM restaurada), el
    // tiempo transcurrido es 0 y jamás hay un falso positivo.
    let elapsed_ms = now_ms.saturating_sub(last_activity_at_ms);

    // Se consulta `can_transition_to` en vez de duplicar la tabla de transiciones: es
    // la única fuente de verdad. Ojo: desde las fases de turno (`Streaming`, etc.) no
    // se puede ir a `Lost` directamente, así que caen al `Stall` de abajo y el `Lost`
    // llega en el tick siguiente, ya desde `Stalled`. Es deseable: `Lost` es
    // irrecuperable y pasar por `Stalled` deja una ventana para un latido tardío.
    if elapsed_ms >= lost_threshold_ms && phase.can_transition_to(SubagentPhase::Lost) {
        return Some(WatchdogDecision::Lost);
    }
    if elapsed_ms >= stall_threshold_ms
        && phase != SubagentPhase::Stalled
        && phase.can_transition_to(SubagentPhase::Stalled)
    {
        return Some(WatchdogDecision::Stall);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const STALL: u64 = 120_000;
    const LOST: u64 = 300_000;
    const T0: u64 = 1_000_000;

    fn eval(phase: SubagentPhase, elapsed: u64) -> Option<WatchdogDecision> {
        evaluate(phase, T0, T0 + elapsed, STALL, LOST)
    }

    #[test]
    fn stall_y_recuperacion() {
        assert_eq!(
            eval(SubagentPhase::Working, STALL + 1),
            Some(WatchdogDecision::Stall)
        );
        // La recuperación la dispara un latido real, no el vigilante: aquí solo se
        // comprueba que la tabla la admite (el plan madre decía «fase previa»; es `Working`).
        assert!(SubagentPhase::Stalled.can_transition_to(SubagentPhase::Working));
    }

    #[test]
    fn lost_terminal() {
        assert_eq!(
            eval(SubagentPhase::Working, LOST + 1),
            Some(WatchdogDecision::Lost)
        );
        assert_eq!(
            eval(SubagentPhase::Stalled, LOST + 1),
            Some(WatchdogDecision::Lost)
        );
    }

    #[test]
    fn no_falso_positivo() {
        assert_eq!(eval(SubagentPhase::Working, STALL - 1), None);
        assert_eq!(eval(SubagentPhase::Streaming, STALL - 1), None);
        assert_eq!(eval(SubagentPhase::Working, 0), None);
    }

    #[test]
    fn esperar_al_humano_no_es_atasco() {
        for phase in [
            SubagentPhase::AwaitingPermission,
            SubagentPhase::AwaitingParentInput,
            SubagentPhase::AwaitingCredentials,
        ] {
            assert_eq!(eval(phase, LOST * 100), None, "{phase:?}");
        }
    }

    #[test]
    fn bordes_exactos_del_umbral() {
        assert_eq!(eval(SubagentPhase::Working, STALL - 1), None);
        assert_eq!(
            eval(SubagentPhase::Working, STALL),
            Some(WatchdogDecision::Stall)
        );
        assert_eq!(
            eval(SubagentPhase::Working, LOST - 1),
            Some(WatchdogDecision::Stall)
        );
        assert_eq!(
            eval(SubagentPhase::Working, LOST),
            Some(WatchdogDecision::Lost)
        );
    }

    #[test]
    fn fases_terminales_no_se_evaluan() {
        for phase in SubagentPhase::ALL.into_iter().filter(|p| p.is_terminal()) {
            assert_eq!(eval(phase, LOST * 100), None, "{phase:?}");
        }
    }

    #[test]
    fn reloj_hacia_atras_no_da_positivo() {
        assert_eq!(
            evaluate(SubagentPhase::Working, T0, T0 - 5_000, STALL, LOST),
            None
        );
        assert_eq!(
            evaluate(SubagentPhase::Working, u64::MAX, 0, STALL, LOST),
            None
        );
    }

    #[test]
    fn stalled_con_tiempo_enorme_da_lost() {
        assert_eq!(
            eval(SubagentPhase::Stalled, LOST * 100),
            Some(WatchdogDecision::Lost)
        );
        // Y ya atascado, entre ambos umbrales no se repite `Stall`.
        assert_eq!(eval(SubagentPhase::Stalled, STALL + 1), None);
    }

    #[test]
    fn lost_en_dos_ticks_desde_streaming() {
        // Primer tick: desde `Streaming` no se puede ir a `Lost`, sale `Stall`.
        assert_eq!(
            eval(SubagentPhase::Streaming, LOST + 1),
            Some(WatchdogDecision::Stall)
        );
        // Segundo tick: ya en `Stalled`, con el mismo tiempo, sale `Lost`.
        assert_eq!(
            eval(SubagentPhase::Stalled, LOST + 1),
            Some(WatchdogDecision::Lost)
        );
    }

    #[test]
    fn cola_no_se_evalua() {
        assert_eq!(eval(SubagentPhase::Queued, LOST * 100), None);
    }

    #[test]
    fn sin_latidos_recorre_los_dos_niveles() {
        // Barrido en `Working`: None → Stall → Lost, en ese orden y sin retrocesos.
        let mut nivel = 0u8;
        for elapsed in (0..=LOST * 2).step_by(1_000) {
            let actual = match eval(SubagentPhase::Working, elapsed) {
                None => 0,
                Some(WatchdogDecision::Stall) => 1,
                Some(WatchdogDecision::Lost) => 2,
            };
            assert!(actual >= nivel, "retroceso en elapsed={elapsed}");
            nivel = actual;
        }
        assert_eq!(nivel, 2);
    }
}
