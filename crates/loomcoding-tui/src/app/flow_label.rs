//! Etiqueta del modo y flujo activo que muestra la cabecera.
//!
//! La interfaz pinta el texto que llega en `FlowChanged` tal cual: no conoce ODD ni depende de
//! `loomcoding-workflows`. Un flujo pesado activo (`gauntlet`, `development_workflow`) manda sobre
//! la etiqueta de ODD; cuando acaba, vuelve la de ODD. Es etiqueta propia de Loom, no un estado A2A.

use super::App;
use loomcoding_core::config::OddConfig;
use loomcoding_core::flow::FlowSummary;
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

/// Base de ODD cuando aún no hay fase: «ODD».
fn base_odd() -> EtiquetaFlujo {
    EtiquetaFlujo {
        completa: "ODD".into(),
        corta: "ODD".into(),
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
    /// Etiqueta que pinta la cabecera, si la hay. Un flujo pesado activo manda sobre ODD.
    pub fn flow_label(&self) -> Option<&EtiquetaFlujo> {
        self.flow_heavy.as_ref().or(self.flow_odd.as_ref())
    }

    /// Aplica la sección `[odd]` de la configuración. Encendido, la base «ODD» aparece en la cabecera.
    pub fn with_odd_config(mut self, odd: OddConfig) -> Self {
        self.set_flows_odd_enabled(odd.enabled);
        self
    }

    fn set_flows_odd_enabled(&mut self, on: bool) {
        self.flows_odd_enabled = on;
        self.flow_odd = on.then(base_odd);
    }

    /// `FlowChanged`: ODD fija su fase (o vuelve a su base si termina). Cualquier otro flujo
    /// genérico pinta su etiqueta mientras no termine.
    pub(crate) fn aplica_flow_changed(&mut self, resumen: &FlowSummary) {
        if resumen.kind == "odd" {
            if self.flows_odd_enabled {
                self.flow_odd = Some(if resumen.terminal {
                    base_odd()
                } else {
                    EtiquetaFlujo::de_resumen(resumen)
                });
            }
        } else {
            self.flow_heavy = (!resumen.terminal).then(|| EtiquetaFlujo::de_resumen(resumen));
        }
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

    /// Fin de turno: ODD vuelve a su base sin fase. Un flujo pesado activo no se toca.
    pub(crate) fn fin_de_turno_flujo(&mut self) {
        if self.flows_odd_enabled {
            self.flow_odd = Some(base_odd());
        }
    }
}
