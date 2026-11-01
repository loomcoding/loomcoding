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
use crate::task::{TaskState, TaskStatus};
use crate::tools::spill::ToolSpill;
use crate::tools::{Tool, ToolRegistry};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

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

/// Niveles restantes de la sesión raíz: ella misma más los del techo de profundidad.
pub fn root_levels(ceiling: u32) -> u32 {
    ceiling.saturating_add(1)
}

/// Niveles restantes de un hijo: los del padre menos uno, sin pasar de lo que su perfil declara (sin
/// `max_depth`, una hoja).
pub fn child_levels(parent_left: u32, profile_max_depth: Option<u32>) -> u32 {
    parent_left
        .saturating_sub(1)
        .min(profile_max_depth.unwrap_or(1).max(1))
}

/// Tope del texto de persona del perfil dentro del prompt adicional del hijo. Las skills tienen el suyo.
pub(crate) const MAX_PROFILE_PROMPT_BYTES: usize = 16_000;

/// Holgura para el marcador que `limit_bytes` añade al recortar (`[... N bytes omitted ...]`).
pub(crate) const TRUNCATION_MARKER_SLACK: usize = 128;

/// Une la persona del perfil y las skills cargadas en el prompt adicional del hijo. Cada parte lleva su
/// tope propio: unas skills grandes no deben recortar la persona.
pub(crate) fn compose_child_system(persona: Option<String>, skills_prompt: &str) -> Option<String> {
    let mut sistema = persona
        .map(|texto| crate::tools::limit_bytes(&texto, MAX_PROFILE_PROMPT_BYTES).into_owned())
        .unwrap_or_default();
    if !skills_prompt.is_empty() {
        if !sistema.is_empty() {
            sistema.push_str("\n\n");
        }
        sistema.push_str(skills_prompt);
    }
    (!sistema.is_empty()).then_some(sistema)
}

/// Si el perfil pide más niveles de los que el techo del árbol deja al hijo.
pub fn depth_clipped(parent_left: u32, profile_max_depth: Option<u32>) -> bool {
    profile_max_depth.is_some_and(|pedidos| pedidos > parent_left.saturating_sub(1))
}

/// Una sesión puede delegar solo si le quedan al menos dos niveles: ella y uno por debajo.
pub fn can_delegate(left: u32) -> bool {
    left >= 2
}

pub struct InProcessSubagentLauncher {
    provider: Arc<RwLock<Arc<dyn Provider>>>,
    store: Arc<dyn SessionStore>,
    tools: Arc<ToolRegistry>,
    permission: Arc<PermissionService>,
    config: Arc<RwLock<CodeLoomConfig>>,
    cwd: PathBuf,
    jobs: Arc<dyn JobRegistry>,
    active: Arc<Semaphore>,
    /// Tope de subagentes activos, el mismo con que se creó `active`; sirve al mensaje de rechazo.
    max_active: usize,
    /// Modo de confirmación al delegar. Con `Always`, cada delegación exige aprobación humana (C3).
    confirm: ConfirmDelegation,
    /// Mando de delegación (`[agents] delegation`): la regla de prompt que también reciben los hijos que delegan.
    delegation: Option<crate::config::DelegationMode>,
    bus: EventBus,
    broker: Arc<InputBroker>,
    /// Catálogo compartido con el agente padre; cada hijo lo usa para sus apuntes de consumo.
    catalog: Arc<RwLock<Catalog>>,
    /// Registro de agentes. Sin él solo existe el agente `default`, y el lanzador se comporta
    /// exactamente como antes de la tarea 6.
    registry: Option<Arc<AgentRegistry>>,
    /// Volcado de salidas recortadas que reciben los hijos; `None` como en el agente padre.
    spill: Option<ToolSpill>,
    /// Presupuesto del padre: cada hijo lo recibe y suma su consumo al total de la familia.
    budget: BudgetConfig,
    /// Lo máximo que `cancel` espera a que el hijo se asiente; `CANCEL_WAIT` salvo en las pruebas.
    cancel_wait: std::time::Duration,
    /// Fábrica de clientes de agentes remotos (A8a). Sin ella, un remoto no se ejecuta.
    remote_clients: Option<Arc<dyn RemoteClientFactory>>,
    /// Cada cuánto se consulta una tarea remota, y cuánto se la sigue como máximo.
    remote_poll: std::time::Duration,
    remote_max_follow: std::time::Duration,
    /// Lo máximo que se espera a que la persona conteste a una pregunta de un remoto.
    remote_input_wait: std::time::Duration,
    profile_provider_resolver: Option<Arc<ProfileProviderResolver>>,
    /// Enrutador de agentes (A12b) y el modo de confirmación con el que convive. Sin él no se elige
    /// agente automáticamente.
    router: Option<(Arc<dyn AgentRouter>, ConfirmDelegation)>,
    /// Niveles restantes de la sesión que usa este lanzador (véase `child_levels`). Por defecto, los de
    /// la raíz con el techo por defecto: la delegación de siempre, sin recursión.
    levels_left: u32,
    /// Herramientas de subagentes que el constructor retira de los hijos y que solo reciben los que
    /// pueden delegar. `development_workflow` nunca figura aquí: solo la sesión raíz la gobierna.
    delegation_tools: Vec<Arc<dyn Tool>>,
    /// Sesión raíz de la familia de presupuesto. `None` en el lanzador de la raíz (su padre es la raíz);
    /// los lanzadores anidados la heredan, de modo que el gasto de los nietos suma a la misma familia.
    family_root: Option<SessionId>,
    /// Perfiles cuyo recorte de profundidad ya se avisó, para avisar una sola vez por nombre.
    clipped_notified: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

/// Descarta la pregunta pendiente del hijo al terminar su trabajo, sea cual sea la
/// causa (final normal, cancelación o aborto), para no dejar entradas huérfanas.
struct DiscardOnExit {
    broker: Arc<InputBroker>,
    child: SessionId,
}

impl Drop for DiscardOnExit {
    fn drop(&mut self) {
        self.broker.discard(self.child);
    }
}

/// Lo máximo que `cancel` espera a que el hijo se asiente tras la orden de parar.
const CANCEL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Cada cuánto se consulta una tarea remota.
const REMOTE_POLL: std::time::Duration = std::time::Duration::from_secs(1);

/// Lo máximo que se sigue una tarea remota antes de darla por perdida (decálogo: todo bucle tiene cota).
const REMOTE_MAX_FOLLOW: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Cuánto se espera a que el remoto abra el flujo de la tarea (A8c) antes de sondear.
const REMOTE_WATCH_OPEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Rondas de pregunta (`input-required`) que se atienden como máximo en una tarea remota.
const REMOTE_MAX_RONDAS_ENTRADA: u32 = 5;
/// Lo máximo que se espera a que la persona conteste a una pregunta de un remoto.
const REMOTE_ESPERA_RESPUESTA: std::time::Duration = std::time::Duration::from_secs(30 * 60);
/// Bytes de la pregunta de un remoto que se muestran (el texto es de un tercero).
const REMOTE_MAX_BYTES_PREGUNTA: usize = 2_000;
/// Lo máximo que se espera a que un remoto conteste a `CancelTask`; menor que `CANCEL_WAIT`.
const REMOTE_CANCEL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

/// Consultas fallidas seguidas tras las que se da por perdido el contacto con el remoto.
const REMOTE_MAX_FALLOS_SEGUIDOS: u32 = 3;

/// Cada cuánto `cancel` relee el estado del hijo mientras espera.
const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(10);

/// Mensaje de la tarea rechazada por capacidad agotada. Es el mismo error que recibe el padre.
fn mensaje_capacidad(maximo: usize) -> String {
    format!("maximum of {maximo} active subagents reached")
}

/// Único punto del lanzador que cambia el estado de la tarea de un hijo. Valida la
/// transición con `TaskState::can_transition_to`, la persiste y publica `SubagentChanged`.
pub struct TaskTracker {
    store: Arc<dyn SessionStore>,
    bus: EventBus,
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

    async fn descriptor(&self, parent: SessionId, child: SessionId) -> Result<SubagentDescriptor> {
        self.store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or_else(|| CoreError::Store(format!("unknown subagent {}", child.0)))
    }
}

impl InProcessSubagentLauncher {
    /// Descriptor vigente de un hijo de `parent`, o `TaskNotFound`.
    async fn descriptor_of(
        &self,
        parent: SessionId,
        child: SessionId,
    ) -> Result<SubagentDescriptor> {
        self.store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or(CoreError::TaskNotFound(child.0))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: Arc<RwLock<Arc<dyn Provider>>>,
        store: Arc<dyn SessionStore>,
        tools: Arc<ToolRegistry>,
        permission: Arc<PermissionService>,
        config: Arc<RwLock<CodeLoomConfig>>,
        cwd: PathBuf,
        jobs: Arc<dyn JobRegistry>,
        bus: EventBus,
    ) -> Self {
        let delegation_tools = ["subagent", "subagent_control", "subagent_list"]
            .into_iter()
            .filter_map(|name| tools.get(name))
            .collect();
        Self {
            provider,
            store,
            tools: Arc::new(
                tools
                    .without("subagent")
                    .without("subagent_control")
                    .without("subagent_list")
                    .without("development_workflow"),
            ),
            permission,
            config,
            cwd,
            jobs,
            active: Arc::new(Semaphore::new(8)),
            max_active: 8,
            confirm: ConfirmDelegation::default(),
            delegation: None,
            bus,
            broker: Arc::new(InputBroker::default()),
            catalog: Arc::new(RwLock::new(Catalog::default())),
            registry: None,
            spill: None,
            budget: BudgetConfig::default(),
            cancel_wait: CANCEL_WAIT,
            remote_clients: None,
            remote_poll: REMOTE_POLL,
            remote_max_follow: REMOTE_MAX_FOLLOW,
            remote_input_wait: REMOTE_ESPERA_RESPUESTA,
            profile_provider_resolver: None,
            router: None,
            levels_left: root_levels(crate::config::DEFAULT_DEPTH_CEILING),
            delegation_tools,
            family_root: None,
            clipped_notified: Arc::default(),
        }
    }

    /// Raíz de la familia de presupuesto de los hijos que `parent` lance con este lanzador: el propio
    /// `parent` si este es el lanzador de la raíz, o la raíz heredada si es uno anidado.
    fn familia(&self, parent: SessionId) -> SessionId {
        self.family_root.unwrap_or(parent)
    }

    /// Lanzador de la sesión `hijo`, que puede delegar: comparte con este el semáforo de activos, el
    /// buzón de preguntas, el almacén, el bus, los trabajos, el catálogo, el registro y el presupuesto, y
    /// ve el árbol entero como una sola familia. Cambian el proveedor, la configuración, el servicio de
    /// permisos y las herramientas, que son los del hijo, y los niveles restantes.
    #[allow(clippy::too_many_arguments)]
    fn anidado(
        &self,
        raiz: SessionId,
        provider: Arc<dyn Provider>,
        config: CodeLoomConfig,
        permission: Arc<PermissionService>,
        tools: &ToolRegistry,
        levels_left: u32,
    ) -> Self {
        Self {
            provider: Arc::new(RwLock::new(provider)),
            store: self.store.clone(),
            tools: Arc::new(
                tools
                    .without("subagent")
                    .without("subagent_control")
                    .without("subagent_list")
                    .without("development_workflow"),
            ),
            permission,
            config: Arc::new(RwLock::new(config)),
            cwd: self.cwd.clone(),
            jobs: self.jobs.clone(),
            active: self.active.clone(),
            max_active: self.max_active,
            confirm: self.confirm,
            delegation: self.delegation,
            bus: self.bus.clone(),
            broker: self.broker.clone(),
            catalog: self.catalog.clone(),
            registry: self.registry.clone(),
            spill: self.spill.clone(),
            budget: self.budget.clone(),
            cancel_wait: self.cancel_wait,
            remote_clients: self.remote_clients.clone(),
            remote_poll: self.remote_poll,
            remote_max_follow: self.remote_max_follow,
            remote_input_wait: self.remote_input_wait,
            profile_provider_resolver: self.profile_provider_resolver.clone(),
            router: self.router.clone(),
            levels_left,
            delegation_tools: self.delegation_tools.clone(),
            family_root: Some(raiz),
            clipped_notified: self.clipped_notified.clone(),
        }
    }

    /// Fija los niveles restantes de la sesión que usa este lanzador. Con la raíz, `root_levels(techo)`.
    pub fn with_levels_left(mut self, levels_left: u32) -> Self {
        self.levels_left = levels_left;
        self
    }

    /// Fija el modo de confirmación al delegar. Con `Always`, `start` pregunta antes de crear el hijo
    /// y deniega si no hay respuesta humana explícita.
    pub fn with_delegation_confirm(mut self, confirm: ConfirmDelegation) -> Self {
        self.confirm = confirm;
        self
    }

    /// Fija el mando de delegación que heredan los hijos que delegan (`[agents] delegation`).
    pub fn with_delegation_mode(mut self, mode: Option<crate::config::DelegationMode>) -> Self {
        self.delegation = mode;
        self
    }

    /// Fija cuántos subagentes pueden estar activos a la vez (`[agents] max_active_subagents`).
    pub fn with_max_active(mut self, maximo: usize) -> Self {
        self.max_active = maximo;
        self.active = Arc::new(Semaphore::new(maximo));
        self
    }

    /// Permite ejecutar agentes remotos: `fabrica` construye el cliente de cada uno.
    pub fn with_remote_clients(mut self, fabrica: Arc<dyn RemoteClientFactory>) -> Self {
        self.remote_clients = Some(fabrica);
        self
    }

    /// Cambia cuánto se espera a que la persona conteste a la pregunta de un remoto.
    pub fn with_remote_input_wait(mut self, espera: std::time::Duration) -> Self {
        self.remote_input_wait = espera;
        self
    }

    /// Cambia cada cuánto se consulta una tarea remota y cuánto se la sigue como máximo.
    pub fn with_remote_timing(
        mut self,
        poll: std::time::Duration,
        max_follow: std::time::Duration,
    ) -> Self {
        self.remote_poll = poll;
        self.remote_max_follow = max_follow;
        self
    }

    /// Cambia cuánto espera `cancel` a que el hijo se asiente. Existe para las pruebas, que no
    /// pueden pausar el reloj porque el almacén usa una piscina de conexiones con plazos reales.
    #[cfg(test)]
    fn with_cancel_wait(mut self, espera: std::time::Duration) -> Self {
        self.cancel_wait = espera;
        self
    }

    /// Comparte el presupuesto de gasto con los hijos que lance: avisan con el total de la familia.
    pub fn with_budget(mut self, budget: BudgetConfig) -> Self {
        self.budget = budget;
        self
    }

    /// Comparte el volcado a disco con los hijos que lance, como `with_catalog`.
    pub fn with_spill(mut self, spill: ToolSpill) -> Self {
        self.spill = Some(spill);
        self
    }

