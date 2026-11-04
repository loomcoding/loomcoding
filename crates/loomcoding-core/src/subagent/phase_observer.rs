//! Observador puro de la fase de un hijo local: traduce los eventos de su bus en transiciones de
//! `SubagentPhase`, sin reloj propio, sin E/S y sin inundar al padre.

use crate::event::LiveEvent;
use crate::subagent_phase::SubagentPhase;

/// Ventana mínima entre dos cambios de «sub-fase» del turno (`Streaming` ↔ `Reasoning`). Un modelo
/// que alterna razonamiento y texto sin parar publicaría un evento por delta; con la ventana, a lo
/// sumo dos por segundo y por hijo.
pub const MIN_FLIP_MS: u64 = 500;

/// Estado del observador de un hijo. Guarda la última fase **publicada**.
#[derive(Debug)]
pub struct PhaseObserver {
    phase: SubagentPhase,
    last_flip_ms: Option<u64>,
}

impl Default for PhaseObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseObserver {
    pub fn new() -> Self {
        Self {
            phase: SubagentPhase::Queued,
            last_flip_ms: None,
        }
    }

    /// Última fase publicada.
    pub fn phase(&self) -> SubagentPhase {
        self.phase
    }

    /// Observa un evento del hijo en el instante `now_ms` y devuelve las fases que hay que
    /// publicar, en orden (vacío si no cambia nada o la transición no es legal).
    pub fn observe(&mut self, event: &LiveEvent, now_ms: u64) -> Vec<SubagentPhase> {
        let Some(target) = target_of(event) else {
            return Vec::new();
        };
        if target == self.phase {
            return Vec::new();
        }
        // Camino legal: directo, o con un salto por `Streaming` (p. ej. de `Starting` a `Reasoning`).
        let path: Vec<SubagentPhase> = if self.phase.can_transition_to(target) {
            vec![target]
        } else if self.phase.can_transition_to(SubagentPhase::Streaming)
            && SubagentPhase::Streaming.can_transition_to(target)
        {
            vec![SubagentPhase::Streaming, target]
        } else {
            return Vec::new();
        };
        let mut previous = self.phase;
        let mut flips = false;
        for next in &path {
            flips |= is_turn_subphase(previous) && is_turn_subphase(*next);
            previous = *next;
        }
        if flips {
            if self
                .last_flip_ms
                .is_some_and(|last| now_ms.saturating_sub(last) < MIN_FLIP_MS)
            {
                return Vec::new();
            }
            self.last_flip_ms = Some(now_ms);
        }
        self.phase = target;
        path
    }
}

/// Fase a la que apunta un evento del bus del hijo, si apunta a alguna.
fn target_of(event: &LiveEvent) -> Option<SubagentPhase> {
    match event {
        LiveEvent::TurnStarted => Some(SubagentPhase::Starting),
        LiveEvent::StepStarted { .. }
        | LiveEvent::Delta { .. }
        | LiveEvent::ToolFinished { .. } => Some(SubagentPhase::Streaming),
        LiveEvent::ReasoningDelta { .. } => Some(SubagentPhase::Reasoning),
        LiveEvent::ToolStarted { .. } => Some(SubagentPhase::ToolRunning),
        _ => None,
    }
}

