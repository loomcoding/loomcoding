//! Pregunta al llegar al tope de pasos: en vez de abandonar el trabajo, se ofrece continuar.

use super::stop::StopReason;
use super::AgentLoop;
use crate::event::LiveEvent;
use crate::question::{Question, QuestionOption};
use tokio_util::sync::CancellationToken;

impl AgentLoop {
    /// Avisa del tope alcanzado y pregunta si se continúa. Solo `"Continue"` prorroga; la falta de
    /// persona que responda (modo yolo), cualquier otra respuesta y la cancelación paran. Un subagente
    /// nunca pregunta: su responder encaminaría la pregunta al padre y el hijo quedaría esperando.
    pub(super) async fn ask_to_continue(
        &self,
        reason: StopReason,
        cancel: &CancellationToken,
    ) -> bool {
        if self.budget_family.is_some() {
            return false;
        }
        let motivo = match reason {
            StopReason::StepBudget => "the work-step budget is spent",
            StopReason::ReadBudget => "the read-round budget is spent",
            StopReason::RepeatedToolCalls
            | StopReason::RepeatedOutput
            | StopReason::OutputLimit => return false,
        };
        self.bus.publish(LiveEvent::RuntimeNotice {
            message: format!("step limit reached: {motivo}"),
        });
        let pregunta = [Question {
            header: "Step limit".into(),
            question: format!("The agent stopped because {motivo}. Continue working?"),
            options: vec![
                QuestionOption {
                    label: "Continue".into(),
                    description: "Grant another batch of steps and keep going".into(),
                },
                QuestionOption {
                    label: "Stop".into(),
                    description: "End the turn here".into(),
                },
            ],
            multi_select: false,
        }];
        let respuestas = tokio::select! {
            _ = cancel.cancelled() => None,
            r = self.permission.ask_user(&pregunta) => r,
        };
        respuestas
            .as_ref()
            .and_then(|r| r.first())
            .is_some_and(|r| r.selected == ["Continue"])
    }
}
