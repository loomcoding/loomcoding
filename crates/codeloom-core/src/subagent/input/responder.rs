//! Responder de un hijo: delega los permisos en el padre y encamina sus preguntas al buzón,
//! llevando al hijo a `auth-required` o `input-required` mientras espera.

use crate::event::{EventBus, LiveEvent};
use crate::permission::{Decision, PermissionRequest, PermissionResponder, PermissionService};
use crate::question::{Answer, Question};
use crate::session::SessionId;
use crate::subagent::{resumen_de_permiso, TaskTracker};
use crate::task::{TaskState, TaskStatus};
use async_trait::async_trait;
use std::sync::Arc;

use super::InputBroker;

/// Responder de un hijo. Los permisos se delegan en el servicio del padre, de modo
/// que comparten decisiones y caché; las preguntas (`ask_user`) se encaminan al
/// buzón y se anuncian en el bus como `SubagentInputRequested` / `SubagentInputResolved`.
pub struct ChildResponder {
    parent_service: Arc<PermissionService>,
    broker: Arc<InputBroker>,
    parent: SessionId,
    child: SessionId,
    label: String,
    bus: EventBus,
    tracker: Arc<TaskTracker>,
}

impl ChildResponder {
    pub fn new(
        parent_service: Arc<PermissionService>,
        broker: Arc<InputBroker>,
        parent: SessionId,
        child: SessionId,
        label: String,
        bus: EventBus,
        tracker: Arc<TaskTracker>,
    ) -> Self {
        Self {
            parent_service,
            broker,
            parent,
            child,
            label,
            bus,
            tracker,
        }
    }
}

/// Devuelve el hijo a `Working` al cerrarse la espera. Si el futuro que esperaba se suelta
/// antes de tiempo (cancelación o aborto), no hay `await` disponible: la transición se lanza
/// en segundo plano. Si la tarea ya terminó, el almacén la rechaza sin daño.
struct VuelveAWorking {
    tracker: Arc<TaskTracker>,
    parent: SessionId,
    child: SessionId,
    armado: bool,
}

impl VuelveAWorking {
    fn armar(tracker: &Arc<TaskTracker>, parent: SessionId, child: SessionId) -> Self {
        Self {
            tracker: tracker.clone(),
            parent,
            child,
            armado: true,
        }
    }

    /// Cierre normal de la espera: la transición se espera y la guarda deja de actuar.
    async fn resolver(&mut self) {
        self.armado = false;
        if let Err(error) = self
            .tracker
            .transition(self.parent, self.child, TaskStatus::new(TaskState::Working))
            .await
        {
            tracing::warn!("persist subagent status: {error}");
        }
    }
}

impl Drop for VuelveAWorking {
    fn drop(&mut self) {
        if !self.armado {
            return;
        }
        // Fuera de un runtime no hay a quién encargar la transición: se omite.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let tracker = self.tracker.clone();
        let (parent, child) = (self.parent, self.child);
        handle.spawn(async move {
            if let Err(error) = tracker
                .transition(parent, child, TaskStatus::new(TaskState::Working))
                .await
            {
                tracing::debug!("subagent already settled: {error}");
            }
        });
    }
}

/// Publica `SubagentInputResolved` al soltarse, de forma que la marca de espera
/// desaparezca también cuando la pregunta se cancela a mitad.
struct ResolvedOnDrop {
    bus: EventBus,
    parent: SessionId,
    child: SessionId,
}

impl Drop for ResolvedOnDrop {
    fn drop(&mut self) {
        self.bus.publish(LiveEvent::SubagentInputResolved {
            parent: self.parent,
            child: self.child,
        });
    }
}

#[async_trait]
impl PermissionResponder for ChildResponder {
    async fn decide(&self, req: &PermissionRequest) -> Decision {
        // Sin aprobación humana pendiente (padre que permite sin preguntar, o permiso ya
        // concedido en la sesión) el hijo no pasa por `auth-required`: se delega sin tocar su estado.
        if !self.parent_service.would_prompt(req).await {
            return self.parent_service.check(req.clone()).await;
        }
        // El padre va a preguntar al usuario: mientras espera, el hijo está en `auth-required`.
        let mut espera = VuelveAWorking::armar(&self.tracker, self.parent, self.child);
        let resumen = resumen_de_permiso(&req.tool, &req.action, &req.detail);
        if let Err(error) = self
            .tracker
            .transition(
                self.parent,
                self.child,
                TaskStatus::new(TaskState::AuthRequired).with_message(resumen),
            )
            .await
        {
            tracing::warn!("persist subagent status: {error}");
        }
        let decision = self.parent_service.check(req.clone()).await;
        espera.resolver().await;
        decision
    }

    /// Un hijo puede anidar otro responder que pida al usuario, así que lo declara como `true`.
    /// La decisión por petición la toma `decide` consultando a `parent_service.would_prompt`.
    fn requires_user(&self) -> bool {
        true
    }

    async fn ask(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        let _resolved = ResolvedOnDrop {
            bus: self.bus.clone(),
            parent: self.parent,
            child: self.child,
        };
        // Mientras la pregunta espera respuesta, el hijo está en `input-required` con su texto.
        let texto = questions
            .iter()
            .map(|question| question.question.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let mut espera = VuelveAWorking::armar(&self.tracker, self.parent, self.child);
        if let Err(error) = self
            .tracker
            .transition(
                self.parent,
                self.child,
                TaskStatus::new(TaskState::InputRequired).with_message(texto),
            )
            .await
        {
            tracing::warn!("persist subagent status: {error}");
        }
        // El aviso sale por el bus solo cuando la petición ya está en el buzón, para
        // que quien reaccione al evento (un despertar del padre) la encuentre.
        let bus = self.bus.clone();
        let (parent, child) = (self.parent, self.child);
        let label = self.label.clone();
        let respuesta = self
            .broker
            .request_with(
                self.parent,
                self.child,
                self.label.clone(),
                questions.to_vec(),
                move || {
                    bus.publish(LiveEvent::SubagentInputRequested {
                        parent,
                        child,
                        label,
                    });
                },
            )
            .await;
        espera.resolver().await;
        respuesta
    }
}
