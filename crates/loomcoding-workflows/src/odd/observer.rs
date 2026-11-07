//! Observador de ODD: publica la fase inferida de las herramientas, solo cuando cambia.
//!
//! Comparte el [`OddState`] con la herramienta y la política de cierre. La última publicación se
//! recuerda por sesión; la clave para no republicar es la etiqueta completa, que lleva el avance.
//! Con ODD apagado, el observador no publica nada.

use super::phase::phase_of;
use super::state::OddState;
use async_trait::async_trait;
use loomcoding_core::agent::{ToolObserveCtx, ToolObserver};
use loomcoding_core::flow::FlowSummary;

/// Resumen de un flujo de ODD: fase y avance. Lo construyen el observador y la herramienta
/// `odd_track`, así el formato de la etiqueta vive en un solo sitio. Sin documento, `avance` es
/// `None` y la etiqueta es «ODD · {fase}»; con documento, «ODD · {fase} ({hechas}/{total})».
pub fn resumen_de_flujo(fase: &str, avance: Option<(usize, usize)>) -> FlowSummary {
    let (hechas, total) = avance.unwrap_or((0, 0));
    let label = if total > 0 {
        format!("ODD · {fase} ({hechas}/{total})")
    } else {
        format!("ODD · {fase}")
    };
    FlowSummary {
        kind: "odd".into(),
        id: 0,
        phase: fase.into(),
        round: u8::try_from(hechas).unwrap_or(u8::MAX),
        max_rounds: u8::try_from(total).unwrap_or(u8::MAX),
        terminal: false,
        label,
    }
}

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
        let resumen = resumen_de_flujo(fase, self.estado.progreso_de(ctx.session));
        let ya_publicada = self
            .estado
            .publicado(ctx.session)
            .is_some_and(|p| p.etiqueta == resumen.label);
        if ya_publicada {
            return None;
        }
        self.estado
            .recordar_publicado(ctx.session, fase, &resumen.label);
        Some(resumen)
    }
}
