//! Dueño del estado de la tarea de un hijo y descarte de su pregunta pendiente al terminar.

use super::*;

/// Descarta la pregunta pendiente del hijo al terminar su trabajo, sea cual sea la
/// causa (final normal, cancelación o aborto), para no dejar entradas huérfanas.
pub(super) struct DiscardOnExit {
    pub(super) broker: Arc<InputBroker>,
    pub(super) child: SessionId,
}

impl Drop for DiscardOnExit {
    fn drop(&mut self) {
        self.broker.discard(self.child);
    }
}

/// Único punto del lanzador que cambia el estado de la tarea de un hijo. Valida la
/// transición con `TaskState::can_transition_to`, la persiste y publica `SubagentChanged`.
pub struct TaskTracker {
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) bus: EventBus,
}

impl TaskTracker {
    pub(crate) fn new(store: Arc<dyn SessionStore>, bus: EventBus) -> Self {
        Self { store, bus }
    }

    /// Lleva la tarea del hijo a `status.state`. Si el estado actual no admite la
    /// transición (un terminal no se reinicia) devuelve error y no toca el almacén.
    pub(crate) async fn transition(
        &self,
        parent: SessionId,
        child: SessionId,
        status: TaskStatus,
    ) -> Result<()> {
        let mut descriptor = self.descriptor(parent, child).await?;
        if !descriptor.status.state.can_transition_to(status.state) {
            return Err(CoreError::Store(format!(
                "transición no admitida: {} -> {}",
                descriptor.status.state.proto_name(),
                status.state.proto_name()
            )));
        }
        self.store
            .update_subagent_status(parent, child, status.clone())
            .await?;
        descriptor.status = status;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        Ok(())
    }

    /// Publica el descriptor guardado del hijo tal cual está, p. ej. tras fijar su resultado.
    pub(crate) async fn publish(&self, parent: SessionId, child: SessionId) -> Result<()> {
        let descriptor = self.descriptor(parent, child).await?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        Ok(())
    }

    pub(super) async fn descriptor(
        &self,
        parent: SessionId,
        child: SessionId,
    ) -> Result<SubagentDescriptor> {
        self.store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or_else(|| CoreError::Store(format!("unknown subagent {}", child.0)))
    }
}
