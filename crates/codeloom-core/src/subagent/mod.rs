use crate::a2a::{Artifact, Message};
use crate::agent::AgentLoop;
use crate::budget::BudgetConfig;
use crate::catalog::Catalog;
use crate::config::{CodeLoomConfig, ConfirmDelegation};
use crate::error::{CoreError, Result};
use crate::event::{EventBus, LiveEvent};
use crate::job::{JobBody, JobHandle, JobId, JobRegistry, JobResult, JobSpec};
use crate::permission::PermissionService;
use crate::provider::{ProfileProviderResolver, Provider};
use crate::registry::router::{AgentRouter, RouteDecision, RouteRequest};
use crate::registry::{
    AgentEntry, AgentOrigin, AgentRegistry, Approval, Execution, AGENTE_POR_DEFECTO,
};
use crate::remote_task::{
    RemoteClientFactory, RemoteReply, RemoteTask, RemoteTaskClient, RemoteTaskError,
};
use crate::session::{SessionId, SessionStore, SubagentDescriptor};
use crate::subagent_input::{ChildResponder, InputBroker, PendingInput};
use crate::subagent_phase::SubagentPhase;
use crate::task::{TaskState, TaskStatus};
use crate::tools::spill::ToolSpill;
use crate::tools::{Tool, ToolRegistry};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

mod depth;
pub use depth::*;

mod profile_prompt;
pub(crate) use profile_prompt::*;

mod tracker;
pub use tracker::*;

mod launcher;
pub use launcher::*;

mod control;
use control::*;

mod remote;
use remote::*;

mod local_run;

mod admission;
pub(crate) use admission::*;

/// Petición de un subagente hecha directamente por la persona usuaria. `skills` es una selección, en orden,
/// dentro de las que declara el perfil del agente; `None` carga todas las declaradas, como `start`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentRequest {
    pub label: String,
    pub prompt: String,
    pub agent: Option<String>,
    pub skills: Option<Vec<String>>,
}

#[async_trait]
pub trait SubagentLauncher: Send + Sync {
    async fn start(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)>;

    /// Arranca un subagente por orden directa de la persona usuaria: no pide la aprobación de
    /// delegación (que es para las del modelo) y admite elegir un subconjunto de las skills del perfil.
    /// Por defecto delega en `start` y rechaza la selección de skills.
    async fn start_requested(
        &self,
        parent: SessionId,
        request: SubagentRequest,
        cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        if request.skills.is_some() {
            return Err(CoreError::Tool(
                "skills selection unavailable for this launcher".into(),
            ));
        }
        self.start(parent, request.label, request.prompt, request.agent, cancel)
            .await
    }

    async fn start_read_only_review(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: String,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Err(CoreError::Tool(
            "read-only workflow reviewer unavailable".into(),
        ))
    }

    async fn start_local_workflow_agent(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: String,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Err(CoreError::Tool("local workflow agent unavailable".into()))
    }

    async fn control(
        &self,
        parent: SessionId,
        child: SessionId,
        action: &str,
        message: Option<String>,
    ) -> Result<()>;

    /// Elige agente para una tarea sin agente explícito (A12b). `None` significa «no sé» o «no hay
    /// enrutador»: se delega en el agente por defecto, como siempre.
    async fn route(
        &self,
        _parent: SessionId,
        _label: &str,
        _prompt: &str,
    ) -> Result<Option<RouteDecision>> {
        Ok(None)
    }

    /// Arranca un hijo como seguimiento de otro ya terminado (A11): una tarea nueva en el mismo
    /// contexto, con el resultado de la anterior en su prompt y el enlace guardado como
    /// `reference_task_ids`.
    async fn start_follow_up(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: Option<String>,
        _follow_up_of: SessionId,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        Err(CoreError::Tool("subagent follow-up unavailable".into()))
    }

    /// `CancelTask` de A2A: pide cancelar al hijo y devuelve su tarea actualizada. Falla con
    /// `TaskNotFound` si no es hijo de `parent` y con `TaskNotCancelable` si ya terminó. El éxito
    /// no está garantizado: si el hijo no se asienta a tiempo se devuelve su estado actual, sin
    /// afirmar que se canceló.
    async fn cancel(&self, _parent: SessionId, _child: SessionId) -> Result<SubagentDescriptor> {
        Err(CoreError::Tool("subagent cancel unavailable".into()))
    }

    async fn list(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>>;

    async fn deliver_notices(&self, parent: SessionId) -> Result<Vec<crate::message::Message>>;

    /// Responde la pregunta pendiente de un hijo (`input-required`).
    async fn answer(
        &self,
        _parent: SessionId,
        _child: SessionId,
        _answers: Vec<String>,
    ) -> Result<()> {
        Err(CoreError::Tool("subagent answers unavailable".into()))
    }

    /// Avisos de las preguntas nuevas de los hijos; cada una sale una sola vez.
    async fn pending_input_notices(
        &self,
        _parent: SessionId,
    ) -> Result<Vec<crate::message::Message>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests;
