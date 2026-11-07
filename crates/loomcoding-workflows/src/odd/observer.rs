//! Observador de ODD: publica la fase inferida de las herramientas, solo cuando cambia.
//!
//! Comparte el [`OddState`] con la herramienta y la política de cierre. La última fase publicada se
//! recuerda por sesión; con ODD apagado, el observador no publica nada.

use super::phase::phase_of;
use super::state::OddState;
use async_trait::async_trait;
use loomcoding_core::agent::{ToolObserveCtx, ToolObserver};
use loomcoding_core::flow::FlowSummary;

/// Observador de herramientas que traduce las fases de ODD a `FlowSummary`.
pub struct OddObserver {
    estado: OddState,
}

impl OddObserver {
    pub fn new(estado: OddState) -> Self {
        Self { estado }
    }
}

#[async_trait]
impl ToolObserver for OddObserver {
    async fn observe(&self, ctx: &ToolObserveCtx) -> Option<FlowSummary> {
        if !self.estado.is_enabled() {
            return None;
        }
        let fase = phase_of(&ctx.name, &ctx.input)?.as_str();
        if self.estado.fase_publicada(ctx.session) == Some(fase) {
            return None;
        }
        self.estado.recordar_fase(ctx.session, fase);
        Some(FlowSummary {
            kind: "odd".into(),
            id: 0,
            phase: fase.into(),
            round: 0,
            max_rounds: 0,
            terminal: false,
            label: format!("ODD · {fase}"),
        })
    }
}
