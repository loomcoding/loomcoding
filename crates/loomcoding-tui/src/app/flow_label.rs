//! Etiqueta del modo y flujo activo que muestra la cabecera.
//!
//! La interfaz pinta el texto que llega en `FlowChanged` tal cual y no conoce ningún modo concreto:
//! los modos llegan de la configuración como `FlowMode`. Un flujo no persistente activo
//! (`gauntlet`, `development_workflow` o uno genérico) manda sobre el persistente; cuando acaba,
//! vuelve el persistente. Al acabar el turno, el persistente vuelve a la base de su modo. Es
//! etiqueta propia de Loom, no un estado A2A.

use super::App;
use loomcoding_core::flow::{FlowMode, FlowSummary};
use loomcoding_core::gauntlet::{GauntletPhase, GauntletSummary};
use loomcoding_core::workflow::{WorkflowPhase, WorkflowSummary};

/// Texto de una etiqueta en dos formas: la completa y la corta (sin fase), para cuando la
/// cabecera es estrecha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtiquetaFlujo {
    pub completa: String,
    pub corta: String,
}

impl EtiquetaFlujo {
    /// Etiqueta de un `FlowChanged`. La forma corta es su primer segmento, el nombre del flujo.
    fn de_resumen(resumen: &FlowSummary) -> Self {
        let corta = resumen
            .label
            .split(" · ")
            .next()
            .unwrap_or(&resumen.label)
            .to_string();
        Self {
            completa: resumen.label.clone(),
            corta,
        }
    }

    /// Etiqueta de un flujo pesado: la completa lleva la fase; la corta, solo el nombre y la ronda.
    fn de_gauntlet(resumen: &GauntletSummary) -> Self {
        Self {
            completa: format!(
                "gauntlet #{} · {} · ronda {}/{}",
                resumen.id.0,
                fase_de_gauntlet(resumen.phase),
                resumen.round,
                resumen.max_rounds
            ),
            corta: format!(
                "gauntlet #{} · ronda {}/{}",
                resumen.id.0, resumen.round, resumen.max_rounds
            ),
        }
    }

    fn de_workflow(resumen: &WorkflowSummary) -> Self {
        Self {
            completa: format!(
                "flujo #{} · {} · ronda {}",
                resumen.id.0,
                fase_de_flujo(resumen.phase),
                resumen.correction_round
            ),
            corta: format!(
                "flujo #{} · ronda {}",
                resumen.id.0, resumen.correction_round
            ),
        }
    }
}

/// Base de un modo cuando aún no hay fase: su etiqueta, tal cual la publica la configuración.
fn base_de(modo: &FlowMode) -> EtiquetaFlujo {
    EtiquetaFlujo {
        completa: modo.label.clone(),
        corta: modo.label.clone(),
    }
}

/// Texto de la fase de un gauntlet, el mismo que escribe el transcript.
pub(crate) fn fase_de_gauntlet(fase: GauntletPhase) -> &'static str {
    match fase {
        GauntletPhase::Building => "construyendo",
        GauntletPhase::Judging => "juzgando",
        GauntletPhase::Rebuilding => "rehaciendo",
        GauntletPhase::AwaitingFinalAcceptance => "aceptar resultado",
        GauntletPhase::Completed => "completado",
        GauntletPhase::Stopped => "detenido (rondas agotadas)",
        GauntletPhase::Inconclusive => "sin veredicto fiable",
        GauntletPhase::Cancelled => "cancelado",
        GauntletPhase::Failed => "fallido",
    }
}

/// Texto de la fase de un flujo de desarrollo, el mismo que escribe el transcript.
pub(crate) fn fase_de_flujo(fase: WorkflowPhase) -> &'static str {
    match fase {
        WorkflowPhase::Specifying => "crear spec+plan",
        WorkflowPhase::AwaitingPlanApproval => "aprobar spec+plan",
        WorkflowPhase::Implementing => "implementando",
        WorkflowPhase::Reviewing => "revisando",
        WorkflowPhase::ChangesRequested => "corregir",
        WorkflowPhase::AwaitingFinalAcceptance => "aceptar resultado",
        WorkflowPhase::Completed => "completado",
        WorkflowPhase::Cancelled => "cancelado",
        WorkflowPhase::Failed => "fallido",
    }
}

impl App {
    /// Etiqueta que pinta la cabecera, si la hay: el flujo no persistente, si está activo; si no,
    /// el persistente; si no, la base del primer modo encendido.
    pub fn flow_label(&self) -> Option<&EtiquetaFlujo> {
        self.flow_heavy
            .as_ref()
            .or(self.flow_persistent.as_ref().map(|(_, etiqueta)| etiqueta))
            .or(self.flow_base.as_ref())
    }

    /// Fija los modos de flujo de la configuración. La base de la cabecera es la del primero encendido.
    pub fn with_flow_modes(mut self, modos: Vec<FlowMode>) -> Self {
        self.flow_base = modos.iter().find(|modo| modo.enabled).map(base_de);
        self.flow_modes = modos;
        self
    }

    /// Modo encendido con ese `kind`, si lo hay.
    fn modo_encendido(&self, kind: &str) -> Option<&FlowMode> {
        self.flow_modes
            .iter()
            .find(|modo| modo.enabled && modo.kind == kind)
    }

    /// `FlowChanged`. Un flujo no persistente pinta su etiqueta mientras no termine. Un persistente
    /// fija su fase, o vuelve a la base de su modo si termina; un modo apagado o desconocido se ignora.
    pub(crate) fn aplica_flow_changed(&mut self, resumen: &FlowSummary) {
        if !resumen.persistent {
            self.flow_heavy = (!resumen.terminal).then(|| EtiquetaFlujo::de_resumen(resumen));
            return;
        }
        let Some(modo) = self.modo_encendido(&resumen.kind) else {
            return;
        };
        let etiqueta = if resumen.terminal {
            base_de(modo)
        } else {
            EtiquetaFlujo::de_resumen(resumen)
        };
        self.flow_persistent = Some((resumen.kind.clone(), etiqueta));
    }

    /// Estado de un gauntlet de esta sesión: su etiqueta mientras no termine, ninguna después.
    pub(crate) fn fija_flujo_gauntlet(&mut self, resumen: &GauntletSummary) {
        self.flow_heavy =
            (!resumen.phase.is_terminal()).then(|| EtiquetaFlujo::de_gauntlet(resumen));
    }

    /// Estado de un flujo de desarrollo de esta sesión: su etiqueta mientras no termine.
    pub(crate) fn fija_flujo_workflow(&mut self, resumen: &WorkflowSummary) {
        self.flow_heavy =
            (!resumen.phase.is_terminal()).then(|| EtiquetaFlujo::de_workflow(resumen));
    }

    /// Fin de turno: el flujo persistente vuelve a la base de su modo. Un flujo no persistente activo no se toca.
    pub(crate) fn fin_de_turno_flujo(&mut self) {
        if let Some((kind, _)) = self.flow_persistent.take() {
            self.flow_persistent = self
                .modo_encendido(&kind)
                .map(|modo| (kind.clone(), base_de(modo)));
        }
    }
}