/// Las dos sub-fases del modelo entre las que un turno puede alternar sin parar.
fn is_turn_subphase(phase: SubagentPhase) -> bool {
    matches!(phase, SubagentPhase::Streaming | SubagentPhase::Reasoning)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta() -> LiveEvent {
        LiveEvent::Delta { text: "x".into() }
    }
    fn reasoning() -> LiveEvent {
        LiveEvent::ReasoningDelta { text: "x".into() }
    }
    fn tool_started() -> LiveEvent {
        LiveEvent::ToolStarted {
            call_id: "c".into(),
            name: "glob".into(),
        }
    }
    fn tool_finished() -> LiveEvent {
        LiveEvent::ToolFinished {
            call_id: "c".into(),
            is_error: false,
        }
    }

    #[test]
    fn un_turno_normal_recorre_starting_streaming_y_herramienta() {
        let mut o = PhaseObserver::new();
        assert_eq!(
            o.observe(&LiveEvent::TurnStarted, 0),
            vec![SubagentPhase::Starting]
        );
        assert_eq!(o.observe(&delta(), 1), vec![SubagentPhase::Streaming]);
        assert_eq!(
            o.observe(&tool_started(), 2),
            vec![SubagentPhase::ToolRunning]
        );
        assert_eq!(
            o.observe(&tool_finished(), 3),
            vec![SubagentPhase::Streaming]
        );
        assert_eq!(o.phase(), SubagentPhase::Streaming);
    }

    #[test]
    fn progreso_coalescido_n_deltas_producen_un_solo_evento() {
        let mut o = PhaseObserver::new();
        o.observe(&LiveEvent::TurnStarted, 0);
        let emitidos: usize = (1..=1_000).map(|t| o.observe(&delta(), t).len()).sum();
        assert_eq!(emitidos, 1, "solo el primer delta cambia de fase");
    }

    #[test]
    fn el_padre_no_se_inunda_cuando_razonamiento_y_texto_alternan() {
        let mut o = PhaseObserver::new();
        o.observe(&LiveEvent::TurnStarted, 0);
        o.observe(&delta(), 1);
        let mut emitidos = 0usize;
        for t in 0..10_000u64 {
            let e = if t % 2 == 0 { reasoning() } else { delta() };
            emitidos += o.observe(&e, 2 + t).len();
        }
        // 10 000 ms de alternancia: a lo sumo dos cambios por segundo.
        assert!(emitidos <= 21, "{emitidos} eventos");
        assert!(emitidos >= 1, "los cambios espaciados sí se publican");
    }

    #[test]
    fn los_cambios_de_herramienta_no_se_retrasan_por_la_ventana() {
        let mut o = PhaseObserver::new();
        o.observe(&LiveEvent::TurnStarted, 0);
        o.observe(&delta(), 1);
        assert_eq!(o.observe(&reasoning(), 2).len(), 1);
        assert_eq!(
            o.observe(&tool_started(), 3),
            vec![SubagentPhase::ToolRunning]
        );
    }

    #[test]
    fn razonar_desde_starting_pasa_antes_por_streaming() {
        let mut o = PhaseObserver::new();
        o.observe(&LiveEvent::TurnStarted, 0);
        assert_eq!(
            o.observe(&reasoning(), 1),
            vec![SubagentPhase::Streaming, SubagentPhase::Reasoning]
        );
    }

    #[test]
    fn toda_fase_publicada_es_una_transicion_legal() {
        let eventos = [
            LiveEvent::TurnStarted,
            delta(),
            reasoning(),
            tool_started(),
            tool_finished(),
            LiveEvent::TurnEnded,
        ];
        let mut o = PhaseObserver::new();
        let mut previa = o.phase();
        for t in 0..600u64 {
            for fase in o.observe(&eventos[(t as usize * 7) % eventos.len()], t * 600) {
                assert!(previa.can_transition_to(fase), "{previa:?} -> {fase:?}");
                previa = fase;
            }
        }
    }

    #[test]
    fn los_eventos_que_no_son_de_fase_no_cambian_nada() {
        let mut o = PhaseObserver::new();
        o.observe(&LiveEvent::TurnStarted, 0);
        for e in [
            LiveEvent::TurnEnded,
            LiveEvent::Usage {
                input_tokens: 1,
                output_tokens: 1,
            },
            LiveEvent::Error {
                message: "x".into(),
            },
        ] {
            assert!(o.observe(&e, 1).is_empty(), "{e:?}");
        }
        assert_eq!(o.phase(), SubagentPhase::Starting);
    }
}
