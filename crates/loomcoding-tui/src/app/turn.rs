//! Despertar por hijos, compactación, cancelación y presupuesto de turnos automáticos.

use super::*;

impl App {
    /// Decide si debe pedirse un despertar del padre ahora. Solo lo pide con el
    /// padre ocioso: sin turno, sin diálogos, sin colas ni ajustes en curso y con
    /// presupuesto. Si el presupuesto se agotó, avisa una vez y descarta el despertar.
    pub fn take_wake(&mut self) -> bool {
        if !self.wake_pending
            || self.wake_in_flight
            || self.compact_requested
            || self.compact_in_flight
            || self.turn_in_flight
            || !self.pending_prompts.is_empty()
            || self.permission.is_some()
            || self.question.is_some()
            || self.queue_open
            || self.queue_paused
            || self.session_loading
            || self.session_picker_open
            || self.settings_open
            || self.settings_saving
            || self.a2a_open
        {
            return false;
        }
        if self.auto_turns_left == 0 {
            self.wake_pending = false;
            // Las causas descartadas no deben contarse en el próximo despertar.
            self.wake_causes.clear();
            self.status = "auto-turns exhausted — write to continue".into();
            return false;
        }
        self.wake_pending = false;
        self.wake_in_flight = true;
        true
    }

    /// Motivo por el que `/compact` no puede pedirse ahora, o `None` si puede. Exige una sesión
    /// abierta y que no haya turno, despertar ni otra compactación en curso.
    pub fn compact_blocked(&self) -> Option<&'static str> {
        if self.session_id.is_none() {
            Some("No session to compact")
        } else if self.turn_in_flight || self.wake_in_flight {
            Some("Wait for the active turn before compacting")
        } else if self.compact_requested || self.compact_in_flight {
            Some("Already compacting")
        } else {
            None
        }
    }

    /// Pide una compactación manual. La entrega al trabajador `take_compact`. Devuelve `false`,
    /// con el motivo en `slash_feedback`, si no puede pedirse ahora.
    pub fn request_compact(&mut self) -> bool {
        if let Some(motivo) = self.compact_blocked() {
            self.slash_feedback = Some(motivo.into());
            return false;
        }
        self.compact_requested = true;
        self.slash_feedback = Some("Compacting…".into());
        true
    }

    /// `true` una sola vez por petición: pasa de «pedida» a «en vuelo» y el bucle debe avisar al
    /// trabajador.
    pub fn take_compact(&mut self) -> bool {
        if !self.compact_requested {
            return false;
        }
        self.compact_requested = false;
        self.compact_in_flight = true;
        true
    }

    /// Rearma el presupuesto de turnos automáticos; lo llama el envío de cada
    /// mensaje del usuario.
    pub fn rearm_auto_turns(&mut self) {
        self.auto_turns_left = MAX_AUTO_TURNS;
    }

    /// Cancelación pedida por el usuario: un despertar pendiente deja de tener sentido.
    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
        self.wake_pending = false;
        self.wake_causes.clear();
    }

    /// Línea de interfaz que explica el último despertar del padre. Con varias causas
    /// pendientes muestra la más reciente y cuántas más quedan. Un hijo que no se conoce
    /// se nombra por su identificador de agente.
    pub(super) fn wake_cause_line(&self) -> Option<String> {
        let (child, motivo) = *self.wake_causes.last()?;
        let etiqueta = self
            .subagents
            .get(&child)
            .map(|subagent| subagent.label.clone())
            .unwrap_or_else(|| format!("agent #{child}"));
        let mas = self.wake_causes.len() - 1;
        let sufijo = if mas > 0 {
            format!(" (+{mas} more)")
        } else {
            String::new()
        };
        Some(format!(
            "⟳ auto-turn · woken by «{etiqueta}» ({motivo}){sufijo}"
        ))
    }
}