    /// Comparte el registro de agentes: `start` resuelve ahí el agente pedido.
    pub fn with_registry(mut self, registry: Arc<AgentRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    pub fn with_profile_provider_resolver(
        mut self,
        resolver: Arc<ProfileProviderResolver>,
    ) -> Self {
        self.profile_provider_resolver = Some(resolver);
        self
    }

    /// Activa la elección automática de agente (A12b) cuando la herramienta no nombra ninguno.
    /// Con `confirm = always` no actúa: ahí la persona usuaria aprueba cada asignación.
    pub fn with_router(mut self, router: Arc<dyn AgentRouter>, confirm: ConfirmDelegation) -> Self {
        self.router = Some((router, confirm));
        self
    }

    /// Comparte el catálogo del agente padre con los hijos que lance.
    pub fn with_catalog(mut self, catalog: Arc<RwLock<Catalog>>) -> Self {
        self.catalog = catalog;
        self
    }

    /// Preguntas pendientes de los hijos de un padre (`input-required`).
    pub fn pending_inputs(&self, parent: SessionId) -> Vec<PendingInput> {
        self.broker.pending(parent)
    }

    /// Guarda un hijo rechazado por capacidad agotada: tarea `rejected` sin trabajo.
    async fn registrar_rechazo(
        &self,
        parent: SessionId,
        label: &str,
        project: &str,
        agent: Option<String>,
        agent_origin: Option<String>,
    ) -> Result<()> {
        let child = self.store.create_session(project).await?;
        let descriptor = SubagentDescriptor {
            parent,
            child,
            job_id: String::new(),
            label: label.to_string(),
            status: TaskStatus::new(TaskState::Rejected)
                .with_message(mensaje_capacidad(self.max_active)),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent,
            agent_origin,
            reference_task_ids: Vec::new(),
        };
        self.store.save_subagent(&descriptor).await?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        Ok(())
    }

    /// Resuelve el agente pedido (`None` = `default`) en la instantánea vigente y devuelve su
    /// entrada copiada por valor: una recarga posterior no la altera. Sin registro, solo
    /// `default` es válido y se devuelve `None` (sin perfil que aplicar). Todo rechazo es un
    /// error de validación: quien llama no ha creado aún tarea ni puesto.
    fn resolver_agente(&self, agent: Option<&str>) -> Result<Option<AgentEntry>> {
        let nombre = agent.unwrap_or(AGENTE_POR_DEFECTO);
        let Some(registro) = &self.registry else {
            if nombre == AGENTE_POR_DEFECTO {
                return Ok(None);
            }
            return Err(CoreError::Tool(format!(
                "no hay registro de agentes configurado: no se puede usar el agente «{nombre}»"
            )));
        };
        let instantanea = registro.snapshot();
        let entrada = instantanea.get(nombre).ok_or_else(|| {
            CoreError::Tool(format!("agente «{nombre}» inexistente en el registro"))
        })?;
        // Sin fábrica de clientes no hay forma de ejecutar un remoto: se dice antes que lo demás.
        if matches!(entrada.execution, Execution::Remote(_)) && self.remote_clients.is_none() {
            return Err(CoreError::Tool(format!(
                "agente remoto no disponible: «{nombre}»"
            )));
        }
        if instantanea.approval(nombre) == Some(Approval::Pending) {
            return Err(CoreError::Tool(format!(
                "agente «{nombre}» pendiente de aprobación: apruébalo antes de usarlo"
            )));
        }
        if let (Execution::Remote(spec), Some(fabrica)) = (&entrada.execution, &self.remote_clients)
        {
            if let Some(motivo) = fabrica.unavailable_reason(nombre, spec) {
                return Err(CoreError::Tool(format!(
                    "el agente remoto «{nombre}» no se puede usar: {motivo}"
                )));
            }
        }
        if let Execution::Local(spec) = &entrada.execution {
            if let Some(modelo) = &spec.model {
                let base_url = spec
                    .provider
                    .as_ref()
                    .map(|provider| provider.base_url.clone())
                    .unwrap_or_else(|| self.config.read().unwrap().provider.base_url.clone());
                let conocido = self.catalog.read().unwrap().knows_model(&base_url, modelo);
                if conocido == Some(false) {
                    return Err(CoreError::Tool(format!(
                        "el modelo «{modelo}» del agente «{nombre}» no figura en el catálogo"
                    )));
                }
            }
        }
        Ok(Some(entrada.clone()))
    }

    /// Cliente del agente remoto elegido; `None` si el agente es local. Un remoto sin cliente
    /// construible (p. ej. sin tarjeta descargada) es un error: no se crea tarea.
    fn cliente_remoto(
        &self,
        perfil: Option<&AgentEntry>,
    ) -> Result<Option<Arc<dyn RemoteTaskClient>>> {
        let Some(AgentEntry {
            name,
            execution: Execution::Remote(spec),
            ..
        }) = perfil
        else {
            return Ok(None);
        };
        self.remote_clients
            .as_ref()
            .and_then(|fabrica| fabrica.client_for(name, spec))
            .map(Some)
            .ok_or_else(|| CoreError::Tool(format!("agente remoto no disponible: «{name}»")))
    }

    /// Con `confirm_delegation = always`, pregunta a la persona usuaria antes de crear el hijo y solo
    /// continúa si responde exactamente «Approve». Sin nadie que responda (sin usuario interactivo,
    /// `--yolo`, pregunta descartada) se deniega: nada se aprueba por omisión. Es el control por código
    /// de esa regla, que antes solo existía como frase del prompt.
    async fn pedir_aprobacion(&self, label: &str, prompt: &str, agent: Option<&str>) -> Result<()> {
        if self.confirm != ConfirmDelegation::Always {
            return Ok(());
        }
        let extracto = crate::tools::limit_bytes(prompt, 600);
        let pregunta = crate::question::Question {
            header: "Delegation".into(),
            question: format!(
                "Start a subagent?
Agent: {}
Task: {label}

{extracto}",
                agent.unwrap_or("default")
            ),
            options: vec![
                crate::question::QuestionOption {
                    label: "Approve".into(),
                    description: String::new(),
                },
                crate::question::QuestionOption {
                    label: "Cancel".into(),
                    description: String::new(),
                },
            ],
            multi_select: false,
        };
        let aprobada = self
            .permission
            .ask_user(&[pregunta])
            .await
            .and_then(|respuestas| respuestas.into_iter().next())
            .is_some_and(|respuesta| respuesta.selected == ["Approve"]);
        if aprobada {
            Ok(())
        } else {
            Err(CoreError::Tool(
                "delegation was not approved by the user".into(),
            ))
        }
    }

    /// Arranque común de `start` y `start_follow_up`. La petición ya viene validada.
    #[allow(clippy::too_many_arguments)]
    async fn arrancar(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        referencias: Vec<SessionId>,
        read_only: bool,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        self.arrancar_con(
            parent,
            label,
            prompt,
            agent,
            referencias,
            read_only,
            None,
            parent_cancel,
        )
        .await
    }

    /// Como `arrancar`, con una selección opcional de las skills del perfil.
    #[allow(clippy::too_many_arguments)]
    async fn arrancar_con(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        referencias: Vec<SessionId>,
        read_only: bool,
        skills: Option<Vec<String>>,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        // El agente se valida antes de reservar puesto o crear tarea: un rechazo no deja rastro.
        let perfil = self.resolver_agente(agent.as_deref())?;
        let skills_elegidas = elegir_skills(perfil.as_ref(), skills)?;
        if read_only
            && perfil
                .as_ref()
                .is_some_and(|entry| !matches!(entry.execution, Execution::Local(_)))
        {
            return Err(CoreError::Tool(
                "workflow reviewer must be a local agent".into(),
            ));
        }
        let cliente_remoto = self.cliente_remoto(perfil.as_ref())?;
        let profile_provider = match &perfil {
            Some(AgentEntry {
                execution: Execution::Local(spec),
                ..
            }) => match &spec.provider {
                Some(config) => {
                    let resolver = self.profile_provider_resolver.as_ref().ok_or_else(|| {
                        CoreError::Tool("no hay proveedor configurado para el perfil".into())
                    })?;
                    Some((resolver(config)?, config.clone()))
                }
                None => None,
            },
            _ => None,
        };
        let (agente, origen) = match &perfil {
            Some(entrada) => (Some(entrada.name.clone()), Some(entrada.origin.as_text())),
            None => (None, None),
        };
        let project = self.cwd.to_string_lossy().into_owned();
        let permit = match self.active.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                // La validación ya pasó: la capacidad agotada sí deja constancia como tarea `rejected`.
                self.registrar_rechazo(parent, &label, &project, agente.clone(), origen.clone())
                    .await?;
                return Err(CoreError::Tool(mensaje_capacidad(self.max_active)));
            }
        };
        let child = self.store.create_session(&project).await?;
        let tracker = Arc::new(TaskTracker::new(self.store.clone(), self.bus.clone()));
        let child_store = self.store.clone();
        let scope = perfil
            .as_ref()
            .and_then(|entry| match &entry.execution {
                Execution::Local(spec) => Some(spec.scope.mcp.as_slice()),
                Execution::Remote(_) => None,
            })
            .unwrap_or_default();
        for server_id in scope {
            if !self.tools.has_mcp_server(server_id) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{}` allows MCP server `{server_id}`, but it is not configured or connected",
                        agente.as_deref().unwrap_or(AGENTE_POR_DEFECTO)
                    ),
                });
            }
        }
        // Niveles restantes del hijo y si puede delegar (véase `child_levels`). Un agente remoto o de
        // revisión de solo lectura nunca delega.
        let profile_depth = match &perfil {
            Some(AgentEntry {
                execution: Execution::Local(spec),
                ..
            }) => spec.max_depth,
            _ => None,
        };
        let child_left = child_levels(self.levels_left, profile_depth);
        let delegates = !read_only && can_delegate(child_left);
        if depth_clipped(self.levels_left, profile_depth) {
            let nombre = agente.clone().unwrap_or_else(|| AGENTE_POR_DEFECTO.into());
            if self.clipped_notified.lock().unwrap().insert(nombre.clone()) {
                self.bus.publish(crate::event::LiveEvent::RuntimeNotice {
                    message: format!(
                        "Agent `{nombre}` asks for max_depth {} but the depth ceiling leaves it {child_left}",
                        profile_depth.unwrap_or_default()
                    ),
                });
            }
        }
        let mut scoped_tools = self.tools.with_mcp_scope(scope);
        if delegates {
            for tool in &self.delegation_tools {
                scoped_tools.register(tool.clone());
            }
        }
        let child_tools = if read_only {
            Arc::new(scoped_tools.only(&["view", "glob", "grep"]))
        } else {
            Arc::new(scoped_tools)
        };
        let child_provider = profile_provider
            .as_ref()
            .map(|(provider, _)| provider.clone())
            .unwrap_or_else(|| self.provider.read().unwrap().clone());
        // Cada hijo tiene su propio servicio de permisos: delega en el del padre y
        // encamina sus preguntas (`ask_user`) al buzón de este lanzador.
        let child_permission = Arc::new(PermissionService::with_responder(Box::new(
            ChildResponder::new(
                self.permission.clone(),
                self.broker.clone(),
                parent,
                child,
                label.clone(),
                self.bus.clone(),
                tracker.clone(),
            ),
        )));
        let child_broker = self.broker.clone();
        // Configuración del hijo: la del padre, con lo que declare el perfil. Lo que el perfil
        // no declara se hereda. Se copia aquí, al arrancar, y la tarea ya no depende del registro.
        let mut child_config = self.config.read().unwrap().clone();
        if let Some((_, provider)) = &profile_provider {
            child_config.provider = provider.clone();
        }
        let mut child_system = None;
        let mut skill_statuses = Vec::new();
        let mut skill_usage_components = Vec::new();
        if let Some(AgentEntry {
            execution: Execution::Local(spec),
            origin,
            ..
        }) = &perfil
        {
            if let Some(modelo) = &spec.model {
                child_config.model = modelo.clone();
            }
            if let Some(esfuerzo) = &spec.effort {
                child_config.effort = Some(esfuerzo.clone());
            }
            // `max_turns` limita los pasos del bucle del hijo: es el límite que ya lleva `max_steps`.
            if let Some(turnos) = spec.max_turns {
                child_config.max_steps = usize::try_from(turnos).unwrap_or(usize::MAX);
            }
            // `max_depth` se aplicará al construir el hijo (véase `child_levels`).
            let sistema = spec.system_prompt();
            if !sistema.trim().is_empty() {
                child_system = Some(sistema);
            }
            let home = std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from);
            let mut raices = crate::skills::SkillRoots::new(self.cwd.clone(), home);
            // Las skills de un catálogo externo solo las ve el agente que viene de ese catálogo.
            if let AgentOrigin::File {
                level: crate::registry::ProfileLevel::Catalog,
                path,
            } = origin
            {
                if let Some(raiz) = crate::registry::catalog_root(path) {
                    raices = raices.with_catalog(raiz.join(".agents").join("skills"));
                }
            }
            let ids = skills_elegidas.as_deref().unwrap_or(&spec.scope.skills);
            let loaded = crate::skills::load_scoped_skills(ids, &raices);
            skill_statuses = loaded.statuses;
            skill_usage_components = skill_statuses
                .iter()
                .filter(|skill| skill.state == crate::skills::SkillLoadState::Loaded)
                .map(|skill| crate::session::UsageComponent {
                    kind: crate::session::UsageComponentKind::Skill,
                    id: skill.id.clone(),
                    input_tokens: skill.input_tokens_per_request,
                })
                .collect();
            if loaded.shared_input_tokens_per_request > 0 {
                skill_usage_components.push(crate::session::UsageComponent {
                    kind: crate::session::UsageComponentKind::Skill,
                    id: "local-skills-preamble".into(),
                    input_tokens: loaded.shared_input_tokens_per_request,
                });
            }
            child_system = compose_child_system(child_system.take(), &loaded.prompt);
        }
        // El hijo que puede delegar recibe un lanzador anidado que comparte con este el tope de activos, el
        // buzón y el presupuesto de toda la familia; los demás no reciben ninguno.
        let child_family = self.familia(parent);
        let child_nested = delegates.then(|| -> Arc<dyn SubagentLauncher> {
            Arc::new(self.anidado(
                child_family,
                child_provider.clone(),
                child_config.clone(),
                child_permission.clone(),
                &child_tools,
                child_left,
            ))
        });
        let child_registry = if delegates {
            self.registry.clone()
        } else {
            None
        };
        let child_confirm = self.confirm;
        let child_delegation = self.delegation;
        let child_max_steps = child_config.max_steps;
        let child_cwd = self.cwd.clone();
        let child_jobs = self.jobs.clone();
        let child_catalog = self.catalog.clone();
        let child_spill = self.spill.clone();
        let child_budget = self.budget.clone();
        let descriptor_label = label.clone();
        // El bus del hijo se crea y se suscribe antes de lanzarlo, para no perder ningún
        // evento de su arranque. Su canal se cierra cuando se sueltan todos sus emisores.
        let child_bus = EventBus::new(256);
        let mut child_rx = child_bus.subscribe();
        if !skill_statuses.is_empty() {
            child_bus.publish(LiveEvent::SkillsLoaded {
                skills: skill_statuses,
            });
        }
        // La compuerta retiene al hijo hasta que su tarea `submitted` está guardada.
        let (abrir_puerta, puerta) = tokio::sync::oneshot::channel::<()>();
        let cuerpo_tracker = tracker.clone();
        let run: JobBody = match cliente_remoto {
            Some(cliente) => cuerpo_remoto(EjecucionRemota {
                cliente,
                parent,
                child,
                prompt,
                tracker: cuerpo_tracker,
                store: self.store.clone(),
                permit,
                puerta,
                parent_cancel,
                poll: self.remote_poll,
                max_follow: self.remote_max_follow,
                broker: child_broker.clone(),
                bus: self.bus.clone(),
                label: label.clone(),
                input_wait: self.remote_input_wait,
            }),
            None => Box::new(move |handle, job_cancel| {
                Box::pin(async move {
                    let _permit = permit;
                    let _discard = DiscardOnExit {
                        broker: child_broker,
                        child,
                    };
                    // Si el lanzador no llega a abrir la compuerta, el hijo no arranca.
                    if puerta.await.is_err() || job_cancel.is_cancelled() {
                        return Ok(None);
                    }
                    if let Err(error) = cuerpo_tracker
                        .transition(parent, child, TaskStatus::new(TaskState::Working))
                        .await
                    {
                        tracing::warn!("persist subagent status: {error}");
                    }
                    let mut child_agent = AgentLoop::new(
                        child_provider,
                        child_store.clone(),
                        child_tools,
                        child_bus,
                        child_permission,
                        child_config,
                        child_cwd,
                    )
                    .with_jobs(child_jobs)
                    .with_catalog(child_catalog)
                    .with_system_extra(child_system)
                    .with_system_usage_components(skill_usage_components)
                    .with_budget(child_budget)
                    .with_budget_family(child_family);
                    if let Some(spill) = child_spill {
                        child_agent = child_agent.with_spill(spill);
                    }
                    if let Some(registro) = child_registry {
                        child_agent = child_agent
                            .with_agent_registry(registro)
                            .with_delegation_confirm(child_confirm);
                    }
                    // Al final: los demás `with_*` reconstruyen el lanzador por defecto del agente.
                    if let Some(lanzador) = child_nested {
                        child_agent = child_agent
                            .with_delegation_mode(child_delegation)
                            .with_subagent_launcher(lanzador);
                    }
                    let child_cancel = CancellationToken::new();
                    let outcome = tokio::select! {
                        result = child_agent.run_internal(child, prompt, child_cancel.clone()) => result?,
                        _ = job_cancel.cancelled() => {
                            child_cancel.cancel();
                            return Ok(None);
                        }
                        _ = parent_cancel.cancelled() => {
                            child_cancel.cancel();
                            // Cancelar al padre cancela al hijo: el trabajo se mata para que la
                            // tarea acabe en `canceled` y no en `completed` sin resultado.
                            job_cancel.cancel();
                            return Ok(None);
                        }
                    };
                    // El agente ya no emite: soltarlo cierra el bus del hijo y permite
                    // que la tarea de reenvío termine.
                    drop(child_agent);
                    let steps = outcome.steps;
                    let text = outcome.final_text;
                    if text.trim().is_empty() {
                        let reason = if steps >= child_max_steps {
                            format!(
                                "subagent reached its step limit ({steps}/{child_max_steps}) without a final response"
                            )
                        } else {
                            format!("subagent produced no final response after {steps} steps")
                        };
                        return Err(CoreError::Tool(reason));
                    }
                    child_store
                        .complete_subagent(parent, child, &[Artifact::text_artifact(text.as_str())])
                        .await?;
                    if let Err(error) = cuerpo_tracker.publish(parent, child).await {
                        tracing::warn!("publish subagent status: {error}");
                    }
                    handle.append(&text);
                    Ok(Some(JobResult { content: text }))
                })
            }),
        };
        let job = self.jobs.start(JobSpec {
            kind: "subagent".into(),
            label,
            owner: parent,
            output_limit: 64 * 1024,
            run,
        })?;
        // Reenvío filtrado: solo los eventos relevantes del hijo llegan al padre,
        // etiquetados con su identidad. La tarea termina cuando el bus del hijo se cierra.
        let forward_bus = self.bus.clone();
        tokio::spawn(async move {
            loop {
                match child_rx.recv().await {
                    Ok(event) => {
                        if event.is_child_relevant() {
                            forward_bus.publish(LiveEvent::ChildEvent {
                                parent,
                                child,
                                event: Box::new(event),
                            });
                        }
                    }
                    // Un hijo muy verboso puede adelantar el búfer: se pierden eventos, no el reenvío.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        // La tarea nace `submitted` al reconocerla; el hijo empieza cuando queda guardada.
        let descriptor = SubagentDescriptor {
            parent,
            child,
            job_id: job.id().0.clone(),
            label: descriptor_label,
            status: TaskStatus::new(TaskState::Submitted),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: agente,
            agent_origin: origen,
            reference_task_ids: referencias,
        };
        if let Err(error) = self.store.save_subagent(&descriptor).await {
            let _ = self.jobs.kill(parent, job.id());
            return Err(error);
        }
        let mut updates = self.jobs.subscribe(parent, job.id())?;
        self.bus.publish(LiveEvent::SubagentChanged(descriptor));
        let _ = abrir_puerta.send(());
        let store = self.store.clone();
        let settle_bus = self.bus.clone();
        tokio::spawn(async move {
            loop {
                let snapshot = updates.borrow_and_update().clone();
                let state = TaskState::from(snapshot.status);
                if state.is_terminal() {
                    // `complete_subagent` ya fijó `Completed` junto con el resultado: aquí solo se
                    // escribe lo que el almacén aún no tiene asentado (muerte, fallo o cierre sin resultado).
                    let asentado = match store.list_subagents(parent).await {
                        Ok(hijos) => hijos
                            .iter()
                            .find(|hijo| hijo.child == child && hijo.status.state.is_terminal())
                            .map(|hijo| hijo.status.clone()),
                        Err(error) => {
                            tracing::warn!("read subagent status: {error}");
                            None
                        }
                    };
                    // Manda el estado ya persistido: un espejo remoto fija el suyo (p. ej. `rejected`)
                    // antes de que el trabajo acabe, y el aviso no debe contradecirlo.
                    let estado_final = match asentado {
                        Some(persistido) => persistido.state,
                        None => {
                            let mut status = TaskStatus::new(state);
                            if state == TaskState::Failed {
                                if let Some(detail) = snapshot.detail.as_deref() {
                                    status.message = Some(Message::agent_text(
                                        crate::tools::limit_bytes(
                                            detail,
                                            crate::tools::MAX_NOTICE_BYTES,
                                        )
                                        .into_owned(),
                                    ));
                                }
                            }
                            if let Err(error) = tracker.transition(parent, child, status).await {
                                tracing::warn!("persist subagent status: {error}");
                            }
                            state
                        }
                    };
                    // Se publica tras el intento de persistir, también si falló: así el
                    // padre nunca lee el aviso antes del estado final.
                    settle_bus.publish(LiveEvent::SubagentSettled {
                        parent,
                        child,
                        status: estado_final,
                    });
                    break;
                }
                if updates.changed().await.is_err() {
                    break;
                }
            }
        });
        Ok((child, job.id().clone()))
    }
}

/// Todo lo que el trabajo espejo de un agente remoto necesita para seguir su tarea.
struct EjecucionRemota {
    cliente: Arc<dyn RemoteTaskClient>,
    parent: SessionId,
    child: SessionId,
    prompt: String,
    tracker: Arc<TaskTracker>,
    store: Arc<dyn SessionStore>,
    permit: OwnedSemaphorePermit,
    puerta: tokio::sync::oneshot::Receiver<()>,
    parent_cancel: CancellationToken,
    poll: std::time::Duration,
    max_follow: std::time::Duration,
    /// Buzón de preguntas, bus y etiqueta: para preguntar a la persona lo que el remoto pida.
    broker: Arc<InputBroker>,
    bus: EventBus,
    label: String,
    input_wait: std::time::Duration,
}

fn cuerpo_remoto(ejecucion: EjecucionRemota) -> JobBody {
    Box::new(move |handle, job_cancel| Box::pin(seguir_remoto(ejecucion, handle, job_cancel)))
}

/// Estado local con el mensaje que explica la razón, con la marca de tiempo de este proceso
/// (la del remoto no es de fiar: es de un tercero).
fn estado_con_mensaje(state: TaskState, mensaje: Option<Message>) -> TaskStatus {
    TaskStatus {
        message: mensaje,
        ..TaskStatus::new(state)
    }
}

/// Los pasos del espejo que cambian la tarea local.
struct Espejo {
    cliente: Arc<dyn RemoteTaskClient>,
    parent: SessionId,
    child: SessionId,
    tracker: Arc<TaskTracker>,
    store: Arc<dyn SessionStore>,
    broker: Arc<InputBroker>,
    bus: EventBus,
    label: String,
}

/// Cómo acaba la espera de la respuesta a una pregunta del remoto.
enum Entrada {
    Respuesta(String),
    /// La persona no contestó a tiempo.
    Expirada,
    /// La petición se descartó sin respuesta.
    Descartada,
    /// Se canceló el hijo o el padre mientras se esperaba.
    Cancelada,
}

/// Texto de la pregunta de un remoto, listo para mostrarse: sin caracteres de control (salvo saltos
/// y tabuladores), acotado y nunca vacío. Es dato de un tercero: se muestra, no se interpreta.
fn pregunta_del_remoto(mensaje: Option<&Message>) -> String {
    let plano: String = mensaje
        .map(Message::text)
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let plano = plano.trim();
    if plano.is_empty() {
        return "The remote agent asks for input but gave no text.".to_string();
    }
    crate::tools::limit_bytes(plano, REMOTE_MAX_BYTES_PREGUNTA).into_owned()
}

impl Espejo {
    /// Pregunta a la persona (por el buzón de los hijos) lo que el remoto pide y espera su respuesta,
    /// con tope de tiempo. Cancelar el hijo o el padre retira la pregunta.
    async fn preguntar(
        &self,
        texto: String,
        espera: std::time::Duration,
        job_cancel: &CancellationToken,
        parent_cancel: &CancellationToken,
    ) -> Entrada {
        let pregunta = crate::question::Question {
            header: "Remote agent".to_string(),
            question: texto,
            options: Vec::new(),
            multi_select: false,
        };
        let (parent, child) = (self.parent, self.child);
        let (bus, label) = (self.bus.clone(), self.label.clone());
        let peticion = self.broker.request_with(
            parent,
            child,
            self.label.clone(),
            vec![pregunta],
            move || {
                bus.publish(LiveEvent::SubagentInputRequested {
                    parent,
                    child,
                    label,
                });
            },
        );
        let resultado = tokio::select! {
            respuestas = peticion => match respuestas {
                Some(respuestas) => Entrada::Respuesta(
                    respuestas
                        .iter()
                        .flat_map(|r| r.selected.iter().cloned())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                None => Entrada::Descartada,
            },
            _ = tokio::time::sleep(espera) => Entrada::Expirada,
            _ = job_cancel.cancelled() => Entrada::Cancelada,
            _ = parent_cancel.cancelled() => {
                job_cancel.cancel();
                Entrada::Cancelada
            }
        };
        // La petición se retira al soltarse; la marca de espera de la interfaz se quita aquí.
        self.bus
            .publish(LiveEvent::SubagentInputResolved { parent, child });
        resultado
    }

    async fn transicion(&self, status: TaskStatus) {
        if let Err(error) = self
            .tracker
            .transition(self.parent, self.child, status)
            .await
        {
            tracing::warn!("persist subagent status: {error}");
        }
    }

    /// Cierra la tarea como fallida con la razón en su mensaje y devuelve el error del trabajo.
    async fn fallar(&self, razon: String) -> Result<Option<JobResult>> {
        self.transicion(estado_con_mensaje(
            TaskState::Failed,
            Some(Message::agent_text(razon.as_str())),
        ))
        .await;
        Err(CoreError::Tool(razon))
    }

    /// Pide al remoto cancelar y deja la tarea local `canceled` con lo que el remoto haya dicho:
    /// sin su confirmación no se afirma que quedó cancelada allí.
    async fn cancelar(&self, id: Option<&str>) -> Result<Option<JobResult>> {
        let nota = match id {
            None => "cancelled before the remote assigned a task id".to_string(),
            Some(id) => {
                match tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, self.cliente.cancel(id)).await {
                    Ok(Ok(tarea)) if tarea.status.state == TaskState::Canceled => {
                        "cancellation confirmed by the remote".to_string()
                    }
                    Ok(Ok(tarea)) => format!(
                        "cancellation requested; the remote reports {}",
                        tarea.status.state.proto_name()
                    ),
                    Ok(Err(RemoteTaskError::NotCancelable)) => {
                        "cancellation requested; the remote says the task is not cancelable"
                            .to_string()
                    }
                    Ok(Err(error)) => {
                        format!("cancellation requested; the remote did not confirm it: {error}")
                    }
                    Err(_) => {
                        "cancellation requested; the remote did not answer in time".to_string()
                    }
                }
            }
        };
        self.transicion(estado_con_mensaje(
            TaskState::Canceled,
            Some(Message::agent_text(nota.as_str())),
        ))
        .await;
        Ok(None)
    }

    /// Cierra la tarea con lo que el remoto dice que acabó (estado terminal).
    async fn terminar(&self, tarea: RemoteTask, handle: &JobHandle) -> Result<Option<JobResult>> {
        if tarea.status.state != TaskState::Completed {
            let razon = tarea
                .status
                .message
                .as_ref()
                .map(Message::text)
                .filter(|texto| !texto.trim().is_empty())
                .unwrap_or_else(|| format!("remote task {}", tarea.status.state.proto_name()));
            self.transicion(estado_con_mensaje(
                tarea.status.state,
                tarea.status.message.clone(),
            ))
            .await;
            return Err(CoreError::Tool(razon));
        }
        let mut artifacts = tarea.artifacts;
        if artifacts.is_empty() {
            if let Some(texto) = tarea
                .status
                .message
                .as_ref()
                .map(Message::text)
                .filter(|texto| !texto.trim().is_empty())
            {
                artifacts.push(Artifact::text_artifact(texto));
            }
        }
        self.store
            .complete_subagent(self.parent, self.child, &artifacts)
            .await?;
        if let Err(error) = self.tracker.publish(self.parent, self.child).await {
            tracing::warn!("publish subagent status: {error}");
        }
        let texto = crate::a2a::artifacts_text(&artifacts);
        handle.append(&texto);
        Ok(Some(JobResult { content: texto }))
    }
}

/// Espejo de una tarea remota: envía el mensaje, la sigue por sondeo y copia su estado y sus
/// artefactos al descriptor local, de modo que el árbol, la cancelación y el presupuesto de tareas
/// la tratan como a cualquier subagente.
async fn seguir_remoto(
    e: EjecucionRemota,
    handle: JobHandle,
    job_cancel: CancellationToken,
) -> Result<Option<JobResult>> {
    let EjecucionRemota {
        cliente,
        parent,
        child,
        prompt,
        tracker,
        store,
        permit,
        puerta,
        parent_cancel,
        poll,
        max_follow,
        broker,
        bus,
        label,
        input_wait,
    } = e;
    let _permit = permit;
    // La compuerta retiene el trabajo hasta que la tarea `submitted` está guardada.
    if puerta.await.is_err() || job_cancel.is_cancelled() {
        return Ok(None);
    }
    let espejo = Espejo {
        cliente: cliente.clone(),
        parent,
        child,
        tracker,
        store,
        broker,
        bus,
        label,
    };
    espejo.transicion(TaskStatus::new(TaskState::Working)).await;

    let enviado = tokio::select! {
        respuesta = cliente.send(Message::user_text(prompt)) => respuesta,
        _ = job_cancel.cancelled() => return espejo.cancelar(None).await,
        _ = parent_cancel.cancelled() => {
            job_cancel.cancel();
            return espejo.cancelar(None).await;
        }
    };
    let mut tarea = match enviado {
        Ok(RemoteReply::Task(tarea)) => tarea,
        Ok(RemoteReply::Message(mensaje)) => {
            // El remoto contestó sin crear tarea: su mensaje es el resultado.
            let tarea = RemoteTask {
                id: String::new(),
                context_id: None,
                status: TaskStatus::new(TaskState::Completed),
                artifacts: vec![Artifact::text_artifact(mensaje.text())],
            };
            return espejo.terminar(tarea, &handle).await;
        }
        Err(error) => return espejo.fallar(format!("remote send failed: {error}")).await,
    };

    let id = tarea.id.clone();
    let mut inicio = tokio::time::Instant::now();
    let mut ultimo = TaskState::Working;
    let mut fallos = 0u32;
    let mut rondas = 0u32;
    // Cada ronda sigue la tarea hasta un estado terminal o hasta que el remoto pide entrada; en este
    // último caso se pregunta a la persona, se responde a la misma tarea y se empieza otra ronda.
    'ronda: loop {
        // A8c: si el remoto declara streaming se sigue el flujo en lugar de sondear. Cualquier fallo
        // al abrirlo, un error a mitad, un cierre antes del estado terminal o una tarea ajena
        // devuelven el seguimiento al sondeo de siempre: el flujo es una mejora, nunca la única vía.
        if !tarea.status.state.is_terminal() && tarea.status.state != TaskState::InputRequired {
            let apertura = tokio::select! {
                apertura = tokio::time::timeout(REMOTE_WATCH_OPEN_TIMEOUT, cliente.watch(&id)) => apertura,
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            };
            if let Ok(Ok(mut flujo)) = apertura {
                let limite = inicio + max_follow;
                loop {
                    tokio::select! {
                        siguiente = futures::StreamExt::next(&mut flujo) => match siguiente {
                            Some(Ok(nueva)) if nueva.id == id => {
                                let estado = nueva.status.state;
                                tarea = nueva;
                                if estado.is_terminal() {
                                    return espejo.terminar(tarea, &handle).await;
                                }
                                if estado != ultimo {
                                    espejo
                                        .transicion(estado_con_mensaje(
                                            estado,
                                            tarea.status.message.clone(),
                                        ))
                                        .await;
                                    ultimo = estado;
                                }
                                // El remoto pide entrada: se atiende fuera del flujo.
                                if estado == TaskState::InputRequired {
                                    break;
                                }
                            }
                            _ => break,
                        },
                        _ = tokio::time::sleep_until(limite) => break,
                        _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                        _ = parent_cancel.cancelled() => {
                            job_cancel.cancel();
                            return espejo.cancelar(Some(&id)).await;
                        }
                    }
                }
            }
        }
        loop {
            let estado = tarea.status.state;
            if estado.is_terminal() {
                return espejo.terminar(tarea, &handle).await;
            }
            if estado != ultimo {
                espejo
                    .transicion(estado_con_mensaje(estado, tarea.status.message.clone()))
                    .await;
                ultimo = estado;
            }
            if estado == TaskState::InputRequired {
                rondas += 1;
                if rondas > REMOTE_MAX_RONDAS_ENTRADA {
                    let _ = tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                    return espejo
                        .fallar(format!(
                            "the remote asked for input more than {REMOTE_MAX_RONDAS_ENTRADA} times"
                        ))
                        .await;
                }
                let texto = pregunta_del_remoto(tarea.status.message.as_ref());
                match espejo
                    .preguntar(texto, input_wait, &job_cancel, &parent_cancel)
                    .await
                {
                    Entrada::Respuesta(respuesta) => {
                        let mut mensaje = Message::user_text(respuesta);
                        mensaje.task_id = Some(id.clone());
                        mensaje.context_id = tarea.context_id.clone();
                        let enviado = tokio::select! {
                            respuesta = cliente.send(mensaje) => respuesta,
                            _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                            _ = parent_cancel.cancelled() => {
                                job_cancel.cancel();
                                return espejo.cancelar(Some(&id)).await;
                            }
                        };
                        match enviado {
                            Ok(RemoteReply::Task(nueva)) if nueva.id == id => tarea = nueva,
                            Ok(RemoteReply::Task(_)) => {
                                return espejo
                                    .fallar("the remote answered about a different task".into())
                                    .await
                            }
                            Ok(RemoteReply::Message(mensaje)) => {
                                // Contestó con un mensaje y sin tarea: ese mensaje es el resultado.
                                let cierre = RemoteTask {
                                    id: id.clone(),
                                    context_id: tarea.context_id.clone(),
                                    status: TaskStatus::new(TaskState::Completed),
                                    artifacts: vec![Artifact::text_artifact(mensaje.text())],
                                };
                                return espejo.terminar(cierre, &handle).await;
                            }
                            Err(error) => {
                                return espejo.fallar(format!("remote send failed: {error}")).await
                            }
                        }
                        if !tarea.status.state.is_terminal() {
                            espejo.transicion(TaskStatus::new(TaskState::Working)).await;
                        }
                        ultimo = TaskState::Working;
                        // Esperar a la persona no cuenta contra el límite de seguimiento.
                        inicio = tokio::time::Instant::now();
                        fallos = 0;
                        continue 'ronda;
                    }
                    Entrada::Expirada => {
                        let _ =
                            tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                        return espejo
                            .fallar("nobody answered the remote agent's question in time".into())
                            .await;
                    }
                    Entrada::Descartada | Entrada::Cancelada => {
                        return espejo.cancelar(Some(&id)).await
                    }
                }
            }
            if inicio.elapsed() >= max_follow {
                // Se avisa al remoto de que ya no interesa, sin esperar su confirmación.
                let _ = tokio::time::timeout(REMOTE_CANCEL_TIMEOUT, cliente.cancel(&id)).await;
                return espejo
                    .fallar(format!(
                        "gave up following the remote task after {} s",
                        max_follow.as_secs()
                    ))
                    .await;
            }
            tokio::select! {
                _ = tokio::time::sleep(poll) => {}
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            }
            let consulta = tokio::select! {
                respuesta = cliente.get(&id) => respuesta,
                _ = job_cancel.cancelled() => return espejo.cancelar(Some(&id)).await,
                _ = parent_cancel.cancelled() => {
                    job_cancel.cancel();
                    return espejo.cancelar(Some(&id)).await;
                }
            };
            match consulta {
                Ok(nueva) if nueva.id == id => {
                    fallos = 0;
                    tarea = nueva;
                }
                Ok(_) => {
                    return espejo
                        .fallar("the remote answered about a different task".into())
                        .await
                }
                Err(RemoteTaskError::NotFound) => {
                    return espejo
                        .fallar("the remote no longer knows the task".into())
                        .await
                }
                Err(error) => {
                    fallos += 1;
                    if fallos >= REMOTE_MAX_FALLOS_SEGUIDOS {
                        return espejo
                            .fallar(format!("lost contact with the remote: {error}"))
                            .await;
                    }
                }
            }
        }
    }
}

/// Resumen de una petición de permiso para el mensaje de `auth-required`: herramienta, acción
/// y detalle recortado, de modo que el padre vea qué se pide sin volcar argumentos enteros.
pub(crate) fn resumen_de_permiso(tool: &str, action: &str, detail: &str) -> String {
    const LIMITE: usize = 160;
    let detalle = detail.trim();
    let resumen = if detalle.is_empty() {
        format!("{tool} {action}")
    } else {
        format!("{tool} {action}: {detalle}")
    };
    if resumen.chars().count() <= LIMITE {
        resumen
    } else {
        let mut recortado: String = resumen.chars().take(LIMITE - 1).collect();
        recortado.push('…');
        recortado
    }
}

/// Validación común del prompt y la etiqueta de un hijo nuevo.
fn validar_peticion(label: &str, prompt: &str) -> Result<()> {
    if prompt.trim().is_empty() || prompt.len() > 32_768 || label.len() > 128 {
        return Err(CoreError::Tool("invalid subagent prompt or label".into()));
    }
    Ok(())
}

/// Prompt de un seguimiento (D4 de A11): el resultado de la tarea anterior, acotado por la regla
/// 8 del decálogo, antepuesto a la petición. Sin resultado se escribe `(no result)` y, si lo hay,
/// el texto del mensaje de su estado.
fn prompt_de_seguimiento(anterior: &SubagentDescriptor, peticion: &str) -> String {
    let resultado = anterior.result_text().unwrap_or_else(|| {
        match anterior
            .status
            .message
            .as_ref()
            .map(|mensaje| mensaje.text())
            .filter(|texto| !texto.trim().is_empty())
        {
            Some(texto) => format!("(no result) {texto}"),
            None => "(no result)".to_string(),
        }
    });
    format!(
        "[Follow-up of subagent {} \"{}\" ({})]\nPrevious result:\n{}\n\n{}",
        anterior.child.0,
        anterior.label,
        anterior.status.state.proto_name(),
        crate::tools::limit_bytes(&resultado, crate::tools::MAX_NOTICE_BYTES),
        peticion
    )
}

/// Valida la selección de skills de una petición directa contra las que declara el perfil. Devuelve las
/// que se cargarán (`None`: todas las declaradas). Rechaza, antes de crear tarea, una selección vacía,
/// repetida, de más de `MAX_SKILLS_PER_AGENT`, con una skill que el perfil no declara, o hecha a un agente
/// que no declara ninguna o es remoto.
fn elegir_skills(
    perfil: Option<&AgentEntry>,
    skills: Option<Vec<String>>,
) -> Result<Option<Vec<String>>> {
    let Some(pedidas) = skills else {
        return Ok(None);
    };
    let declaradas: &[String] = match perfil {
        Some(AgentEntry {
            execution: Execution::Local(spec),
            ..
        }) => &spec.scope.skills,
        _ => &[],
    };
    if declaradas.is_empty() {
        return Err(CoreError::Tool(
            "this agent declares no skills to choose from".into(),
        ));
    }
    if pedidas.is_empty() || pedidas.len() > crate::skills::MAX_SKILLS_PER_AGENT {
        return Err(CoreError::Tool(format!(
            "choose between 1 and {} skills",
            crate::skills::MAX_SKILLS_PER_AGENT
        )));
    }
    let mut vistas = std::collections::HashSet::new();
    for id in &pedidas {
        if !vistas.insert(id.as_str()) {
            return Err(CoreError::Tool(format!("skills: «{id}» is repeated")));
        }
        if !declaradas.contains(id) {
            return Err(CoreError::Tool(format!(
                "skills: «{id}» is not declared by the agent"
            )));
        }
    }
    Ok(Some(pedidas))
}

#[async_trait]
impl SubagentLauncher for InProcessSubagentLauncher {
    async fn start_requested(
        &self,
        parent: SessionId,
        request: SubagentRequest,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&request.label, &request.prompt)?;
        // Orden de la persona usuaria: no hay aprobación de delegación que pedir.
        self.arrancar_con(
            parent,
            request.label,
            request.prompt,
            request.agent,
            Vec::new(),
            false,
            request.skills,
            parent_cancel,
        )
        .await
    }

    async fn start(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&label, &prompt)?;
        self.pedir_aprobacion(&label, &prompt, agent.as_deref())
            .await?;
        self.arrancar(
            parent,
            label,
            prompt,
            agent,
            Vec::new(),
            false,
            parent_cancel,
        )
        .await
    }

    async fn start_read_only_review(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: String,
        cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&label, &prompt)?;
        self.arrancar(parent, label, prompt, Some(agent), Vec::new(), true, cancel)
            .await
    }

    async fn start_local_workflow_agent(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: String,
        cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&label, &prompt)?;
        if self
            .resolver_agente(Some(&agent))?
            .as_ref()
            .is_some_and(|entry| !matches!(entry.execution, Execution::Local(_)))
        {
            return Err(CoreError::Tool("workflow agents must be local".into()));
        }
        self.arrancar(
            parent,
            label,
            prompt,
            Some(agent),
            Vec::new(),
            false,
            cancel,
        )
        .await
    }

    async fn route(
        &self,
        _parent: SessionId,
        label: &str,
        prompt: &str,
    ) -> Result<Option<RouteDecision>> {
        let Some((router, confirm)) = &self.router else {
            return Ok(None);
        };
        // `always`: cada asignación la aprueba la persona usuaria, no se decide por ella.
        if *confirm == ConfirmDelegation::Always {
            return Ok(None);
        }
        let Some(registro) = &self.registry else {
            return Ok(None);
        };
        let candidatos = registro.snapshot().route_candidates();
        let peticion = RouteRequest {
            prompt: prompt.to_string(),
            label: label.to_string(),
        };
        // Un fallo del enrutador nunca impide delegar: se vuelve al comportamiento de siempre.
        let Ok(Some(decision)) = router.route(&peticion, &candidatos).await else {
            return Ok(None);
        };
        // La respuesta se valida contra los candidatos aprobados: un nombre inventado no vale.
        Ok(candidatos
            .iter()
            .any(|c| c.name == decision.agent)
            .then_some(decision))
    }

    async fn start_follow_up(
        &self,
        parent: SessionId,
        label: String,
        prompt: String,
        agent: Option<String>,
        follow_up_of: SessionId,
        parent_cancel: CancellationToken,
    ) -> Result<(SessionId, JobId)> {
        validar_peticion(&label, &prompt)?;
        // Solo se sigue a una tarea terminal de este mismo padre; de otro padre no se revela.
        let anterior = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == follow_up_of)
            .ok_or(CoreError::TaskNotFound(follow_up_of.0))?;
        if !anterior.status.state.is_terminal() {
            return Err(CoreError::Tool(format!(
                "subagent {} is still {}; wait for it to finish, or continue it with send_message or answer",
                follow_up_of.0,
                anterior.status.state.proto_name()
            )));
        }
        let prompt = prompt_de_seguimiento(&anterior, &prompt);
        self.arrancar(
            parent,
            label,
            prompt,
            agent,
            vec![follow_up_of],
            false,
            parent_cancel,
        )
        .await
    }

    async fn control(
        &self,
        parent: SessionId,
        child: SessionId,
        action: &str,
        message: Option<String>,
    ) -> Result<()> {
        if action == "interrupt" {
            return self.cancel(parent, child).await.map(|_| ());
        }
        let descriptor = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or_else(|| CoreError::Tool("unknown subagent".into()))?;
        let job = JobId(descriptor.job_id);
        if descriptor.status.state.is_terminal() {
            return Err(CoreError::Tool("subagent has already finished".into()));
        }
        let snapshot = self
            .jobs
            .list(parent)
            .into_iter()
            .find(|snapshot| snapshot.id == job)
            .ok_or_else(|| CoreError::Tool("unknown subagent".into()))?;
        if !matches!(
            snapshot.status,
            crate::job::JobStatus::Running | crate::job::JobStatus::Stopping
        ) {
            return Err(CoreError::Tool("subagent has already finished".into()));
        }
        match action {
            "send_message" => {
                let message = message
                    .filter(|text| !text.trim().is_empty() && text.len() <= 32_768)
                    .ok_or_else(|| CoreError::Tool("message must be 1 to 32768 bytes".into()))?;
                self.store
                    .append_message(child, &crate::message::Message::user(message))
                    .await
            }
            _ => Err(CoreError::Tool(
                "action must be send_message or interrupt".into(),
            )),
        }
    }

    async fn cancel(&self, parent: SessionId, child: SessionId) -> Result<SubagentDescriptor> {
        let descriptor = self
            .store
            .list_subagents(parent)
            .await?
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .ok_or(CoreError::TaskNotFound(child.0))?;
        if descriptor.status.state.is_terminal() {
            return Err(CoreError::TaskNotCancelable {
                child: child.0,
                state: descriptor.status.state,
            });
        }
        // Solo se ordena parar a un trabajo en marcha: si ya se está deteniendo, la orden se dio
        // antes y repetirla no cambia nada (idempotente mientras no se asiente).
        let job = JobId(descriptor.job_id.clone());
        let en_marcha = self.jobs.list(parent).into_iter().any(|snapshot| {
            snapshot.id == job && snapshot.status == crate::job::JobStatus::Running
        });
        if en_marcha {
            self.jobs.kill(parent, &job)?;
        }
        let asentado = async {
            loop {
                let actual = self.descriptor_of(parent, child).await?;
                if actual.status.state.is_terminal() {
                    return Ok::<_, CoreError>(actual);
                }
                tokio::time::sleep(CANCEL_POLL).await;
            }
        };
        match tokio::time::timeout(self.cancel_wait, asentado).await {
            Ok(resultado) => resultado,
            // No se asentó a tiempo: se devuelve lo último que consta, que puede no ser `Canceled`.
            Err(_) => self.descriptor_of(parent, child).await,
        }
    }

    async fn list(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        self.store.list_subagents(parent).await
    }

    async fn deliver_notices(&self, parent: SessionId) -> Result<Vec<crate::message::Message>> {
        let mut messages = Vec::new();
        for descriptor in self.store.list_subagents(parent).await? {
            if descriptor.status.state != TaskState::Completed || descriptor.notice_delivered {
                continue;
            }
            if let Some(result) = descriptor.result_text() {
                let message = crate::message::Message::user(format!(
                    "[Subagent {} completed]\n{}",
                    descriptor.label,
                    crate::tools::limit_bytes(&result, crate::tools::MAX_NOTICE_BYTES)
                ));
                if self
                    .store
                    .append_subagent_notice(parent, descriptor.child, &message)
                    .await?
                {
                    messages.push(message);
                }
            }
        }
        Ok(messages)
    }

    async fn answer(
        &self,
        parent: SessionId,
        child: SessionId,
        answers: Vec<String>,
    ) -> Result<()> {
        self.broker.answer(parent, child, answers)
    }

    async fn pending_input_notices(
        &self,
        parent: SessionId,
    ) -> Result<Vec<crate::message::Message>> {
        Ok(self
            .broker
            .take_unannounced(parent)
            .into_iter()
            .map(|input| {
                let mut lines = vec![format!("[Subagent {} needs input]", input.label)];
                for (index, question) in input.questions.iter().enumerate() {
                    let options: Vec<&str> = question
                        .options
                        .iter()
                        .map(|option| option.label.as_str())
                        .collect();
                    lines.push(format!("{}. {}", index + 1, question.question));
                    lines.push(format!("   options: {}", options.join(" | ")));
                }
                lines.push(format!(
                    "Reply with subagent_control action=answer child_id={} answers=[<one string per question, in order>]",
                    input.child.0
                ));
                crate::message::Message::user(lines.join("
"))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        can_delegate, child_levels, compose_child_system, depth_clipped, root_levels,
        MAX_PROFILE_PROMPT_BYTES,
    };

    #[test]
    fn a_full_persona_and_a_full_skills_prompt_both_reach_the_child_untouched() {
        let persona = "p".repeat(MAX_PROFILE_PROMPT_BYTES);
        let skills = "s".repeat(crate::skills::MAX_SKILL_PROMPT_TOKENS * 4);
        let sistema = compose_child_system(Some(persona.clone()), &skills).unwrap();
        assert!(sistema.starts_with(&persona));
        assert!(sistema.ends_with(&skills));
        assert_eq!(sistema.len(), persona.len() + 2 + skills.len());
    }

    #[test]
    fn the_persona_is_capped_on_its_own_and_without_skills_it_is_returned_as_is() {
        let larga = "p".repeat(MAX_PROFILE_PROMPT_BYTES * 2);
        let sistema = compose_child_system(Some(larga.clone()), "").unwrap();
        // El recorte añade su marcador, que cabe en la holgura.
        assert!(sistema.len() <= MAX_PROFILE_PROMPT_BYTES + super::TRUNCATION_MARKER_SLACK);
        assert!(sistema.contains("bytes omitted"));
        assert_eq!(compose_child_system(None, ""), None);
        assert_eq!(
            compose_child_system(None, "skills").as_deref(),
            Some("skills")
        );
    }

    #[test]
    fn with_the_default_ceiling_the_child_never_delegates_even_if_its_profile_asks() {
        let raiz = root_levels(1);
        assert!(can_delegate(raiz));
        let hijo = child_levels(raiz, Some(3));
        assert_eq!(hijo, 1);
        assert!(!can_delegate(hijo));
        assert!(depth_clipped(raiz, Some(3)));
        assert!(!depth_clipped(raiz, Some(1)));
        assert!(!depth_clipped(raiz, None));
    }

    #[test]
    fn ceiling_two_and_a_profile_with_two_levels_give_a_grandchild_that_is_a_leaf() {
        let raiz = root_levels(2);
        let hijo = child_levels(raiz, Some(2));
        assert!(can_delegate(hijo));
        let nieto = child_levels(hijo, None);
        assert_eq!(nieto, 1);
        assert!(!can_delegate(nieto));
    }

    #[test]
    fn ceiling_three_gives_a_chain_whose_last_member_is_a_leaf() {
        let hijo = child_levels(root_levels(3), Some(3));
        let nieto = child_levels(hijo, Some(2));
        assert!(can_delegate(hijo) && can_delegate(nieto));
        let bisnieto = child_levels(nieto, Some(3));
        assert_eq!(bisnieto, 1);
        assert!(!can_delegate(bisnieto));
    }

    #[test]
    fn a_profile_without_max_depth_is_a_leaf_even_with_a_high_ceiling() {
        let hijo = child_levels(root_levels(2), None);
        assert_eq!(hijo, 1);
        assert!(!can_delegate(hijo));
        assert!(!depth_clipped(root_levels(2), None));
    }

    #[test]
    fn a_zero_in_the_profile_still_counts_as_a_leaf() {
        assert_eq!(child_levels(root_levels(3), Some(0)), 1);
    }

    use super::*;
    use crate::config::{ProviderConfig, ProviderKind};
    use crate::job::{JobRegistry, JobStatus, LocalJobRegistry};
    use crate::message::FinishReason;
    use crate::permission::PermissionService;
    use crate::provider::testutil::ScriptedProvider;
    use crate::provider::StreamChunk;
    use crate::registry::{
        AgentOrigin, AgentSource, BuiltInSource, FileSource, ProfileLevel, SourceLoad, TrustStore,
    };
    use crate::session::sqlite::SqliteStore;
    use crate::task::TaskState;
    use crate::tools::subagent::SubagentTool;
    use crate::tools::{Tool, ToolCtx, ToolOutput, MAX_NOTICE_BYTES};
    use std::time::Duration;

    #[tokio::test]
    async fn runs_a_child_and_queues_its_parent_notice_without_recursive_tool() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("subagents.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider {
            script: vec![
                StreamChunk::TextDelta("child result".into()),
                StreamChunk::Done(FinishReason::Stop),
            ],
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(SubagentTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 2,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        assert!(runtime.tools.get("subagent").is_none());
        assert!(runtime.tools.get("subagent_control").is_none());
        assert!(runtime.tools.get("subagent_list").is_none());
        let (child, job) = runtime
            .start(
                parent,
                "research".into(),
                "do work".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(runtime
            .control(SessionId(999), child, "interrupt", None)
            .await
            .is_err());
        let outcome = jobs.wait(parent, &job).await.unwrap();

        assert_eq!(outcome.status, JobStatus::Completed);
        assert!(store.messages(child).await.unwrap().iter().any(|message| {
            message.role == crate::message::Role::Assistant
                && message.text().contains("child result")
        }));
        assert!(runtime
            .deliver_notices(parent)
            .await
            .unwrap()
            .iter()
            .any(|message| message.text().contains("child result")));
        assert!(runtime.deliver_notices(parent).await.unwrap().is_empty());
        assert!(runtime
            .control(parent, child, "interrupt", None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn child_at_step_limit_without_final_text_fails_instead_of_completing_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("empty-final.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "echo".into(),
                        input: serde_json::json!({"text": "inspect"}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(EchoTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 1,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        let (child, job) = runtime
            .start(
                parent,
                "review".into(),
                "inspect files".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let outcome = jobs.wait(parent, &job).await.unwrap();
        assert_eq!(outcome.status, JobStatus::Failed);
        espera_estado_del_hijo(&store, parent, child, TaskState::Failed).await;
        let descriptor = store
            .list_subagents(parent)
            .await
            .unwrap()
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .unwrap();
        assert_eq!(descriptor.status.state, TaskState::Failed);
        assert!(descriptor
            .status
            .message_text()
            .unwrap()
            .contains("step limit (1/1)"));
        assert_eq!(descriptor.result_text(), None);
    }

    #[tokio::test]
    async fn reviewer_cannot_execute_a_model_requested_write_tool() {
        let dir = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        escribir_perfil(
            global.path(),
            "reviewer",
            &PERFIL_CON_PERSONA.replace("frontend", "reviewer"),
        );
        let registry = registro_global(global.path());
        let (store, parent) = tienda_de_prueba(dir.path(), "reviewer-write.db").await;
        let attempted = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(WriteMarker(attempted.clone())));
        let provider: Arc<dyn Provider> = Arc::new(WriteAttemptProvider {
            requests: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store,
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(32),
        )
        .with_registry(registry);

        let (_, job) = runtime
            .start_read_only_review(
                parent,
                "review".into(),
                "review changes".into(),
                "reviewer".into(),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("reviewer should finish")
            .unwrap();

        assert_eq!(outcome.status, JobStatus::Completed);
        assert!(!attempted.load(std::sync::atomic::Ordering::SeqCst));
    }

    struct WriteMarker(Arc<std::sync::atomic::AtomicBool>);
    #[async_trait]
    impl Tool for WriteMarker {
        fn name(&self) -> &str {
            "bash"
        }
        fn description(&self) -> &str {
            "write marker"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn run(
            &self,
            _ctx: ToolCtx,
            _input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> Result<ToolOutput> {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(ToolOutput::ok("written"))
        }
    }

    struct WriteAttemptProvider {
        requests: Arc<std::sync::atomic::AtomicUsize>,
    }
    #[async_trait]
    impl Provider for WriteAttemptProvider {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            use std::sync::atomic::Ordering;
            let chunks = if self.requests.fetch_add(1, Ordering::SeqCst) == 0 {
                vec![
                    StreamChunk::ToolCallComplete {
                        id: "write-attempt".into(),
                        name: "bash".into(),
                        input: serde_json::json!({}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]
            } else {
                vec![
                    StreamChunk::TextDelta("review complete".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ]
            };
            Ok(Box::pin(futures::stream::iter(chunks)))
        }
    }

    #[tokio::test]
    async fn parent_notice_text_is_unchanged_after_the_artifact_change() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("aviso.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider {
            script: vec![
                StreamChunk::TextDelta("child result".into()),
                StreamChunk::Done(FinishReason::Stop),
            ],
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(SubagentTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 2,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );
        let (_child, job) = runtime
            .start(
                parent,
                "research".into(),
                "do work".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let avisos = runtime.deliver_notices(parent).await.unwrap();
        assert_eq!(avisos.len(), 1);
        // Texto exacto del aviso al padre: lo que había antes del cambio a artefactos.
        assert_eq!(
            avisos[0].text(),
            "[Subagent research completed]\nchild result"
        );
    }

    #[tokio::test]
    async fn subagent_notice_result_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("tope.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider {
            script: vec![
                StreamChunk::TextDelta("y".repeat(50_000)),
                StreamChunk::Done(FinishReason::Stop),
            ],
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(SubagentTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 2,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );
        let (_child, job) = runtime
            .start(
                parent,
                "research".into(),
                "do work".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let avisos = runtime.deliver_notices(parent).await.unwrap();
        assert_eq!(avisos.len(), 1);
        let texto = avisos[0].text();
        assert!(texto.starts_with("[Subagent research completed]\n"));
        // Tope de 8 000 bytes de resultado más el aviso de recorte; lejos de los 50 000 originales.
        assert!(
            texto.len() < MAX_NOTICE_BYTES + 200,
            "medía {}",
            texto.len()
        );
        // ADAPTADO: el aviso se recorta ahora con cabeza y cola y el marcador «bytes omitted».
        assert!(texto.contains("bytes omitted ...]"));
    }

    #[tokio::test]
    async fn settled_event_is_published_after_the_status_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("settled.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider {
            script: vec![
                StreamChunk::TextDelta("child result".into()),
                StreamChunk::Done(FinishReason::Stop),
            ],
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(SubagentTool));
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 2,
                effort: None,
            })),
            dir.path().to_path_buf(),
            Arc::new(LocalJobRegistry::default()),
            bus,
        );

        let (child, _job) = runtime
            .start(
                parent,
                "research".into(),
                "do work".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let status = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(crate::event::LiveEvent::SubagentSettled {
                        child: settled,
                        status,
                        ..
                    }) if settled == child => return status,
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el hijo debería asentarse");

        assert_eq!(status, TaskState::Completed);
        // Al llegar el evento, el estado final ya debe estar persistido y el aviso listo.
        assert!(store
            .list_subagents(parent)
            .await
            .unwrap()
            .iter()
            .any(|d| d.child == child && d.status.state == TaskState::Completed));
        assert!(runtime
            .deliver_notices(parent)
            .await
            .unwrap()
            .iter()
            .any(|message| message.text().contains("child result")));
    }

    #[tokio::test]
    async fn child_keeps_ask_user_tool() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("subagents.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider { script: vec![] });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        assert!(tools.get("ask_user").is_some());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store,
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 2,
                effort: None,
            })),
            dir.path().to_path_buf(),
            Arc::new(LocalJobRegistry::default()),
            EventBus::new(64),
        );
        assert!(runtime.tools.get("ask_user").is_some());
    }

    /// Proveedor de prueba con un guion por cada paso de la conversación.
    struct QueueProvider {
        scripts: std::sync::Mutex<std::collections::VecDeque<Vec<StreamChunk>>>,
    }

    #[async_trait]
    impl Provider for QueueProvider {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            let script = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
            Ok(Box::pin(futures::stream::iter(script)))
        }
    }

    #[tokio::test]
    async fn interrupt_discards_the_pending_question() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("interrupt.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "ask_user".into(),
                        input: serde_json::json!({"questions": [{
                            "question": "¿Cuál?",
                            "options": [{"label": "Sí"}, {"label": "No"}]
                        }]}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        runtime
            .control(parent, child, "interrupt", None)
            .await
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo interrumpido debería terminar")
            .unwrap();

        assert_eq!(outcome.status, JobStatus::Killed);
        assert!(runtime.pending_inputs(parent).is_empty());
        assert!(runtime
            .answer(parent, child, vec!["Sí".into()])
            .await
            .is_err());
    }

    #[tokio::test]
    async fn killed_job_becomes_canceled() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("killed.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "ask_user".into(),
                        input: serde_json::json!({"questions": [{
                            "question": "¿Cuál?",
                            "options": [{"label": "Sí"}, {"label": "No"}]
                        }]}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.path().to_path_buf(),
            Arc::new(LocalJobRegistry::default()),
            bus,
        );

        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        runtime
            .control(parent, child, "interrupt", None)
            .await
            .unwrap();
        let settled = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(crate::event::LiveEvent::SubagentSettled {
                        child: settled,
                        status,
                        ..
                    }) if settled == child => return status,
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el hijo matado debería asentarse");

        assert_eq!(settled, TaskState::Canceled);
        let listed = store.list_subagents(parent).await.unwrap();
        assert_eq!(listed[0].status.state, TaskState::Canceled);
    }

    #[tokio::test]
    async fn child_that_asks_blocks_until_the_parent_answers() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("ask.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![
                    vec![
                        StreamChunk::ToolCallComplete {
                            id: "c1".into(),
                            name: "ask_user".into(),
                            input: serde_json::json!({"questions": [{
                                "question": "¿Cuál?",
                                "options": [{"label": "Sí"}, {"label": "No"}]
                            }]}),
                        },
                        StreamChunk::Done(FinishReason::ToolCalls),
                    ],
                    vec![
                        StreamChunk::TextDelta("hecho".into()),
                        StreamChunk::Done(FinishReason::Stop),
                    ],
                ]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");
        assert_eq!(runtime.pending_inputs(parent)[0].child, child);

        runtime
            .answer(parent, child, vec!["Sí".into()])
            .await
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo debería terminar tras la respuesta")
            .unwrap();

        assert_eq!(outcome.status, JobStatus::Completed);
        assert!(runtime.pending_inputs(parent).is_empty());
        let messages = store.messages(child).await.unwrap();
        assert!(messages
            .iter()
            .flat_map(|message| message.parts.iter())
            .any(|part| matches!(
                part,
                crate::message::ContentPart::ToolResult { content, .. } if content.contains("Sí")
            )));
    }

    #[tokio::test]
    async fn pending_input_notices_are_delivered_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("notices.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "ask_user".into(),
                        input: serde_json::json!({"questions": [{
                            "question": "¿Cuál?",
                            "options": [{"label": "Sí"}, {"label": "No"}]
                        }]}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        let first = runtime.pending_input_notices(parent).await.unwrap();
        assert_eq!(first.len(), 1);
        let text = first[0].text();
        assert!(text.starts_with("[Subagent obrero needs input]"), "{text}");
        assert!(text.contains("¿Cuál?"), "{text}");
        assert!(text.contains("Sí") && text.contains("No"), "{text}");
        assert!(
            text.contains(&format!(
                "subagent_control action=answer child_id={}",
                child.0
            )),
            "{text}"
        );
        assert!(runtime
            .pending_input_notices(parent)
            .await
            .unwrap()
            .is_empty());

        runtime
            .answer(parent, child, vec!["Sí".into()])
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo debería terminar tras la respuesta")
            .unwrap();
    }

    /// Herramienta trivial que devuelve su entrada; sirve para observar sus eventos desde el padre.
    struct EchoTool;

    #[async_trait]
    impl crate::tools::Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "devuelve su entrada"
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn run(
            &self,
            _ctx: crate::tools::ToolCtx,
            input: serde_json::Value,
            _cancel: CancellationToken,
        ) -> Result<crate::tools::ToolOutput> {
            Ok(crate::tools::ToolOutput::ok(input.to_string()))
        }
    }

    /// Lanzador con la herramienta `echo` y un guion de pasos para el hijo.
    fn echo_launcher(
        dir: &std::path::Path,
        store: Arc<SqliteStore>,
        jobs: Arc<LocalJobRegistry>,
        bus: EventBus,
        script: Vec<Vec<StreamChunk>>,
    ) -> InProcessSubagentLauncher {
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(EchoTool));
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(script.into()),
        });
        InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store,
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.to_path_buf(),
            jobs,
            bus,
        )
    }

    #[tokio::test]
    async fn child_events_are_forwarded_tagged_with_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("forward.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            jobs.clone(),
            bus.clone(),
            vec![
                vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "echo".into(),
                        input: serde_json::json!({"texto": "hola"}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ],
                vec![
                    StreamChunk::TextDelta("listo".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ],
            ],
        );

        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "usa echo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // Se recogen los reenvíos hasta el TurnEnded del hijo (acotado en tiempo).
        let forwarded = tokio::time::timeout(Duration::from_secs(5), async {
            let mut seen = Vec::new();
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::ChildEvent {
                        parent: p,
                        child: c,
                        event,
                    }) => {
                        let finished = c == child && matches!(*event, LiveEvent::TurnEnded);
                        seen.push((p, c, *event));
                        if finished {
                            return seen;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus del padre debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el hijo debería reenviar su TurnEnded");

        assert!(
            forwarded
                .iter()
                .all(|(p, c, _)| *p == parent && *c == child),
            "{forwarded:?}"
        );
        assert!(
            forwarded
                .iter()
                .any(|(_, _, e)| matches!(e, LiveEvent::Delta { text } if text == "listo")),
            "el texto visible del hijo se reenvía: {forwarded:?}"
        );
        let started = forwarded.iter().position(
            |(_, _, e)| matches!(e, LiveEvent::ToolStarted { name, .. } if name.as_str() == "echo"),
        );
        let finished = forwarded.iter().position(|(_, _, e)| {
            matches!(
                e,
                LiveEvent::ToolFinished {
                    is_error: false,
                    ..
                }
            )
        });
        let ended = forwarded
            .iter()
            .position(|(_, _, e)| matches!(e, LiveEvent::TurnEnded));
        let (Some(started), Some(finished), Some(ended)) = (started, finished, ended) else {
            panic!("faltan eventos reenviados: {forwarded:?}");
        };
        assert!(started < finished && finished < ended, "{forwarded:?}");
    }

    #[tokio::test]
    async fn child_messages_are_forwarded_tagged_with_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("messages.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            jobs.clone(),
            bus.clone(),
            vec![vec![
                StreamChunk::TextDelta("listo".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]],
        );

        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "responde".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // Se recogen los reenvíos hasta el TurnEnded del hijo (acotado en tiempo).
        let forwarded = tokio::time::timeout(Duration::from_secs(5), async {
            let mut seen = Vec::new();
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::ChildEvent {
                        parent: p,
                        child: c,
                        event,
                    }) => {
                        let finished = c == child && matches!(*event, LiveEvent::TurnEnded);
                        seen.push((p, c, *event));
                        if finished {
                            return seen;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus del padre debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el hijo debería reenviar su TurnEnded");

        let message = forwarded.iter().find_map(|(p, c, e)| match e {
            LiveEvent::MessageAdded(_) => Some((*p, *c)),
            _ => None,
        });
        assert_eq!(
            message,
            Some((parent, child)),
            "el mensaje del hijo debería llegar al padre etiquetado con su identidad: {forwarded:?}"
        );
        assert!(
            forwarded.iter().any(|(_, _, event)| matches!(
                event,
                LiveEvent::MessageAdded(message)
                    if message.role == crate::message::Role::User
                        && message.origin == crate::message::MessageOrigin::Internal
            )),
            "la tarea asignada al hijo nunca se atribuye a la persona usuaria: {forwarded:?}"
        );
    }

    #[tokio::test]
    async fn forwarder_ends_when_the_child_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("ends.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            jobs.clone(),
            bus.clone(),
            vec![vec![
                StreamChunk::TextDelta("listo".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]],
        );

        let metrics = tokio::runtime::Handle::current().metrics();
        let baseline = metrics.num_alive_tasks();
        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "termina".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo debería terminar")
            .unwrap();
        assert_eq!(outcome.status, JobStatus::Completed);

        // Al terminar el hijo, el reenvío debe acabar y no quedar tareas huérfanas.
        tokio::time::timeout(Duration::from_secs(5), async {
            while metrics.num_alive_tasks() > baseline {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el reenvío debería terminar con el hijo");

        // Todo lo reenviado ya está en el bus; tras el TurnEnded no llega nada más del hijo.
        let mut child_events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let LiveEvent::ChildEvent {
                child: c, event, ..
            } = event
            {
                if c == child {
                    child_events.push(*event);
                }
            }
        }
        assert!(
            matches!(child_events.last(), Some(LiveEvent::TurnEnded)),
            "{child_events:?}"
        );
        assert_eq!(
            child_events
                .iter()
                .filter(|e| matches!(e, LiveEvent::TurnEnded))
                .count(),
            1,
            "{child_events:?}"
        );
    }

    /// Lanzador cuyo hijo pregunta al padre y se queda esperando `input-required`.
    fn lanzador_que_pregunta(
        dir: &std::path::Path,
        store: Arc<SqliteStore>,
        jobs: Arc<LocalJobRegistry>,
        bus: EventBus,
    ) -> InProcessSubagentLauncher {
        let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
            scripts: std::sync::Mutex::new(
                vec![vec![
                    StreamChunk::ToolCallComplete {
                        id: "c1".into(),
                        name: "ask_user".into(),
                        input: serde_json::json!({"questions": [{
                            "question": "¿Cuál?",
                            "options": [{"label": "Sí"}, {"label": "No"}]
                        }]}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ]]
                .into(),
            ),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
        InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store,
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(CodeLoomConfig {
                provider: ProviderConfig {
                    kind: ProviderKind::OpenAiCompat,
                    base_url: "http://localhost".into(),
                    api_key_env: "KEY".into(),
                },
                model: "test".into(),
                max_steps: 4,
                effort: None,
            })),
            dir.to_path_buf(),
            jobs,
            bus,
        )
    }

    /// Espera, con cota de tiempo, a que el almacén registre el estado pedido para el hijo.
    async fn espera_estado_del_hijo(
        store: &SqliteStore,
        parent: SessionId,
        child: SessionId,
        estado: TaskState,
    ) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let actual = store
                    .list_subagents(parent)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|d| d.child == child)
                    .map(|d| d.status.state);
                if actual == Some(estado) {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("el hijo debería quedar en {estado:?}"));
    }

    #[tokio::test]
    async fn launcher_saves_the_child_as_submitted_then_working() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("submitted.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            bus,
            vec![vec![
                StreamChunk::TextDelta("listo".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]],
        );

        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "termina".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let estados = tokio::time::timeout(Duration::from_secs(5), async {
            let mut visto = Vec::new();
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::SubagentChanged(descriptor)) if descriptor.child == child => {
                        visto.push(descriptor.status.state);
                    }
                    Ok(LiveEvent::SubagentSettled { child: settled, .. }) if settled == child => {
                        return visto;
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el hijo debería asentarse");

        assert_eq!(
            estados,
            vec![
                TaskState::Submitted,
                TaskState::Working,
                TaskState::Completed
            ],
            "cada transición se publica en orden"
        );
    }

    /// Responde a las preguntas con una opción fija; `None` simula que no hay nadie a quien preguntar.
    struct FixedAnswer(Option<&'static str>);

    #[async_trait]
    impl crate::permission::PermissionResponder for FixedAnswer {
        async fn decide(
            &self,
            _request: &crate::permission::PermissionRequest,
        ) -> crate::permission::Decision {
            crate::permission::Decision::Allow
        }

        async fn ask(
            &self,
            questions: &[crate::question::Question],
        ) -> Option<Vec<crate::question::Answer>> {
            let choice = self.0?;
            Some(
                questions
                    .iter()
                    .map(|question| crate::question::Answer {
                        question: question.question.clone(),
                        selected: vec![choice.to_string()],
                    })
                    .collect(),
            )
        }
    }

    async fn delegating_with(
        responder: Option<FixedAnswer>,
        confirm: ConfirmDelegation,
    ) -> (Result<(SessionId, JobId)>, usize) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("confirm.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let mut runtime = echo_launcher(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            EventBus::new(64),
            vec![vec![
                StreamChunk::TextDelta("hecho".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]],
        )
        .with_delegation_confirm(confirm);
        runtime.permission = Arc::new(match responder {
            Some(responder) => PermissionService::with_responder(Box::new(responder)),
            None => PermissionService::allow_all(),
        });
        let result = runtime
            .start(
                parent,
                "revisar".into(),
                "revisa el módulo".into(),
                None,
                CancellationToken::new(),
            )
            .await;
        let hijos = store.list_subagents(parent).await.unwrap().len();
        (result, hijos)
    }

    #[tokio::test]
    async fn always_launches_only_after_an_explicit_approval() {
        let (resultado, hijos) = delegating_with(
            Some(FixedAnswer(Some("Approve"))),
            ConfirmDelegation::Always,
        )
        .await;
        assert!(resultado.is_ok(), "{resultado:?}");
        assert_eq!(hijos, 1);
    }

    #[tokio::test]
    async fn always_denies_on_cancel_without_leaving_a_child() {
        let (resultado, hijos) =
            delegating_with(Some(FixedAnswer(Some("Cancel"))), ConfirmDelegation::Always).await;
        let error = resultado.unwrap_err().to_string();
        assert!(error.contains("not approved"), "{error}");
        assert_eq!(hijos, 0, "una denegación no deja sesión hija ni tarea");
    }

    #[tokio::test]
    async fn always_denies_when_nobody_can_answer_as_under_yolo() {
        // `allow_all` (yolo) concede permisos pero no responde preguntas.
        let (sin_responder, hijos) = delegating_with(None, ConfirmDelegation::Always).await;
        assert!(sin_responder.is_err());
        assert_eq!(hijos, 0);
        let (descartada, _) =
            delegating_with(Some(FixedAnswer(None)), ConfirmDelegation::Always).await;
        assert!(descartada.is_err());
    }

    #[tokio::test]
    async fn other_confirmation_modes_do_not_ask() {
        for modo in [ConfirmDelegation::Doubt, ConfirmDelegation::Never] {
            let (resultado, hijos) = delegating_with(None, modo).await;
            assert!(resultado.is_ok(), "{modo:?}: {resultado:?}");
            assert_eq!(hijos, 1);
        }
    }

    #[tokio::test]
    async fn configured_capacity_replaces_the_default_of_eight() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("capacity.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            EventBus::new(64),
            vec![],
        )
        .with_max_active(2);
        let _ocupados: Vec<_> = (0..2)
            .map(|_| runtime.active.clone().try_acquire_owned().unwrap())
            .collect();
        let error = runtime
            .start(
                parent,
                "tercero".into(),
                "hazlo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("maximum of 2 active subagents reached"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn launcher_rejects_when_capacity_is_exhausted_and_records_a_rejected_task() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("rejected.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            bus,
            vec![],
        );
        // Se ocupan los ocho puestos a mano: el noveno hijo debe rechazarse.
        let _ocupados: Vec<_> = (0..8)
            .map(|_| runtime.active.clone().try_acquire_owned().unwrap())
            .collect();

        let error = runtime
            .start(
                parent,
                "noveno".into(),
                "hazlo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("maximum of 8 active subagents reached"),
            "{error}"
        );

        let hijos = store.list_subagents(parent).await.unwrap();
        assert_eq!(hijos.len(), 1, "queda una tarea rechazada");
        let hijo = &hijos[0];
        assert_eq!(hijo.status.state, TaskState::Rejected);
        assert_eq!(
            hijo.status.message_text().as_deref(),
            Some("maximum of 8 active subagents reached")
        );
        assert!(hijo.job_id.is_empty(), "un rechazado no tiene trabajo");

        let publicado = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::SubagentChanged(descriptor))
                        if descriptor.child == hijo.child =>
                    {
                        return descriptor.status.state;
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el rechazo debería publicarse");
        assert_eq!(publicado, TaskState::Rejected);
    }

    #[tokio::test]
    async fn launcher_validation_errors_create_no_task() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("validation.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let runtime = echo_launcher(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            EventBus::new(16),
            vec![],
        );

        let casos = [
            ("obrero".to_string(), "   ".to_string()),
            ("obrero".to_string(), "x".repeat(32_769)),
            ("x".repeat(129), "hola".to_string()),
        ];
        for (etiqueta, prompt) in casos {
            assert!(
                runtime
                    .start(parent, etiqueta, prompt, None, CancellationToken::new())
                    .await
                    .is_err(),
                "debería rechazarse la entrada inválida"
            );
        }
        assert!(
            store.list_subagents(parent).await.unwrap().is_empty(),
            "la validación no crea tarea"
        );
    }

    #[tokio::test]
    async fn interrupt_moves_the_task_to_canceled() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("interrupt_canceled.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime = lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), bus);
        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        runtime
            .control(parent, child, "interrupt", None)
            .await
            .unwrap();
        espera_estado_del_hijo(&store, parent, child, TaskState::Canceled).await;
        let publicado = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::SubagentChanged(descriptor))
                        if descriptor.child == child
                            && descriptor.status.state == TaskState::Canceled =>
                    {
                        return true;
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("la cancelación debería publicarse");
        assert!(publicado);
    }

    #[tokio::test]
    async fn parent_cancel_moves_the_child_to_canceled() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("parent_cancel.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime =
            lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), EventBus::new(64));
        let padre = CancellationToken::new();
        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                padre.clone(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        // Esc del padre: cancela su turno, y con él al hijo, que no puede terminar en `completed`.
        padre.cancel();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo debería terminar al cancelar al padre")
            .unwrap();
        assert_eq!(outcome.status, JobStatus::Killed);
        espera_estado_del_hijo(&store, parent, child, TaskState::Canceled).await;
        assert!(runtime.pending_inputs(parent).is_empty());
    }

    /// Petición grabada: modelo, sistema y esfuerzo.
    type PeticionGrabada = (String, String, Option<String>);

    /// Proveedor que graba cada petición (modelo, sistema y esfuerzo). Si tiene puerta, la
    /// respuesta espera a que se abra, para poder cambiar el registro con el hijo en curso.
    struct ProveedorGrabador {
        peticiones: Arc<std::sync::Mutex<Vec<PeticionGrabada>>>,
        puerta: Option<Arc<tokio::sync::Notify>>,
    }

    #[async_trait]
    impl Provider for ProveedorGrabador {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            self.peticiones.lock().unwrap().push((
                req.model.clone(),
                req.system.clone(),
                req.effort.clone(),
            ));
            if let Some(puerta) = &self.puerta {
                puerta.notified().await;
            }
            Ok(Box::pin(futures::stream::iter(vec![
                StreamChunk::TextDelta("listo".into()),
                StreamChunk::Done(FinishReason::Stop),
            ])))
        }
    }

    /// Proveedor que responde `listo` declarando `entrada` tokens de consumo.
    struct ProveedorConUso {
        entrada: u32,
    }

    #[async_trait]
    impl Provider for ProveedorConUso {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: false,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            Ok(Box::pin(futures::stream::iter(vec![
                StreamChunk::TextDelta("listo".into()),
                StreamChunk::Usage(crate::message::Usage {
                    input_tokens: self.entrada,
                    output_tokens: 0,
                    reported_cost_usd: None,
                }),
                StreamChunk::Done(FinishReason::Stop),
            ])))
        }
    }

    #[tokio::test]
    async fn a_child_inherits_the_budget_and_its_warning_reaches_the_parent_bus() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "presupuesto.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let bus = EventBus::new(256);
        let mut rx = bus.subscribe();
        let provider: Arc<dyn Provider> = Arc::new(ProveedorConUso { entrada: 900 });
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(ToolRegistry::new()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.path().to_path_buf(),
            jobs.clone(),
            bus,
        )
        .with_budget(BudgetConfig {
            session_tokens: Some(1000),
            session_cost_usd: None,
            hard_stop: false,
        });

        let (_, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "tarea".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let aviso = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::ChildEvent { event, .. }) => {
                        if let LiveEvent::BudgetWarning {
                            level_percent,
                            used,
                            ..
                        } = *event
                        {
                            return (level_percent, used);
                        }
                    }
                    Ok(_) => {}
                    Err(error) => panic!("el bus debería seguir abierto: {error}"),
                }
            }
        })
        .await
        .expect("el aviso del hijo debería llegar al bus del padre");
        assert_eq!(aviso, (80, 900.0));
    }

    /// Configuración del padre de las pruebas: modelo `test` y esfuerzo `low`.
    fn config_de_prueba() -> CodeLoomConfig {
        CodeLoomConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://localhost".into(),
                api_key_env: "KEY".into(),
            },
            model: "test".into(),
            max_steps: 4,
            effort: Some("low".into()),
        }
    }

    /// Lanzador de prueba con el proveedor grabador y, si se pasan, un registro y un catálogo.
    fn lanzador_grabador(
        dir: &std::path::Path,
        store: Arc<SqliteStore>,
        jobs: Arc<LocalJobRegistry>,
        proveedor: ProveedorGrabador,
        registro: Option<Arc<AgentRegistry>>,
        catalogo: Option<Catalog>,
    ) -> InProcessSubagentLauncher {
        let provider: Arc<dyn Provider> = Arc::new(proveedor);
        let lanzador = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store,
            Arc::new(ToolRegistry::new()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.to_path_buf(),
            jobs,
            EventBus::new(64),
        );
        let lanzador = match catalogo {
            Some(catalogo) => lanzador.with_catalog(Arc::new(RwLock::new(catalogo))),
            None => lanzador,
        };
        match registro {
            Some(registro) => lanzador.with_registry(registro),
            None => lanzador,
        }
    }

    /// Enrutador de prueba: contesta lo que se le diga.
    struct EnrutadorFijo(Result<Option<RouteDecision>>);

    #[async_trait]
    impl crate::registry::router::AgentRouter for EnrutadorFijo {
        async fn route(
            &self,
            _request: &RouteRequest,
            _candidates: &[crate::registry::router::RouteCandidate],
        ) -> Result<Option<RouteDecision>> {
            match &self.0 {
                Ok(decision) => Ok(decision.clone()),
                Err(_) => Err(CoreError::Tool("el enrutador falló".into())),
            }
        }
    }

    fn decision_para(agente: &str) -> RouteDecision {
        RouteDecision {
            agent: agente.into(),
            reason: "prueba".into(),
            source: crate::registry::router::RouteSource::Rule,
        }
    }

    /// Lanzador con registro (integrado + `revisor` global) y el enrutador dado.
    fn lanzador_con_enrutador(
        dir: &std::path::Path,
        store: Arc<SqliteStore>,
        enrutador: Option<(Result<Option<RouteDecision>>, ConfirmDelegation)>,
    ) -> InProcessSubagentLauncher {
        let global = dir.join("global");
        std::fs::create_dir_all(&global).unwrap();
        escribir_perfil(
            &global,
            "revisor",
            "---\nname: revisor\ndescription: Revisa\n---\n\nCuerpo.\n",
        );
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir,
            store,
            Arc::new(LocalJobRegistry::default()),
            proveedor,
            Some(registro_global(&global)),
            None,
        );
        match enrutador {
            Some((respuesta, modo)) => {
                lanzador.with_router(Arc::new(EnrutadorFijo(respuesta)), modo)
            }
            None => lanzador,
        }
    }

    #[tokio::test]
    async fn route_without_a_router_abstains() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "r1.db").await;
        let lanzador = lanzador_con_enrutador(dir.path(), store, None);

        assert_eq!(lanzador.route(parent, "t", "revisa").await.unwrap(), None);
    }

    #[tokio::test]
    async fn route_returns_the_routers_decision_for_an_approved_agent() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "r2.db").await;
        for modo in [ConfirmDelegation::Doubt, ConfirmDelegation::Never] {
            let lanzador = lanzador_con_enrutador(
                dir.path(),
                store.clone(),
                Some((Ok(Some(decision_para("revisor"))), modo)),
            );

            let decision = lanzador.route(parent, "t", "revisa").await.unwrap();

            assert_eq!(decision.map(|d| d.agent), Some("revisor".to_string()));
        }
    }

    #[tokio::test]
    async fn always_mode_still_asks_so_the_router_does_not_decide() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "r3.db").await;
        let lanzador = lanzador_con_enrutador(
            dir.path(),
            store,
            Some((
                Ok(Some(decision_para("revisor"))),
                ConfirmDelegation::Always,
            )),
        );

        assert_eq!(lanzador.route(parent, "t", "revisa").await.unwrap(), None);
    }

    #[tokio::test]
    async fn an_invented_agent_name_or_a_failing_router_never_blocks_delegation() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "r4.db").await;
        let inventado = lanzador_con_enrutador(
            dir.path(),
            store.clone(),
            Some((
                Ok(Some(decision_para("no-existe"))),
                ConfirmDelegation::Doubt,
            )),
        );
        let roto = lanzador_con_enrutador(
            dir.path(),
            store,
            Some((Err(CoreError::Tool("x".into())), ConfirmDelegation::Doubt)),
        );

        assert_eq!(inventado.route(parent, "t", "revisa").await.unwrap(), None);
        assert_eq!(roto.route(parent, "t", "revisa").await.unwrap(), None);
    }

    async fn tienda_de_prueba(
        dir: &std::path::Path,
        nombre: &str,
    ) -> (Arc<SqliteStore>, SessionId) {
        let store = Arc::new(
            SqliteStore::open(dir.join(nombre).to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        (store, parent)
    }

    /// Registro con el integrado y una carpeta global de perfiles.
    fn registro_global(global: &std::path::Path) -> Arc<AgentRegistry> {
        Arc::new(AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FileSource::new(global.to_path_buf(), ProfileLevel::Global)),
        ]))
    }

    fn escribir_perfil(dir: &std::path::Path, nombre: &str, texto: &str) {
        std::fs::write(dir.join(format!("{nombre}.md")), texto).unwrap();
    }

    const PERFIL_CON_PERSONA: &str = "---\nname: frontend\ndescription: UI\npersona:\n  role: Ingeniero\nmodel: modelo-perfil\neffort: high\n---\n\nCuerpo del agente.\n";

    /// Fuente de prueba con un agente remoto. Nada lo produce todavía (A8).
    struct FuenteRemotaDePrueba;

    impl AgentSource for FuenteRemotaDePrueba {
        fn id(&self) -> &str {
            "remota-de-prueba"
        }
        fn load(&self) -> SourceLoad {
            SourceLoad {
                entries: vec![AgentEntry {
                    name: "remoto".into(),
                    origin: AgentOrigin::Remote {
                        card_url: "https://agente.example/card".into(),
                    },
                    card: crate::card::AgentCard::local(
                        "remoto",
                        "Agente remoto de prueba",
                        "1.0.0",
                    ),
                    execution: Execution::Remote(crate::registry::RemoteSpec {
                        endpoint: "https://agente.example".into(),
                        fetched: None,
                        credentials: Default::default(),
                    }),
                    content_hash: None,
                }],
                problems: Vec::new(),
            }
        }
    }

    #[tokio::test]
    async fn cancel_returns_the_task_canceled_and_drops_its_question() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "cancel-ok.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let runtime =
            lanzador_que_pregunta(dir.path(), store.clone(), jobs.clone(), EventBus::new(64));
        let (child, _job) = runtime
            .start(
                parent,
                "obrero".into(),
                "decide algo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while runtime.pending_inputs(parent).is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería quedar esperando una respuesta");

        // Cancelable en `input-required` (D3): la tarea actualizada vuelve ya asentada.
        let tarea = runtime.cancel(parent, child).await.unwrap();

        assert_eq!(tarea.child, child);
        assert_eq!(tarea.status.state, TaskState::Canceled);
        assert!(runtime.pending_inputs(parent).is_empty());

        // Una tarea ya cancelada no vuelve a ser cancelable (D4).
        match runtime.cancel(parent, child).await {
            Err(CoreError::TaskNotCancelable { child: id, state }) => {
                assert_eq!((id, state), (child.0, TaskState::Canceled));
            }
            otro => panic!("se esperaba TaskNotCancelable, y salió {otro:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_of_a_completed_child_is_not_cancelable() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "cancel-completed.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let runtime = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            None,
            None,
        );
        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "tarea".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();
        espera_estado_del_hijo(&store, parent, child, TaskState::Completed).await;

        match runtime.cancel(parent, child).await {
            Err(CoreError::TaskNotCancelable { state, .. }) => {
                assert_eq!(state, TaskState::Completed);
            }
            otro => panic!("se esperaba TaskNotCancelable, y salió {otro:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_of_a_child_of_another_parent_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "cancel-ajeno.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let runtime = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            None,
            None,
        );
        let (child, job) = runtime
            .start(
                parent,
                "obrero".into(),
                "tarea".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();
        let otro_padre = store.create_session("/project").await.unwrap();

        match runtime.cancel(otro_padre, child).await {
            Err(CoreError::TaskNotFound(id)) => assert_eq!(id, child.0),
            otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
        }
        match runtime.cancel(parent, SessionId(9999)).await {
            Err(CoreError::TaskNotFound(id)) => assert_eq!(id, 9999),
            otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
        }
    }

    /// Hijo cuyo trabajo ignora la orden de parar: nunca termina. Sirve para ver qué hace `cancel`
    /// cuando el éxito no llega.
    async fn hijo_que_no_se_detiene(
        store: &Arc<SqliteStore>,
        jobs: &Arc<LocalJobRegistry>,
        parent: SessionId,
    ) -> SessionId {
        let child = store.create_session("/project").await.unwrap();
        let job = jobs
            .start(JobSpec {
                kind: "subagent".into(),
                label: "terco".into(),
                owner: parent,
                output_limit: 1024,
                run: Box::new(|_handle, _cancel| {
                    Box::pin(async move {
                        std::future::pending::<()>().await;
                        Ok(None)
                    })
                }),
            })
            .unwrap();
        store
            .save_subagent(&SubagentDescriptor {
                parent,
                child,
                job_id: job.id().0.clone(),
                label: "terco".into(),
                status: TaskStatus::new(TaskState::Working),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();
        child
    }

    #[tokio::test]
    async fn cancel_does_not_claim_success_if_the_child_does_not_settle() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "cancel-terco.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let runtime = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            None,
            None,
        );
        let runtime = runtime.with_cancel_wait(Duration::from_millis(150));
        let child = hijo_que_no_se_detiene(&store, &jobs, parent).await;

        let tarea = runtime.cancel(parent, child).await.unwrap();

        assert_eq!(
            tarea.status.state,
            TaskState::Working,
            "no se asentó: no puede decirse que se canceló"
        );
        // El trabajo recibió la orden de parar (D2) pero sigue deteniéndose.
        let job = JobId(tarea.job_id.clone());
        let estado = jobs
            .list(parent)
            .into_iter()
            .find(|snapshot| snapshot.id == job)
            .unwrap()
            .status;
        assert_eq!(estado, JobStatus::Stopping);
    }

    #[tokio::test]
    async fn a_repeated_cancel_while_stopping_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "cancel-repetido.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let runtime = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            None,
            None,
        );
        let runtime = runtime.with_cancel_wait(Duration::from_millis(150));
        let child = hijo_que_no_se_detiene(&store, &jobs, parent).await;

        let primera = runtime.cancel(parent, child).await.unwrap();
        let segunda = runtime
            .cancel(parent, child)
            .await
            .expect("repetir la orden mientras se detiene no es un error (D4)");

        assert_eq!(primera.status.state, TaskState::Working);
        assert_eq!(segunda.status.state, TaskState::Working);
    }

    #[tokio::test]
    async fn start_without_registry_behaves_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "sin-registro.db").await;
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: peticiones.clone(),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            None,
            None,
        );

        // Sin agente y con `default`, sin registro: el mismo camino de siempre.
        for agente in [None, Some(AGENTE_POR_DEFECTO.to_string())] {
            let (_child, job) = lanzador
                .start(
                    parent,
                    "obrero".into(),
                    "hazlo".into(),
                    agente,
                    CancellationToken::new(),
                )
                .await
                .unwrap();
            jobs.wait(parent, &job).await.unwrap();
        }

        let vistas = peticiones.lock().unwrap().clone();
        assert_eq!(vistas.len(), 2);
        for (modelo, sistema, esfuerzo) in vistas {
            assert_eq!(modelo, "test", "el modelo del padre");
            assert_eq!(esfuerzo.as_deref(), Some("low"), "el esfuerzo del padre");
            assert_eq!(
                sistema,
                crate::config::context::load_context_files(dir.path()).join("\n\n"),
                "el sistema es el contexto del proyecto, sin añadidos"
            );
        }
        assert!(store
            .list_subagents(parent)
            .await
            .unwrap()
            .iter()
            .all(|hijo| hijo.agent.is_none() && hijo.agent_origin.is_none()));
    }

    #[tokio::test]
    async fn start_with_a_named_agent_without_registry_is_a_validation_error() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "sin-registro-error.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(dir.path(), store.clone(), jobs, proveedor, None, None);

        let error = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("obrero".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("obrero"), "{error}");
        assert!(store.list_subagents(parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn start_without_agent_uses_default_and_inherits_model() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "defecto.db").await;
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: peticiones.clone(),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            Some(registro_global(global.path())),
            None,
        );

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
        assert_eq!(modelo, "test");
        assert_eq!(esfuerzo.as_deref(), Some("low"));
        assert_eq!(
            sistema,
            crate::config::context::load_context_files(dir.path()).join("\n\n"),
            "`default` no aporta prompt de sistema propio"
        );
        let hijo = &store.list_subagents(parent).await.unwrap()[0];
        assert_eq!(hijo.agent.as_deref(), Some(AGENTE_POR_DEFECTO));
        assert_eq!(hijo.agent_origin.as_deref(), Some("built-in"));
    }

    #[tokio::test]
    async fn start_with_a_file_agent_applies_persona_model_and_effort() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(global.path(), "frontend", PERFIL_CON_PERSONA);
        let (store, parent) = tienda_de_prueba(dir.path(), "perfil.db").await;
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: peticiones.clone(),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store,
            jobs.clone(),
            proveedor,
            Some(registro_global(global.path())),
            None,
        );

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("frontend".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
        assert_eq!(modelo, "modelo-perfil");
        assert_eq!(esfuerzo.as_deref(), Some("high"));
        assert!(sistema.contains("Rol: Ingeniero\n"), "{sistema}");
        assert!(sistema.contains("Cuerpo del agente."), "{sistema}");
    }

    #[tokio::test]
    async fn local_profile_provider_override_does_not_change_the_parent_provider() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(
            global.path(),
            "backend",
            "---\nname: backend\ndescription: Rust\nmodel: recursive/flint-v1.0:free\nprovider:\n  kind: openai_compat\n  base_url: https://openrouter.ai/api/v1\n  api_key_env: OPENROUTER_API_KEY\n---\n\nBackend.\n",
        );
        let (store, parent) = tienda_de_prueba(dir.path(), "proveedor-perfil.db").await;
        let parent_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let child_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let jobs = Arc::new(LocalJobRegistry::default());
        let parent_provider = ProveedorGrabador {
            peticiones: parent_requests.clone(),
            puerta: None,
        };
        let child_requests_for_resolver = child_requests.clone();
        let resolver: Arc<ProfileProviderResolver> = Arc::new(move |config| {
            assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
            Ok(Arc::new(ProveedorGrabador {
                peticiones: child_requests_for_resolver.clone(),
                puerta: None,
            }))
        });
        let lanzador = lanzador_grabador(
            dir.path(),
            store,
            jobs.clone(),
            parent_provider,
            Some(registro_global(global.path())),
            None,
        )
        .with_profile_provider_resolver(resolver);

        let (_, job) = lanzador
            .start(
                parent,
                "backend".into(),
                "implementa una función".into(),
                Some("backend".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        assert!(parent_requests.lock().unwrap().is_empty());
        assert_eq!(
            child_requests.lock().unwrap()[0].0,
            "recursive/flint-v1.0:free"
        );
    }

    struct ProveedorFallido;

    #[async_trait]
    impl Provider for ProveedorFallido {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: false,
                reasoning: false,
                streaming: true,
            }
        }

        async fn stream(
            &self,
            _req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            Err(CoreError::Provider("mock backend failure".into()))
        }
    }

    #[tokio::test]
    async fn local_subagent_failure_is_persisted_for_diagnosis() {
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "fallo-diagnostico.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let provider: Arc<dyn Provider> = Arc::new(ProveedorFallido);
        let runtime = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(ToolRegistry::new()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(64),
        );

        let (child, job) = runtime
            .start(
                parent,
                "backend".into(),
                "revisa arquitectura".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let outcome = jobs.wait(parent, &job).await.unwrap();
        assert_eq!(outcome.status, crate::job::JobStatus::Failed);
        espera_estado_del_hijo(&store, parent, child, TaskState::Failed).await;

        let descriptor = store
            .list_subagents(parent)
            .await
            .unwrap()
            .into_iter()
            .find(|descriptor| descriptor.child == child)
            .unwrap();
        assert_eq!(descriptor.status.state, TaskState::Failed);
        assert!(descriptor
            .status
            .message_text()
            .unwrap()
            .contains("mock backend failure"));
    }

    #[tokio::test]
    async fn start_with_missing_model_in_profile_inherits_from_parent() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(
            global.path(),
            "revisor",
            "---\nname: revisor\ndescription: Revisa\n---\n\nRevisa con cuidado.\n",
        );
        let (store, parent) = tienda_de_prueba(dir.path(), "herencia.db").await;
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: peticiones.clone(),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store,
            jobs.clone(),
            proveedor,
            Some(registro_global(global.path())),
            None,
        );

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("revisor".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let (modelo, sistema, esfuerzo) = peticiones.lock().unwrap()[0].clone();
        assert_eq!(modelo, "test", "sin modelo en el perfil, el del padre");
        assert_eq!(
            esfuerzo.as_deref(),
            Some("low"),
            "sin esfuerzo, el del padre"
        );
        assert!(sistema.contains("Revisa con cuidado."), "{sistema}");
    }

    #[tokio::test]
    async fn start_with_unknown_agent_creates_no_task() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "inexistente.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs,
            proveedor,
            Some(registro_global(global.path())),
            None,
        );

        let error = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("no-existe".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("no-existe"), "{error}");
        assert!(store.list_subagents(parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn start_with_pending_project_agent_creates_no_task_and_says_so() {
        let proyecto = tempfile::tempdir().unwrap();
        let datos = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(
            proyecto.path(),
            "revisor",
            &PERFIL_CON_PERSONA.replace("frontend", "revisor"),
        );
        let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
        let registro = AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FileSource::new(
                proyecto.path().to_path_buf(),
                ProfileLevel::Project,
            )),
        ])
        .with_trust(almacen, proyecto.path().to_path_buf(), false);
        let (store, parent) = tienda_de_prueba(dir.path(), "pendiente.db").await;
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            proveedor,
            Some(Arc::new(registro)),
            None,
        );

        let error = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("revisor".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("pendiente de aprobación"),
            "{error}"
        );
        assert!(error.to_string().contains("revisor"), "{error}");
        assert!(store.list_subagents(parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn start_with_remote_agent_creates_no_task() {
        let dir = tempfile::tempdir().unwrap();
        let registro = AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FuenteRemotaDePrueba),
        ]);
        let (store, parent) = tienda_de_prueba(dir.path(), "remoto.db").await;
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            Arc::new(LocalJobRegistry::default()),
            proveedor,
            Some(Arc::new(registro)),
            None,
        );

        let error = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("remoto".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("agente remoto no disponible"),
            "{error}"
        );
        assert!(store.list_subagents(parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn start_with_model_unknown_to_catalog_is_a_validation_error() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(
            global.path(),
            "inventado",
            "---\nname: inventado\ndescription: Modelo falso\nmodel: m-inventado\n---\n\nCuerpo.\n",
        );
        escribir_perfil(
            global.path(),
            "real",
            "---\nname: real\ndescription: Modelo conocido\nmodel: m-real\n---\n\nCuerpo.\n",
        );
        // El catálogo tiene un proveedor con la URL base del padre (`http://localhost`).
        let catalogo = Catalog::from_json(
            r#"{"p": {"id": "p", "npm": "@ai-sdk/openai-compatible", "api": "http://localhost",
                "models": {"m-real": {"id": "m-real", "tool_call": true}}}}"#,
        )
        .unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "catalogo.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            Some(registro_global(global.path())),
            Some(catalogo),
        );

        let error = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("inventado".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("m-inventado"), "{error}");
        assert!(store.list_subagents(parent).await.unwrap().is_empty());

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("real".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();
    }

    #[tokio::test]
    async fn reload_after_start_does_not_change_the_running_task() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(
            global.path(),
            "frontend",
            "---\nname: frontend\ndescription: UI\nmodel: modelo-v1\n---\n\nv1.\n",
        );
        let (store, parent) = tienda_de_prueba(dir.path(), "recarga.db").await;
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let puerta = Arc::new(tokio::sync::Notify::new());
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: peticiones.clone(),
            puerta: Some(puerta.clone()),
        };
        let registro = registro_global(global.path());
        let lanzador = lanzador_grabador(
            dir.path(),
            store,
            jobs.clone(),
            proveedor,
            Some(registro.clone()),
            None,
        );

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("frontend".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // El hijo queda retenido en la puerta, con su petición ya registrada.
        tokio::time::timeout(Duration::from_secs(5), async {
            while peticiones.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("el hijo debería haber hecho su petición");

        escribir_perfil(
            global.path(),
            "frontend",
            "---\nname: frontend\ndescription: UI\nmodel: modelo-v2\n---\n\nv2.\n",
        );
        registro.reload();
        let recargado = match &registro.snapshot().get("frontend").unwrap().execution {
            Execution::Local(spec) => spec.model.clone(),
            Execution::Remote(_) => None,
        };
        assert_eq!(
            recargado.as_deref(),
            Some("modelo-v2"),
            "la recarga sí cambió"
        );

        puerta.notify_one();
        let outcome = tokio::time::timeout(Duration::from_secs(5), jobs.wait(parent, &job))
            .await
            .expect("el hijo debería terminar")
            .unwrap();
        assert_eq!(outcome.status, JobStatus::Completed);
        assert_eq!(
            peticiones.lock().unwrap()[0].0,
            "modelo-v1",
            "la tarea en curso no cambia"
        );
    }

    #[tokio::test]
    async fn descriptor_records_agent_name_and_origin() {
        let global = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        escribir_perfil(global.path(), "frontend", PERFIL_CON_PERSONA);
        let (store, parent) = tienda_de_prueba(dir.path(), "descriptor.db").await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let lanzador = lanzador_grabador(
            dir.path(),
            store.clone(),
            jobs.clone(),
            proveedor,
            Some(registro_global(global.path())),
            None,
        );

        let (_child, job) = lanzador
            .start(
                parent,
                "obrero".into(),
                "hazlo".into(),
                Some("frontend".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let hijo = &store.list_subagents(parent).await.unwrap()[0];
        assert_eq!(hijo.agent.as_deref(), Some("frontend"));
        let origen = AgentOrigin::File {
            path: global.path().join("frontend.md"),
            level: ProfileLevel::Global,
        };
        assert_eq!(
            hijo.agent_origin.as_deref(),
            Some(origen.as_text().as_str())
        );
    }

    #[tokio::test]
    async fn tracker_refuses_to_leave_a_terminal_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            SqliteStore::open(dir.path().join("tracker.db").to_str().unwrap())
                .await
                .unwrap(),
        );
        let parent = store.create_session("/project").await.unwrap();
        let child = store.create_session("/project").await.unwrap();
        store
            .save_subagent(&SubagentDescriptor {
                parent,
                child,
                job_id: "subagent-1".into(),
                label: "obrero".into(),
                status: TaskStatus::new(TaskState::Completed),
                artifacts: vec![Artifact::text_artifact("hecho")],
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();
        let tracker = TaskTracker::new(store.clone(), EventBus::new(8));

        let error = tracker
            .transition(parent, child, TaskStatus::new(TaskState::Working))
            .await
            .unwrap_err();
        assert!(matches!(error, CoreError::Store(_)), "{error:?}");
        assert_eq!(
            store.list_subagents(parent).await.unwrap()[0].status.state,
            TaskState::Completed
        );
    }

    /// Lanzador de prueba con un hijo ya terminado (`listo`) y el proveedor grabador.
    async fn lanzador_con_un_hijo_terminado(
        dir: &std::path::Path,
        nombre: &str,
    ) -> (
        InProcessSubagentLauncher,
        Arc<SqliteStore>,
        Arc<LocalJobRegistry>,
        SessionId,
        SessionId,
    ) {
        let (store, parent) = tienda_de_prueba(dir, nombre).await;
        let jobs = Arc::new(LocalJobRegistry::default());
        let proveedor = ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        };
        let runtime = lanzador_grabador(dir, store.clone(), jobs.clone(), proveedor, None, None);
        let (anterior, job) = runtime
            .start(
                parent,
                "primero".into(),
                "haz la primera parte".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();
        espera_estado_del_hijo(&store, parent, anterior, TaskState::Completed).await;
        (runtime, store, jobs, parent, anterior)
    }

    #[tokio::test]
    async fn a_follow_up_carries_the_previous_result_in_its_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, jobs, parent, anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento.db").await;

        let (nuevo, job) = runtime
            .start_follow_up(
                parent,
                "segundo".into(),
                "ahora la segunda parte".into(),
                None,
                anterior,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let primero = store.messages(nuevo).await.unwrap()[0].text();
        assert!(
            primero.starts_with("[Follow-up of subagent"),
            "el prompt debería abrir con el encabezado, y es: {primero}"
        );
        assert!(primero.contains(&format!("subagent {} \"primero\"", anterior.0)));
        assert!(primero.contains("(TASK_STATE_COMPLETED)"));
        assert!(primero.contains("Previous result:\nlisto"));
        assert!(primero.ends_with("\n\nahora la segunda parte"));
        let hijos = store.list_subagents(parent).await.unwrap();
        let descriptor = hijos.iter().find(|d| d.child == nuevo).unwrap();
        assert_eq!(descriptor.reference_task_ids, vec![anterior]);
        let previo = hijos.iter().find(|d| d.child == anterior).unwrap();
        assert!(previo.reference_task_ids.is_empty());
    }

    #[tokio::test]
    async fn a_follow_up_shares_the_context_id() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, jobs, parent, anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-contexto.db").await;

        let (nuevo, job) = runtime
            .start_follow_up(
                parent,
                "segundo".into(),
                "sigue".into(),
                None,
                anterior,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let hijos = store.list_subagents(parent).await.unwrap();
        let contexto = |id: SessionId| hijos.iter().find(|d| d.child == id).unwrap().context_id();
        assert_eq!(contexto(nuevo), contexto(anterior));
    }

    #[tokio::test]
    async fn a_follow_up_of_a_running_child_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, jobs, parent, _anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-vivo.db").await;
        let vivo = hijo_que_no_se_detiene(&store, &jobs, parent).await;
        let antes = store.list_subagents(parent).await.unwrap().len();

        match runtime
            .start_follow_up(
                parent,
                "segundo".into(),
                "sigue".into(),
                None,
                vivo,
                CancellationToken::new(),
            )
            .await
        {
            Err(CoreError::Tool(texto)) => {
                assert!(texto.contains("TASK_STATE_WORKING"), "{texto}");
            }
            otro => panic!("se esperaba un error de herramienta, y salió {otro:?}"),
        }
        assert_eq!(store.list_subagents(parent).await.unwrap().len(), antes);
    }

    #[tokio::test]
    async fn a_follow_up_of_another_parents_child_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, _jobs, parent, anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-ajeno.db").await;
        let otro_padre = store.create_session("/project").await.unwrap();

        for referido in [anterior, SessionId(9999)] {
            match runtime
                .start_follow_up(
                    otro_padre,
                    "segundo".into(),
                    "sigue".into(),
                    None,
                    referido,
                    CancellationToken::new(),
                )
                .await
            {
                Err(CoreError::TaskNotFound(id)) => assert_eq!(id, referido.0),
                otro => panic!("se esperaba TaskNotFound, y salió {otro:?}"),
            }
        }
        assert!(store.list_subagents(otro_padre).await.unwrap().is_empty());
        assert_eq!(store.list_subagents(parent).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_previous_result_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, jobs, parent, _anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-tope.db").await;
        let grande = store.create_session("/project").await.unwrap();
        store
            .save_subagent(&SubagentDescriptor {
                parent,
                child: grande,
                job_id: "subagent-grande".into(),
                label: "grande".into(),
                status: TaskStatus::new(TaskState::Completed),
                artifacts: vec![Artifact::text_artifact("x".repeat(100_000))],
                notice_delivered: true,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();

        let (nuevo, job) = runtime
            .start_follow_up(
                parent,
                "segundo".into(),
                "sigue".into(),
                None,
                grande,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let primero = store.messages(nuevo).await.unwrap()[0].text();
        assert!(
            primero.len() < MAX_NOTICE_BYTES + 400,
            "el prompt de seguimiento mide {} bytes",
            primero.len()
        );
        assert!(primero.ends_with("\n\nsigue"));
    }

    #[tokio::test]
    async fn a_follow_up_of_a_child_without_result_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let (runtime, store, jobs, parent, _anterior) =
            lanzador_con_un_hijo_terminado(dir.path(), "seguimiento-vacio.db").await;
        let fallido = store.create_session("/project").await.unwrap();
        store
            .save_subagent(&SubagentDescriptor {
                parent,
                child: fallido,
                job_id: "subagent-fallido".into(),
                label: "fallido".into(),
                status: TaskStatus::new(TaskState::Failed),
                artifacts: Vec::new(),
                notice_delivered: true,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            })
            .await
            .unwrap();

        let (nuevo, job) = runtime
            .start_follow_up(
                parent,
                "segundo".into(),
                "reintenta".into(),
                None,
                fallido,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        jobs.wait(parent, &job).await.unwrap();

        let primero = store.messages(nuevo).await.unwrap()[0].text();
        assert!(primero.contains("(TASK_STATE_FAILED)"), "{primero}");
        assert!(
            primero.contains("Previous result:\n(no result)"),
            "{primero}"
        );
    }

    // ---------------------------------------------------------------- espejo de remotos (A8a, T5a)

    use crate::registry::RemoteFetched;
    use std::collections::VecDeque;

    /// Cómo responde `watch` el cliente guionizado.
    enum GuionFlujo {
        /// Un flujo que entrega estos elementos y se cierra.
        Elementos(Vec<std::result::Result<RemoteTask, RemoteTaskError>>),
        /// No se puede abrir.
        Falla(RemoteTaskError),
        /// Un flujo que se abre y no dice nunca nada.
        Pendiente,
    }

    /// Cliente remoto guionizado: contesta lo que se le diga y anota lo que recibe.
    struct ClienteGuionizado {
        /// `None`: el remoto no ofrece flujo (`watch` falla con «no soportado»).
        flujo: std::sync::Mutex<Option<GuionFlujo>>,
        /// Veces que se ha sondeado con `get`.
        sondeos: std::sync::Mutex<usize>,
        envio: std::sync::Mutex<Option<std::result::Result<RemoteReply, RemoteTaskError>>>,
        /// Respuestas a los envíos posteriores al primero (las respuestas a una pregunta).
        respuestas: std::sync::Mutex<VecDeque<std::result::Result<RemoteReply, RemoteTaskError>>>,
        consultas: std::sync::Mutex<VecDeque<std::result::Result<RemoteTask, RemoteTaskError>>>,
        /// Respuesta cuando la cola de consultas se agota.
        consulta_final: std::sync::Mutex<std::result::Result<RemoteTask, RemoteTaskError>>,
        cancelacion: std::sync::Mutex<std::result::Result<RemoteTask, RemoteTaskError>>,
        enviados: std::sync::Mutex<Vec<Message>>,
        canceladas: std::sync::Mutex<Vec<String>>,
    }

    fn tarea(id: &str, estado: TaskState, mensaje: Option<&str>, resultado: &[&str]) -> RemoteTask {
        RemoteTask {
            id: id.to_string(),
            context_id: None,
            status: TaskStatus {
                message: mensaje.map(Message::agent_text),
                ..TaskStatus::new(estado)
            },
            artifacts: resultado
                .iter()
                .map(|t| Artifact::text_artifact(*t))
                .collect(),
        }
    }

    impl ClienteGuionizado {
        fn nuevo(
            envio: std::result::Result<RemoteReply, RemoteTaskError>,
            consultas: Vec<std::result::Result<RemoteTask, RemoteTaskError>>,
        ) -> Arc<Self> {
            Arc::new(ClienteGuionizado {
                flujo: std::sync::Mutex::new(None),
                sondeos: std::sync::Mutex::new(0),
                envio: std::sync::Mutex::new(Some(envio)),
                respuestas: std::sync::Mutex::new(VecDeque::new()),
                consultas: std::sync::Mutex::new(consultas.into()),
                consulta_final: std::sync::Mutex::new(Ok(tarea(
                    "r1",
                    TaskState::Working,
                    None,
                    &[],
                ))),
                cancelacion: std::sync::Mutex::new(Ok(tarea("r1", TaskState::Canceled, None, &[]))),
                enviados: std::sync::Mutex::new(Vec::new()),
                canceladas: std::sync::Mutex::new(Vec::new()),
            })
        }

        fn trabajando() -> Arc<Self> {
            Self::nuevo(
                Ok(RemoteReply::Task(tarea(
                    "r1",
                    TaskState::Working,
                    None,
                    &[],
                ))),
                vec![],
            )
        }
    }

    #[async_trait]
    impl RemoteTaskClient for ClienteGuionizado {
        async fn send(
            &self,
            message: Message,
        ) -> std::result::Result<RemoteReply, RemoteTaskError> {
            self.enviados.lock().unwrap().push(message);
            if let Some(primero) = self.envio.lock().unwrap().take() {
                return primero;
            }
            self.respuestas
                .lock()
                .unwrap()
                .pop_front()
                .expect("envío no previsto")
        }
        async fn watch(
            &self,
            _id: &str,
        ) -> std::result::Result<
            futures::stream::BoxStream<'static, std::result::Result<RemoteTask, RemoteTaskError>>,
            RemoteTaskError,
        > {
            use futures::StreamExt;
            match self.flujo.lock().unwrap().take() {
                None => Err(RemoteTaskError::Unsupported("sin flujo".into())),
                Some(GuionFlujo::Falla(error)) => Err(error),
                Some(GuionFlujo::Elementos(elementos)) => {
                    Ok(futures::stream::iter(elementos).boxed())
                }
                Some(GuionFlujo::Pendiente) => Ok(futures::stream::pending::<
                    std::result::Result<RemoteTask, RemoteTaskError>,
                >()
                .boxed()),
            }
        }
        async fn get(&self, _id: &str) -> std::result::Result<RemoteTask, RemoteTaskError> {
            *self.sondeos.lock().unwrap() += 1;
            match self.consultas.lock().unwrap().pop_front() {
                Some(respuesta) => respuesta,
                None => self.consulta_final.lock().unwrap().clone(),
            }
        }
        async fn cancel(&self, id: &str) -> std::result::Result<RemoteTask, RemoteTaskError> {
            self.canceladas.lock().unwrap().push(id.to_string());
            self.cancelacion.lock().unwrap().clone()
        }
    }

    /// Fábrica de prueba: entrega siempre el mismo cliente; con motivo, dice que el remoto no se
    /// puede usar (como haría la real si faltase una credencial).
    struct FabricaFija(Arc<ClienteGuionizado>, Option<String>);

    impl RemoteClientFactory for FabricaFija {
        fn client_for(
            &self,
            _name: &str,
            _spec: &crate::registry::RemoteSpec,
        ) -> Option<Arc<dyn RemoteTaskClient>> {
            Some(self.0.clone())
        }

        fn unavailable_reason(
            &self,
            _name: &str,
            _spec: &crate::registry::RemoteSpec,
        ) -> Option<String> {
            self.1.clone()
        }
    }

    struct Entorno {
        lanzador: InProcessSubagentLauncher,
        store: Arc<SqliteStore>,
        parent: SessionId,
        bus: EventBus,
        cliente: Arc<ClienteGuionizado>,
        // Mantiene vivos los directorios temporales.
        _dirs: (tempfile::TempDir, tempfile::TempDir),
    }

    async fn entorno_remoto(
        cliente: Arc<ClienteGuionizado>,
        aprobar: bool,
        requires_auth: bool,
        max_follow: Duration,
    ) -> Entorno {
        let dir = tempfile::tempdir().unwrap();
        let datos = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "remoto.db").await;
        let registro = AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FuenteRemotaDePrueba),
        ])
        .with_trust(
            TrustStore::open(datos.path().to_path_buf()).unwrap(),
            datos.path().to_path_buf(),
            false,
        );
        // Igual que en producción: la tarjeta llega después de cargar el perfil, y solo entonces
        // se puede aprobar.
        registro.set_remote_card(
            &EventBus::new(8),
            "remoto",
            "https://agente.example",
            Ok((
                crate::card::AgentCard::local("remoto", "Agente remoto", "1.0.0"),
                RemoteFetched {
                    card_hash: "abc".into(),
                    jsonrpc_url: "https://agente.example/a2a/v1".into(),
                    tenant: None,
                    requires_auth,
                    security: None,
                    auth_problem: None,
                    streaming: false,
                },
            )),
        );
        if aprobar {
            registro.approve("remoto").unwrap();
        }
        let bus = EventBus::new(256);
        let provider: Arc<dyn Provider> = Arc::new(ProveedorGrabador {
            peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
            puerta: None,
        });
        let lanzador = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(ToolRegistry::new()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.path().to_path_buf(),
            Arc::new(LocalJobRegistry::default()),
            bus.clone(),
        )
        .with_registry(Arc::new(registro))
        .with_remote_clients(Arc::new(FabricaFija(
            cliente.clone(),
            requires_auth.then(|| "falta la variable de entorno AGENTE_KEY".to_string()),
        )))
        .with_remote_timing(Duration::from_millis(5), max_follow);
        Entorno {
            lanzador,
            store,
            parent,
            bus,
            cliente,
            _dirs: (dir, datos),
        }
    }

    impl Entorno {
        async fn lanzar(&self) -> SessionId {
            self.lanzador
                .start(
                    self.parent,
                    "lejano".into(),
                    "haz lo tuyo".into(),
                    Some("remoto".into()),
                    CancellationToken::new(),
                )
                .await
                .unwrap()
                .0
        }

        async fn descriptor(&self, child: SessionId) -> SubagentDescriptor {
            self.store
                .list_subagents(self.parent)
                .await
                .unwrap()
                .into_iter()
                .find(|d| d.child == child)
                .unwrap()
        }

        /// Espera a que el aviso de cierre llegue al bus y devuelve el estado que lleva.
        async fn estado_del_aviso(
            &self,
            rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
        ) -> TaskState {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Ok(LiveEvent::SubagentSettled { status, .. }) = rx.recv().await {
                        return status;
                    }
                }
            })
            .await
            .expect("debería llegar el aviso de cierre")
        }
    }

    #[tokio::test]
    async fn a_remote_agent_runs_to_completion_and_its_result_is_mirrored() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![
                Ok(tarea("r1", TaskState::Working, None, &[])),
                Ok(tarea(
                    "r1",
                    TaskState::Completed,
                    None,
                    &["resultado remoto"],
                )),
            ],
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        let descriptor = entorno.descriptor(hijo).await;
        assert_eq!(descriptor.status.state, TaskState::Completed);
        assert_eq!(descriptor.artifacts[0].text(), "resultado remoto");
        assert_eq!(descriptor.agent.as_deref(), Some("remoto"));
        let enviados = entorno.cliente.enviados.lock().unwrap();
        assert_eq!(enviados.len(), 1);
        assert_eq!(enviados[0].text(), "haz lo tuyo");
    }

    #[tokio::test]
    async fn the_mirrored_states_follow_the_remote_in_order() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![
                Ok(tarea(
                    "r1",
                    TaskState::InputRequired,
                    Some("¿qué color?"),
                    &[],
                )),
                Ok(tarea("r1", TaskState::Working, None, &[])),
                Ok(tarea("r1", TaskState::Completed, None, &["azul"])),
            ],
        );
        cliente
            .respuestas
            .lock()
            .unwrap()
            .push_back(Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))));
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;
        responder(&entorno, hijo, "azul").await;

        let mut vistos: Vec<(TaskState, String)> = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match rx.recv().await {
                    Ok(LiveEvent::SubagentChanged(d)) => {
                        let texto = d
                            .status
                            .message
                            .as_ref()
                            .map(Message::text)
                            .unwrap_or_default();
                        if vistos.last().map(|(s, _)| *s) != Some(d.status.state) {
                            vistos.push((d.status.state, texto));
                        }
                    }
                    Ok(LiveEvent::SubagentSettled { .. }) => return,
                    _ => {}
                }
            }
        })
        .await
        .expect("debería llegar el aviso de cierre");

        let estados: Vec<TaskState> = vistos.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            estados,
            vec![
                TaskState::Submitted,
                TaskState::Working,
                TaskState::InputRequired,
                TaskState::Working,
                TaskState::Completed
            ],
            "{vistos:?}"
        );
        assert_eq!(vistos[2].1, "¿qué color?");
    }

    /// Espera a que el hijo tenga una pregunta pendiente, la contesta y devuelve su texto.
    async fn responder(entorno: &Entorno, hijo: SessionId, texto: &str) -> String {
        let pregunta = esperar_pregunta(entorno, hijo).await;
        entorno
            .lanzador
            .answer(entorno.parent, hijo, vec![texto.to_string()])
            .await
            .unwrap();
        pregunta
    }

    async fn esperar_pregunta(entorno: &Entorno, hijo: SessionId) -> String {
        let limite = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(pendiente) = entorno
                .lanzador
                .pending_inputs(entorno.parent)
                .into_iter()
                .find(|p| p.child == hijo)
            {
                return pendiente.questions[0].question.clone();
            }
            assert!(
                std::time::Instant::now() < limite,
                "no llegó ninguna pregunta"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn pregunta_remota(cliente: &Arc<ClienteGuionizado>, texto: &str) {
        *cliente.envio.lock().unwrap() = Some(Ok(RemoteReply::Task(tarea(
            "r1",
            TaskState::InputRequired,
            Some(texto),
            &[],
        ))));
    }

    #[tokio::test]
    async fn a_remote_input_required_is_asked_and_the_answer_resumes_the_task() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿qué color?");
        cliente
            .respuestas
            .lock()
            .unwrap()
            .push_back(Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Completed,
                None,
                &["azul"],
            ))));
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;
        let pregunta = responder(&entorno, hijo, "azul").await;

        assert_eq!(pregunta, "¿qué color?");
        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        assert_eq!(entorno.descriptor(hijo).await.artifacts[0].text(), "azul");
        let enviados = entorno.cliente.enviados.lock().unwrap();
        assert_eq!(enviados.len(), 2);
        // La respuesta continúa la misma tarea del remoto.
        assert_eq!(enviados[1].text(), "azul");
        assert_eq!(enviados[1].task_id.as_deref(), Some("r1"));
        assert!(entorno.lanzador.pending_inputs(entorno.parent).is_empty());
    }

    #[tokio::test]
    async fn the_remote_question_is_capped_and_cleaned() {
        let cliente = ClienteGuionizado::trabajando();
        let sucia = format!("hola\u{7}\u{1b}[31m{}", "x".repeat(10_000));
        pregunta_remota(&cliente, &sucia);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;

        let hijo = entorno.lanzar().await;
        let pregunta = esperar_pregunta(&entorno, hijo).await;

        assert!(
            pregunta.len() < REMOTE_MAX_BYTES_PREGUNTA + 200,
            "{}",
            pregunta.len()
        );
        assert!(!pregunta.contains('\u{7}') && !pregunta.contains('\u{1b}'));
        assert!(pregunta.starts_with("hola"));
    }

    #[tokio::test]
    async fn an_empty_remote_question_still_shows_something() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "   ");
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;

        let hijo = entorno.lanzar().await;

        assert!(!esperar_pregunta(&entorno, hijo).await.trim().is_empty());
    }

    #[tokio::test]
    async fn a_cancel_while_waiting_cancels_the_remote_task_and_drops_the_question() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿color?");
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();
        let cancel_padre = CancellationToken::new();
        let (hijo, _) = entorno
            .lanzador
            .start(
                entorno.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                cancel_padre.clone(),
            )
            .await
            .unwrap();
        esperar_pregunta(&entorno, hijo).await;

        cancel_padre.cancel();

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Canceled);
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
        assert!(entorno.lanzador.pending_inputs(entorno.parent).is_empty());
    }

    #[tokio::test]
    async fn too_many_question_rounds_fail_the_task() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿otra vez?");
        for _ in 0..REMOTE_MAX_RONDAS_ENTRADA {
            cliente
                .respuestas
                .lock()
                .unwrap()
                .push_back(Ok(RemoteReply::Task(tarea(
                    "r1",
                    TaskState::InputRequired,
                    Some("¿otra vez?"),
                    &[],
                ))));
        }
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;
        for _ in 0..REMOTE_MAX_RONDAS_ENTRADA {
            responder(&entorno, hijo, "sí").await;
        }

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
    }

    #[tokio::test]
    async fn an_unanswered_question_times_out_and_cancels_the_remote_task() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿color?");
        let mut entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        entorno.lanzador = entorno
            .lanzador
            .with_remote_input_wait(Duration::from_millis(80));
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
        let razon = entorno
            .descriptor(hijo)
            .await
            .status
            .message
            .map(|m| m.text())
            .unwrap_or_default();
        assert!(razon.contains("in time"), "{razon}");
    }

    #[tokio::test]
    async fn the_wait_for_the_user_does_not_count_against_the_follow_limit() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿color?");
        cliente
            .respuestas
            .lock()
            .unwrap()
            .push_back(Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Completed,
                None,
                &["a tiempo"],
            ))));
        // Seguimiento máximo de 300 ms; la persona tarda más que eso en contestar.
        let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(300)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;
        esperar_pregunta(&entorno, hijo).await;
        tokio::time::sleep(Duration::from_millis(450)).await;
        entorno
            .lanzador
            .answer(entorno.parent, hijo, vec!["azul".into()])
            .await
            .unwrap();

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
    }

    #[tokio::test]
    async fn a_message_reply_to_the_answer_is_the_result() {
        let cliente = ClienteGuionizado::trabajando();
        pregunta_remota(&cliente, "¿color?");
        cliente
            .respuestas
            .lock()
            .unwrap()
            .push_back(Ok(RemoteReply::Message(Message::agent_text("era azul"))));
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;
        responder(&entorno, hijo, "azul").await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        assert_eq!(
            entorno.descriptor(hijo).await.artifacts[0].text(),
            "era azul"
        );
    }

    #[tokio::test]
    async fn a_failed_remote_task_fails_the_local_one_with_the_remote_message() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Ok(tarea("r1", TaskState::Failed, Some("no pude"), &[]))],
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        let descriptor = entorno.descriptor(hijo).await;
        assert_eq!(descriptor.status.state, TaskState::Failed);
        assert_eq!(descriptor.status.message.unwrap().text(), "no pude");
    }

    #[tokio::test]
    async fn a_rejected_or_canceled_remote_task_keeps_its_own_state_in_the_settled_notice() {
        for estado in [TaskState::Rejected, TaskState::Canceled] {
            let cliente = ClienteGuionizado::nuevo(
                Ok(RemoteReply::Task(tarea(
                    "r1",
                    TaskState::Working,
                    None,
                    &[],
                ))),
                vec![Ok(tarea("r1", estado, Some("porque sí"), &[]))],
            );
            let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
            let mut rx = entorno.bus.subscribe();

            let hijo = entorno.lanzar().await;

            assert_eq!(entorno.estado_del_aviso(&mut rx).await, estado);
            assert_eq!(entorno.descriptor(hijo).await.status.state, estado);
        }
    }

    #[tokio::test]
    async fn a_direct_message_reply_completes_the_task_with_its_text() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Message(Message::agent_text(
                "respuesta directa",
            ))),
            vec![],
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        assert_eq!(
            entorno.descriptor(hijo).await.artifacts[0].text(),
            "respuesta directa"
        );
    }

    #[tokio::test]
    async fn a_failed_send_fails_the_task_visibly() {
        let cliente = ClienteGuionizado::nuevo(Err(RemoteTaskError::Other("boom".into())), vec![]);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        let mensaje = entorno
            .descriptor(hijo)
            .await
            .status
            .message
            .unwrap()
            .text();
        assert!(
            mensaje.contains("remote send failed") && mensaje.contains("boom"),
            "{mensaje}"
        );
    }

    #[tokio::test]
    async fn losing_contact_after_three_failed_polls_fails_the_task() {
        let cliente = ClienteGuionizado::trabajando();
        *cliente.consulta_final.lock().unwrap() = Err(RemoteTaskError::Timeout);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        let mensaje = entorno
            .descriptor(hijo)
            .await
            .status
            .message
            .unwrap()
            .text();
        assert!(mensaje.contains("lost contact"), "{mensaje}");
    }

    #[tokio::test]
    async fn a_remote_task_that_never_finishes_is_given_up_and_the_remote_is_told() {
        let cliente = ClienteGuionizado::trabajando();
        let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        let mensaje = entorno
            .descriptor(hijo)
            .await
            .status
            .message
            .unwrap()
            .text();
        assert!(mensaje.contains("gave up following"), "{mensaje}");
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
    }

    #[tokio::test]
    async fn the_remote_forgetting_the_task_fails_it() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Err(RemoteTaskError::NotFound)],
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        let mensaje = entorno
            .descriptor(hijo)
            .await
            .status
            .message
            .unwrap()
            .text();
        assert!(mensaje.contains("no longer knows"), "{mensaje}");
    }

    #[tokio::test]
    async fn cancel_asks_the_remote_to_cancel_and_says_only_what_it_answered() {
        let casos: Vec<(std::result::Result<RemoteTask, RemoteTaskError>, &str)> = vec![
            (
                Ok(tarea("r1", TaskState::Canceled, None, &[])),
                "cancellation confirmed by the remote",
            ),
            (
                Ok(tarea("r1", TaskState::Working, None, &[])),
                "the remote reports TASK_STATE_WORKING",
            ),
            (
                Err(RemoteTaskError::NotCancelable),
                "the remote says the task is not cancelable",
            ),
            (
                Err(RemoteTaskError::Other("caído".into())),
                "the remote did not confirm it",
            ),
        ];
        for (respuesta, esperado) in casos {
            let cliente = ClienteGuionizado::trabajando();
            *cliente.cancelacion.lock().unwrap() = respuesta;
            let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
            let hijo = entorno.lanzar().await;
            espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Working).await;

            let devuelto = entorno.lanzador.cancel(entorno.parent, hijo).await.unwrap();

            assert_eq!(devuelto.status.state, TaskState::Canceled);
            let mensaje = devuelto.status.message.unwrap().text();
            assert!(mensaje.contains(esperado), "{esperado}: {mensaje}");
            assert_eq!(
                *entorno.cliente.canceladas.lock().unwrap(),
                vec!["r1".to_string()]
            );
        }
    }

    #[tokio::test]
    async fn cancelling_the_parent_cancels_the_remote_task_too() {
        let cliente = ClienteGuionizado::trabajando();
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let cancel_padre = CancellationToken::new();
        let (hijo, _) = entorno
            .lanzador
            .start(
                entorno.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                cancel_padre.clone(),
            )
            .await
            .unwrap();
        espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Working).await;

        cancel_padre.cancel();

        espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Canceled).await;
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
    }

    fn con_flujo(cliente: &Arc<ClienteGuionizado>, guion: GuionFlujo) {
        *cliente.flujo.lock().unwrap() = Some(guion);
    }

    #[tokio::test]
    async fn a_streaming_remote_is_followed_by_the_stream_without_polling() {
        let cliente = ClienteGuionizado::trabajando();
        con_flujo(
            &cliente,
            GuionFlujo::Elementos(vec![
                Ok(tarea("r1", TaskState::Working, None, &[])),
                Ok(tarea("r1", TaskState::Completed, None, &["por el flujo"])),
            ]),
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        let descriptor = entorno.descriptor(hijo).await;
        assert_eq!(descriptor.artifacts[0].text(), "por el flujo");
        assert_eq!(
            *entorno.cliente.sondeos.lock().unwrap(),
            0,
            "no debía sondear"
        );
    }

    #[tokio::test]
    async fn a_stream_cut_before_the_terminal_state_falls_back_to_polling() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Ok(tarea("r1", TaskState::Completed, None, &["por sondeo"]))],
        );
        con_flujo(
            &cliente,
            GuionFlujo::Elementos(vec![Ok(tarea("r1", TaskState::Working, None, &[]))]),
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(
            entorno.estado_del_aviso(&mut rx).await,
            TaskState::Completed
        );
        assert_eq!(
            entorno.descriptor(hijo).await.artifacts[0].text(),
            "por sondeo"
        );
        assert!(*entorno.cliente.sondeos.lock().unwrap() >= 1);
    }

    #[tokio::test]
    async fn a_stream_that_fails_to_open_or_midway_falls_back_to_polling() {
        for guion in [
            GuionFlujo::Falla(RemoteTaskError::Other("no abre".into())),
            GuionFlujo::Falla(RemoteTaskError::AuthFailed("HTTP 401".into())),
            GuionFlujo::Elementos(vec![
                Ok(tarea("r1", TaskState::Working, None, &[])),
                Err(RemoteTaskError::Other("se cortó".into())),
            ]),
        ] {
            let cliente = ClienteGuionizado::nuevo(
                Ok(RemoteReply::Task(tarea(
                    "r1",
                    TaskState::Working,
                    None,
                    &[],
                ))),
                vec![Ok(tarea("r1", TaskState::Completed, None, &["a salvo"]))],
            );
            con_flujo(&cliente, guion);
            let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
            let mut rx = entorno.bus.subscribe();

            let hijo = entorno.lanzar().await;

            assert_eq!(
                entorno.estado_del_aviso(&mut rx).await,
                TaskState::Completed
            );
            assert_eq!(
                entorno.descriptor(hijo).await.artifacts[0].text(),
                "a salvo"
            );
        }
    }

    #[tokio::test]
    async fn a_stream_about_another_task_is_dropped_and_polling_takes_over() {
        let cliente = ClienteGuionizado::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![Ok(tarea("r1", TaskState::Failed, Some("de verdad"), &[]))],
        );
        // Un flujo que dice que «otra» tarea se completó no puede dar por buena la nuestra.
        con_flujo(
            &cliente,
            GuionFlujo::Elementos(vec![Ok(tarea(
                "otra",
                TaskState::Completed,
                None,
                &["falso"],
            ))]),
        );
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let mut rx = entorno.bus.subscribe();

        let hijo = entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
        assert!(entorno.descriptor(hijo).await.artifacts.is_empty());
    }

    #[tokio::test]
    async fn cancelling_the_parent_while_following_the_stream_cancels_the_remote_task() {
        let cliente = ClienteGuionizado::trabajando();
        con_flujo(&cliente, GuionFlujo::Pendiente);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_secs(60)).await;
        let cancel_padre = CancellationToken::new();
        let (hijo, _) = entorno
            .lanzador
            .start(
                entorno.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                cancel_padre.clone(),
            )
            .await
            .unwrap();
        espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Working).await;

        cancel_padre.cancel();

        espera_estado_del_hijo(&entorno.store, entorno.parent, hijo, TaskState::Canceled).await;
        assert_eq!(
            *entorno.cliente.canceladas.lock().unwrap(),
            vec!["r1".to_string()]
        );
        assert_eq!(*entorno.cliente.sondeos.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_silent_stream_is_given_up_at_the_follow_limit_and_polling_gives_up_too() {
        let cliente = ClienteGuionizado::trabajando();
        con_flujo(&cliente, GuionFlujo::Pendiente);
        let entorno = entorno_remoto(cliente, true, false, Duration::from_millis(200)).await;
        let mut rx = entorno.bus.subscribe();

        entorno.lanzar().await;

        assert_eq!(entorno.estado_del_aviso(&mut rx).await, TaskState::Failed);
    }

    #[tokio::test]
    async fn an_unapproved_remote_creates_no_task() {
        let entorno = entorno_remoto(
            ClienteGuionizado::trabajando(),
            false,
            false,
            Duration::from_secs(60),
        )
        .await;

        let error = entorno
            .lanzador
            .start(
                entorno.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("pendiente de aprobación"),
            "{error}"
        );
        assert!(entorno
            .store
            .list_subagents(entorno.parent)
            .await
            .unwrap()
            .is_empty());
        assert!(entorno.cliente.enviados.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_remote_is_refused_with_the_reason_the_factory_gives() {
        let entorno = entorno_remoto(
            ClienteGuionizado::trabajando(),
            true,
            true,
            Duration::from_secs(60),
        )
        .await;

        let error = entorno
            .lanzador
            .start(
                entorno.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("AGENTE_KEY"), "{error}");
        assert!(error.to_string().contains("no se puede usar"), "{error}");
        assert!(entorno
            .store
            .list_subagents(entorno.parent)
            .await
            .unwrap()
            .is_empty());
    }

    /// Proveedor de prueba que encadena delegaciones según el primer mensaje de cada conversación:
    /// si el plan tiene una entrada para ese texto y aún no hay resultado de herramienta, llama a
    /// `subagent`; si no, responde. Registra las herramientas que vio cada conversación.
    struct ProveedorEnCadena {
        plan: std::collections::HashMap<String, (Option<String>, String)>,
        herramientas: Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
        puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
        /// Tokens de entrada que declara cada respuesta; con 0 no se emite consumo.
        consumo: Arc<std::sync::atomic::AtomicU32>,
        /// Primer mensaje de la conversación de cada petición recibida, en orden.
        peticiones: Arc<std::sync::Mutex<Vec<String>>>,
        /// Prompt de sistema de la primera petición de cada conversación.
        sistemas: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    }

    #[async_trait]
    impl Provider for ProveedorEnCadena {
        fn capabilities(&self) -> crate::provider::Capabilities {
            crate::provider::Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            req: crate::provider::ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<crate::provider::ProviderStream> {
            let primero = req
                .messages
                .iter()
                .find(|mensaje| mensaje.role == crate::message::Role::User)
                .map(|mensaje| mensaje.text())
                .unwrap_or_default();
            self.peticiones.lock().unwrap().push(primero.clone());
            self.sistemas
                .lock()
                .unwrap()
                .entry(primero.clone())
                .or_insert_with(|| req.system.clone());
            self.herramientas
                .lock()
                .unwrap()
                .entry(primero.clone())
                .or_insert_with(|| req.tools.iter().map(|tool| tool.name.clone()).collect());
            let con_resultado = req
                .messages
                .iter()
                .any(|mensaje| mensaje.role == crate::message::Role::Tool);
            if let Some(puerta) = self.puertas.get(&primero) {
                puerta.notified().await;
            }
            let mut trozos = match self.plan.get(&primero) {
                Some((agente, siguiente)) if !con_resultado => vec![
                    StreamChunk::ToolCallComplete {
                        id: format!("llamada-{primero}"),
                        name: "subagent".into(),
                        input: serde_json::json!({
                            "label": siguiente,
                            "prompt": siguiente,
                            "agent": agente,
                        }),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ],
                _ => vec![
                    StreamChunk::TextDelta(format!("{primero} listo")),
                    StreamChunk::Done(FinishReason::Stop),
                ],
            };
            let consumo = self.consumo.load(std::sync::atomic::Ordering::SeqCst);
            if consumo > 0 {
                let cierre = trozos.len() - 1;
                trozos.insert(
                    cierre,
                    StreamChunk::Usage(crate::message::Usage {
                        input_tokens: consumo,
                        output_tokens: 1,
                        reported_cost_usd: None,
                    }),
                );
            }
            Ok(Box::pin(futures::stream::iter(trozos)))
        }
    }

    struct EntornoRecursivo {
        _dir: tempfile::TempDir,
        _global: tempfile::TempDir,
        store: Arc<SqliteStore>,
        parent: SessionId,
        jobs: Arc<LocalJobRegistry>,
        lanzador: InProcessSubagentLauncher,
        herramientas: Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
        puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>>,
        consumo: Arc<std::sync::atomic::AtomicU32>,
        peticiones: Arc<std::sync::Mutex<Vec<String>>>,
        sistemas: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    }

    impl EntornoRecursivo {
        /// Directorio de trabajo del lanzador, que hace de raíz del proyecto en las pruebas.
        fn parent_dir(&self) -> PathBuf {
            self._dir.path().to_path_buf()
        }
    }

    /// Lanzador con las tres herramientas de subagentes y `development_workflow` registradas, los
    /// perfiles dados y el plan de delegaciones dado. `techo` es el techo de profundidad del árbol.
    async fn entorno_recursivo(
        techo: u32,
        perfiles: &[(&str, &str)],
        plan: &[(&str, Option<&str>, &str)],
        puertas: &[&str],
    ) -> EntornoRecursivo {
        entorno_recursivo_con_tope(techo, 8, perfiles, plan, puertas).await
    }

    /// Como `entorno_recursivo`, con el tope de subagentes activos del árbol.
    async fn entorno_recursivo_con_tope(
        techo: u32,
        max_activos: usize,
        perfiles: &[(&str, &str)],
        plan: &[(&str, Option<&str>, &str)],
        puertas: &[&str],
    ) -> EntornoRecursivo {
        let puertas: std::collections::HashMap<String, Arc<tokio::sync::Notify>> = puertas
            .iter()
            .map(|primero| ((*primero).to_string(), Arc::new(tokio::sync::Notify::new())))
            .collect();
        let global = tempfile::tempdir().unwrap();
        for (nombre, texto) in perfiles {
            escribir_perfil(global.path(), nombre, texto);
        }
        let dir = tempfile::tempdir().unwrap();
        let (store, parent) = tienda_de_prueba(dir.path(), "recursivo.db").await;
        let herramientas = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let consumo = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let peticiones = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sistemas = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let proveedor: Arc<dyn Provider> = Arc::new(ProveedorEnCadena {
            consumo: consumo.clone(),
            peticiones: peticiones.clone(),
            sistemas: sistemas.clone(),
            plan: plan
                .iter()
                .map(|(primero, agente, siguiente)| {
                    (
                        (*primero).to_string(),
                        (agente.map(str::to_string), (*siguiente).to_string()),
                    )
                })
                .collect(),
            herramientas: herramientas.clone(),
            puertas: puertas.clone(),
        });
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(SubagentTool));
        tools.register(Arc::new(
            crate::tools::subagent_control::SubagentControlTool,
        ));
        tools.register(Arc::new(crate::tools::subagent_list::SubagentListTool));
        tools.register(Arc::new(
            crate::tools::development_workflow::DevelopmentWorkflowTool,
        ));
        let jobs = Arc::new(LocalJobRegistry::default());
        let lanzador = InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(proveedor)),
            store.clone(),
            Arc::new(tools),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config_de_prueba())),
            dir.path().to_path_buf(),
            jobs.clone(),
            EventBus::new(256),
        )
        .with_registry(registro_global(global.path()))
        .with_max_active(max_activos)
        .with_levels_left(root_levels(techo));
        EntornoRecursivo {
            _dir: dir,
            _global: global,
            store,
            parent,
            jobs,
            lanzador,
            herramientas,
            puertas,
            consumo,
            peticiones,
            sistemas,
        }
    }

    fn perfil_con_profundidad(nombre: &str, profundidad: Option<u32>) -> String {
        let extra = profundidad
            .map(|n| format!("max_depth: {n}\n"))
            .unwrap_or_default();
        format!("---\nname: {nombre}\ndescription: prueba\n{extra}---\n\nCuerpo.\n")
    }

    /// Espera (con cota) a que todos los hijos de `padre` estén en estado terminal.
    async fn esperar_hijos_terminales(store: &SqliteStore, padre: SessionId) {
        for _ in 0..200 {
            let hijos = store.list_subagents(padre).await.unwrap();
            if !hijos.is_empty() && hijos.iter().all(|hijo| hijo.status.state.is_terminal()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("los hijos de {padre:?} no terminaron");
    }

    fn herramientas_de(entorno: &EntornoRecursivo, primero: &str) -> Vec<String> {
        entorno
            .herramientas
            .lock()
            .unwrap()
            .get(primero)
            .cloned()
            .unwrap_or_else(|| panic!("la conversación {primero} no llegó al proveedor"))
    }

    #[tokio::test]
    async fn with_the_default_ceiling_a_child_never_sees_the_subagent_tools() {
        let perfil = perfil_con_profundidad("coord", Some(3));
        let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
        let (_hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let vistas = herramientas_de(&e, "hijo");
        for fuera in [
            "subagent",
            "subagent_control",
            "subagent_list",
            "development_workflow",
        ] {
            assert!(!vistas.iter().any(|n| n == fuera), "{fuera} en {vistas:?}");
        }
    }

    #[tokio::test]
    async fn a_child_without_max_depth_is_a_leaf_even_with_a_higher_ceiling() {
        let e = entorno_recursivo(2, &[], &[], &[]).await;
        let (_hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        assert!(!herramientas_de(&e, "hijo").iter().any(|n| n == "subagent"));
    }

    #[tokio::test]
    async fn ceiling_two_and_a_two_level_profile_let_the_child_launch_a_leaf_grandchild() {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &[],
        )
        .await;
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        esperar_hijos_terminales(&e.store, hijo).await;

        let del_hijo = herramientas_de(&e, "hijo");
        assert!(del_hijo.iter().any(|n| n == "subagent"), "{del_hijo:?}");
        assert!(
            del_hijo.iter().any(|n| n == "subagent_list"),
            "{del_hijo:?}"
        );
        assert!(
            !del_hijo.iter().any(|n| n == "development_workflow"),
            "{del_hijo:?}"
        );
        let del_nieto = herramientas_de(&e, "nieto");
        assert!(!del_nieto.iter().any(|n| n == "subagent"), "{del_nieto:?}");
        // El nieto queda guardado como tarea de su padre, con su resultado.
        let nietos = e.store.list_subagents(hijo).await.unwrap();
        assert_eq!(nietos.len(), 1);
        assert_eq!(nietos[0].status.state, TaskState::Completed);
    }

    #[tokio::test]
    async fn ceiling_three_builds_a_chain_whose_last_member_is_a_leaf() {
        let a = perfil_con_profundidad("a", Some(3));
        let b = perfil_con_profundidad("b", Some(2));
        let e = entorno_recursivo(
            3,
            &[("a", a.as_str()), ("b", b.as_str())],
            &[("hijo", Some("b"), "nieto"), ("nieto", None, "bisnieto")],
            &[],
        )
        .await;
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("a".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        esperar_hijos_terminales(&e.store, hijo).await;
        let nieto = e.store.list_subagents(hijo).await.unwrap()[0].child;
        esperar_hijos_terminales(&e.store, nieto).await;

        assert!(herramientas_de(&e, "hijo").iter().any(|n| n == "subagent"));
        assert!(herramientas_de(&e, "nieto").iter().any(|n| n == "subagent"));
        assert!(!herramientas_de(&e, "bisnieto")
            .iter()
            .any(|n| n == "subagent"));
    }

    /// Recoge, hasta que se agote la espera, los eventos de nieto que llegan al bus de la raíz:
    /// los descriptores cuyo padre es `hijo` y los `ChildEvent` de ese padre.
    async fn eventos_de_nieto(
        rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
        hijo: SessionId,
    ) -> (Vec<SessionId>, usize) {
        let mut descriptores = Vec::new();
        let mut reenviados = 0;
        while let Ok(Ok(evento)) = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await
        {
            match evento {
                LiveEvent::SubagentChanged(d) if d.parent == hijo => descriptores.push(d.child),
                LiveEvent::ChildEvent { parent, .. } if parent == hijo => reenviados += 1,
                _ => {}
            }
        }
        (descriptores, reenviados)
    }

    #[tokio::test]
    async fn the_root_bus_receives_the_grandchild_events_untouched() {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &[],
        )
        .await;
        let mut rx = e.lanzador.bus.subscribe();
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        esperar_hijos_terminales(&e.store, hijo).await;
        let nieto = e.store.list_subagents(hijo).await.unwrap()[0].child;
        let (descriptores, reenviados) = eventos_de_nieto(&mut rx, hijo).await;
        assert!(descriptores.contains(&nieto), "{descriptores:?}");
        assert!(
            reenviados > 0,
            "ningún ChildEvent del nieto llegó a la raíz"
        );
    }

    #[tokio::test]
    async fn with_the_default_ceiling_no_grandchild_event_reaches_the_root_bus() {
        let perfil = perfil_con_profundidad("coord", Some(3));
        let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
        let mut rx = e.lanzador.bus.subscribe();
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let (descriptores, reenviados) = eventos_de_nieto(&mut rx, hijo).await;
        assert!(descriptores.is_empty());
        assert_eq!(reenviados, 0);
    }

    /// El nieto cuenta en la misma familia de presupuesto que la raíz: con `hard_stop`, si el gasto del
    /// hijo ya agotó el límite, el nieto lanzado no llega a pedir ninguna respuesta al modelo.
    #[tokio::test]
    async fn hard_stop_reaches_a_grandchild_through_the_shared_family() {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let mut e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &[],
        )
        .await;
        e.consumo.store(1000, std::sync::atomic::Ordering::SeqCst);
        e.lanzador = e.lanzador.with_budget(crate::budget::BudgetConfig {
            session_tokens: Some(1000),
            session_cost_usd: None,
            hard_stop: true,
        });
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // El hijo puede acabar con error (sin respuesta final): lo que importa es que se detuvo.
        let _ = e.jobs.wait(e.parent, &job).await;
        esperar_hijos_terminales(&e.store, hijo).await;
        let peticiones = e.peticiones.lock().unwrap().clone();
        assert_eq!(
            peticiones.iter().filter(|p| p.as_str() == "hijo").count(),
            1,
            "{peticiones:?}"
        );
        assert_eq!(
            peticiones.iter().filter(|p| p.as_str() == "nieto").count(),
            0,
            "el nieto no debe pedir respuesta con la familia agotada: {peticiones:?}"
        );
    }

    /// Prompts de sistema del hijo y del nieto con el modo de delegación dado en el lanzador.
    async fn sistemas_con_modo(modo: Option<crate::config::DelegationMode>) -> (String, String) {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let mut e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &[],
        )
        .await;
        e.lanzador = e.lanzador.with_delegation_mode(modo);
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        esperar_hijos_terminales(&e.store, hijo).await;
        let sistemas = e.sistemas.lock().unwrap();
        (sistemas["hijo"].clone(), sistemas["nieto"].clone())
    }

    #[tokio::test]
    async fn a_delegating_child_gets_the_delegation_rule_of_the_root() {
        use crate::config::DelegationMode;
        let (hijo, _) = sistemas_con_modo(Some(DelegationMode::OnRequest)).await;
        assert!(hijo.contains("explicitly asks"), "{hijo}");
        let (hijo, _) = sistemas_con_modo(Some(DelegationMode::Proactive)).await;
        assert!(hijo.contains("proactively"), "{hijo}");
    }

    #[tokio::test]
    async fn without_a_delegation_mode_the_child_prompt_has_no_rule() {
        let (hijo, _) = sistemas_con_modo(None).await;
        assert!(!hijo.contains("explicitly asks") && !hijo.contains("proactively"));
    }

    #[tokio::test]
    async fn a_leaf_grandchild_gets_no_delegation_rule() {
        use crate::config::DelegationMode;
        let (_, nieto) = sistemas_con_modo(Some(DelegationMode::Proactive)).await;
        assert!(!nieto.contains("proactively"), "{nieto}");
    }

    /// Un agente de catálogo aprobado recibe en su prompt las skills que cita, tomadas de la carpeta de
    /// skills de su catálogo; un agente sin catálogo no las ve.
    #[tokio::test]
    async fn an_approved_catalog_agent_gets_the_skills_of_its_own_catalog() {
        use crate::registry::{BuiltInSource, FileSource, ProfileLevel, TrustStore};
        let mut e = entorno_recursivo(1, &[], &[], &[]).await;
        let catalogo = tempfile::tempdir().unwrap();
        let agentes = catalogo.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agentes).unwrap();
        std::fs::write(
            agentes.join("revisor.md"),
            "---
name: revisor
description: x
scope:
  skills: [\"auditar\"]
---

Cuerpo.
",
        )
        .unwrap();
        let skill = catalogo
            .path()
            .join(".agents")
            .join("skills")
            .join("auditar");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---
name: Auditar
---
Cuerpo de la skill del catálogo.",
        )
        .unwrap();
        let datos = tempfile::tempdir().unwrap();
        let registro = Arc::new(
            AgentRegistry::new(vec![
                Box::new(BuiltInSource),
                Box::new(FileSource::new(agentes, ProfileLevel::Catalog)),
            ])
            .with_trust(
                TrustStore::open(datos.path().to_path_buf()).unwrap(),
                e.parent_dir(),
                false,
            ),
        );
        registro.approve("revisor").unwrap();
        e.lanzador = e.lanzador.with_registry(registro);
        let (_hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("revisor".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
        assert!(
            sistema.contains("Cuerpo de la skill del catálogo."),
            "{sistema}"
        );
    }

    /// Perfil global `rev` que declara las skills dadas, y esas mismas skills en el proyecto del entorno
    /// con el cuerpo `cuerpo-<id>`.
    async fn entorno_con_skills(declaradas: &[&str]) -> EntornoRecursivo {
        let lista = declaradas
            .iter()
            .map(|id| format!("\"{id}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let perfil = format!(
            "---
name: rev
description: x
scope:
  skills: [{lista}]
---

Cuerpo.
"
        );
        let e = entorno_recursivo(1, &[("rev", perfil.as_str())], &[], &[]).await;
        for id in declaradas {
            let dir = e.parent_dir().join(".agents").join("skills").join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("SKILL.md"),
                format!(
                    "---
name: {id}
---
cuerpo-{id}"
                ),
            )
            .unwrap();
        }
        e
    }

    fn peticion(skills: Option<&[&str]>) -> SubagentRequest {
        SubagentRequest {
            label: "h".into(),
            prompt: "hijo".into(),
            agent: Some("rev".into()),
            skills: skills.map(|ids| ids.iter().map(|id| id.to_string()).collect()),
        }
    }

    #[tokio::test]
    async fn a_user_selection_loads_only_the_chosen_skills_of_the_profile() {
        let e = entorno_con_skills(&["a", "b", "c"]).await;
        let (_hijo, job) = e
            .lanzador
            .start_requested(
                e.parent,
                peticion(Some(&["a", "c"])),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
        assert!(
            sistema.contains("cuerpo-a") && sistema.contains("cuerpo-c"),
            "{sistema}"
        );
        assert!(!sistema.contains("cuerpo-b"), "{sistema}");
    }

    #[tokio::test]
    async fn without_a_selection_every_declared_skill_loads_as_before() {
        let e = entorno_con_skills(&["a", "b"]).await;
        let (_hijo, job) = e
            .lanzador
            .start_requested(e.parent, peticion(None), CancellationToken::new())
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let sistema = e.sistemas.lock().unwrap()["hijo"].clone();
        assert!(
            sistema.contains("cuerpo-a") && sistema.contains("cuerpo-b"),
            "{sistema}"
        );
    }

    #[tokio::test]
    async fn an_invalid_selection_is_rejected_before_creating_any_task() {
        let e = entorno_con_skills(&["a", "b"]).await;
        let diecisiete: Vec<String> = (0..17).map(|n| format!("s{n}")).collect();
        let diecisiete_refs: Vec<&str> = diecisiete.iter().map(String::as_str).collect();
        for malas in [
            vec!["z"],       // no declarada
            vec!["a", "a"],  // repetida
            vec![],          // vacía
            diecisiete_refs, // demasiadas
        ] {
            let error = e
                .lanzador
                .start_requested(e.parent, peticion(Some(&malas)), CancellationToken::new())
                .await
                .unwrap_err();
            assert!(error.to_string().contains("skills"), "{malas:?}: {error}");
        }
        assert!(e.store.list_subagents(e.parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_selection_for_an_agent_without_skills_is_rejected() {
        let e = entorno_recursivo(1, &[], &[], &[]).await;
        let mut pedida = peticion(Some(&["a"]));
        pedida.agent = None;
        let error = e
            .lanzador
            .start_requested(e.parent, pedida, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("skills"), "{error}");
        assert!(e.store.list_subagents(e.parent).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_profile_clipped_by_the_ceiling_is_reported_once() {
        let perfil = perfil_con_profundidad("coord", Some(3));
        let e = entorno_recursivo(1, &[("coord", perfil.as_str())], &[], &[]).await;
        let mut avisos = e.lanzador.bus.subscribe();
        for etiqueta in ["uno", "dos"] {
            let (_hijo, job) = e
                .lanzador
                .start(
                    e.parent,
                    etiqueta.into(),
                    etiqueta.into(),
                    Some("coord".into()),
                    CancellationToken::new(),
                )
                .await
                .unwrap();
            e.jobs.wait(e.parent, &job).await.unwrap();
        }
        let mut recortes = 0;
        while let Ok(evento) = avisos.try_recv() {
            if let LiveEvent::RuntimeNotice { message } = evento {
                if message.contains("asks for max_depth 3") {
                    recortes += 1;
                }
            }
        }
        assert_eq!(recortes, 1);
    }

    /// Espera (con cota) a que `padre` tenga al menos un hijo guardado y devuelve el primero.
    async fn esperar_primer_hijo(store: &SqliteStore, padre: SessionId) -> SessionId {
        for _ in 0..200 {
            if let Some(hijo) = store.list_subagents(padre).await.unwrap().first() {
                return hijo.child;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("{padre:?} no llegó a tener hijos");
    }

    #[tokio::test]
    async fn the_active_cap_counts_the_whole_tree_not_each_level() {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let e = entorno_recursivo_con_tope(
            2,
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &["hijo", "nieto"],
        )
        .await;
        // La primera petición del hijo pasa; la segunda espera, para que su puesto siga ocupado.
        e.puertas["hijo"].notify_one();
        let (hijo, _job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // El nieto está lanzado y detenido en su puerta: padre e hijo ocupan los dos puestos.
        esperar_primer_hijo(&e.store, hijo).await;
        let rechazo = e
            .lanzador
            .start(
                e.parent,
                "otro".into(),
                "otro".into(),
                None,
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            rechazo
                .to_string()
                .contains("maximum of 2 active subagents"),
            "{rechazo}"
        );
        e.puertas["nieto"].notify_one();
        e.puertas["hijo"].notify_one();
        esperar_hijos_terminales(&e.store, hijo).await;
    }

    #[tokio::test]
    async fn canceling_the_parent_cancels_the_grandchild() {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &["hijo", "nieto"],
        )
        .await;
        e.puertas["hijo"].notify_one();
        let cancelar = CancellationToken::new();
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                cancelar.clone(),
            )
            .await
            .unwrap();
        // El nieto está lanzado y detenido en su puerta, que nadie abre.
        esperar_primer_hijo(&e.store, hijo).await;
        cancelar.cancel();
        e.jobs.wait(e.parent, &job).await.unwrap();
        esperar_hijos_terminales(&e.store, hijo).await;
        let nietos = e.store.list_subagents(hijo).await.unwrap();
        assert_eq!(nietos[0].status.state, TaskState::Canceled);
    }

    #[tokio::test]
    async fn grandchildren_join_the_budget_family_of_the_root_not_of_their_parent() {
        let perfil = perfil_con_profundidad("coord", Some(3));
        let e = entorno_recursivo(3, &[("coord", perfil.as_str())], &[], &[]).await;
        let raiz = e.parent;
        // El lanzador de la raíz agrupa a sus hijos bajo la propia raíz.
        assert_eq!(e.lanzador.familia(raiz), raiz);
        // Uno anidado (el de un hijo) conserva esa misma raíz, lance quien lance.
        let anidado = e.lanzador.anidado(
            e.lanzador.familia(raiz),
            e.lanzador.provider.read().unwrap().clone(),
            config_de_prueba(),
            Arc::new(PermissionService::allow_all()),
            &ToolRegistry::new(),
            2,
        );
        let hijo = SessionId(raiz.0 + 1);
        assert_eq!(anidado.familia(hijo), raiz);
        let bisnieto = anidado.anidado(
            anidado.familia(hijo),
            anidado.provider.read().unwrap().clone(),
            config_de_prueba(),
            Arc::new(PermissionService::allow_all()),
            &ToolRegistry::new(),
            1,
        );
        assert_eq!(bisnieto.familia(SessionId(raiz.0 + 2)), raiz);
        // Y comparten el mismo semáforo de activos.
        assert!(Arc::ptr_eq(&e.lanzador.active, &bisnieto.active));
    }

    /// Con `confirm_delegation = always` y la raíz aprobando, el hijo delega en un nieto: su pregunta
    /// llega al buzón de la raíz como la de cualquier hijo. Devuelve la pregunta vista y cuántos nietos
    /// existen después de contestar `respuesta`.
    async fn aprobar_nieto(respuesta: &str) -> (String, usize) {
        let perfil = perfil_con_profundidad("coord", Some(2));
        let mut e = entorno_recursivo(
            2,
            &[("coord", perfil.as_str())],
            &[("hijo", None, "nieto")],
            &[],
        )
        .await;
        e.lanzador = e
            .lanzador
            .with_delegation_confirm(ConfirmDelegation::Always);
        e.lanzador.permission = Arc::new(PermissionService::with_responder(Box::new(FixedAnswer(
            Some("Approve"),
        ))));
        let (hijo, job) = e
            .lanzador
            .start(
                e.parent,
                "h".into(),
                "hijo".into(),
                Some("coord".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // La delegación del nieto espera la contestación de la persona usuaria, igual que otra pregunta.
        let mut pendiente = None;
        for _ in 0..200 {
            if let Some(entrada) = e.lanzador.pending_inputs(e.parent).into_iter().next() {
                pendiente = Some(entrada);
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let pendiente = pendiente.expect("la delegación del nieto no pidió aprobación");
        assert_eq!(pendiente.child, hijo);
        assert!(
            e.store.list_subagents(hijo).await.unwrap().is_empty(),
            "el nieto no debe existir antes de la aprobación"
        );
        let pregunta = pendiente.questions[0].question.clone();
        e.lanzador
            .answer(e.parent, hijo, vec![respuesta.to_string()])
            .await
            .unwrap();
        e.jobs.wait(e.parent, &job).await.unwrap();
        let nietos = e.store.list_subagents(hijo).await.unwrap();
        for nieto in &nietos {
            esperar_hijos_terminales(&e.store, hijo).await;
            assert_eq!(nieto.parent, hijo);
        }
        (pregunta, nietos.len())
    }

    #[tokio::test]
    async fn a_grandchild_delegation_asks_for_approval_and_launches_only_when_approved() {
        let (pregunta, nietos) = aprobar_nieto("Approve").await;
        assert!(pregunta.contains("Task: nieto"), "{pregunta}");
        assert_eq!(nietos, 1);
    }

    #[tokio::test]
    async fn a_canceled_grandchild_delegation_leaves_no_grandchild() {
        let (_pregunta, nietos) = aprobar_nieto("Cancel").await;
        assert_eq!(nietos, 0);
    }
}
