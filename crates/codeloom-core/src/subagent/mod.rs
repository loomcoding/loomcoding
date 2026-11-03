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
        let delegation_tools = [
            "subagent",
            "subagent_control",
            "subagent_list",
            "artifact_read",
        ]
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
                    .without("artifact_read")
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
                    .without("artifact_read")
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
            phase: SubagentPhase::Rejected,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
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
        let mut child_tools = if read_only {
            scoped_tools.only(&["view", "glob", "grep"])
        } else {
            scoped_tools
        };
        // Solo los hijos pueden dejar avances; la sesión raíz no tiene tarea a la que adjuntarlos.
        child_tools.register(Arc::new(
            crate::tools::artifact_publish::ArtifactPublishTool,
        ));
        let child_tools = Arc::new(child_tools);
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
                        // El motivo importa: antes todo se reportaba como "sin respuesta
                        // final tras N pasos", que sonaba a presupuesto agotado incluso
                        // cuando el corte había sido por repetición de tool calls.
                        let reason = if steps >= child_max_steps {
                            format!(
                                "subagent reached its step limit ({steps}/{child_max_steps}) without a final response"
                            )
                        } else {
                            format!(
                                "subagent produced no final response after {steps} steps \
                                 (the turn was cut early: repeated tool calls or cancellation)"
                            )
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
                        // El avance que publica el propio hijo cambia su descriptor: sube tal cual, sin
                        // envolver, como el resto de cambios de tarea que publica el lanzador.
                        if matches!(&event, LiveEvent::SubagentChanged(d) if d.parent == parent && d.child == child)
                        {
                            forward_bus.publish(event);
                        } else if event.is_child_relevant() {
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
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
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
                let mut texto = format!(
                    "[Subagent {} completed]\n{}",
                    descriptor.label,
                    crate::tools::limit_bytes(&result, crate::tools::MAX_NOTICE_BYTES)
                );
                // Los avances no se vuelcan: se nombran con su tamaño y se leen con `artifact_read`.
                let avances: Vec<String> = descriptor
                    .progress_artifacts()
                    .map(|a| {
                        format!(
                            "{} ({} bytes)",
                            a.name.as_deref().unwrap_or("?"),
                            a.text().len()
                        )
                    })
                    .collect();
                if !avances.is_empty() {
                    texto.push_str(&format!(
                        "\n[Progress notes: {}; read with artifact_read]",
                        avances.join(", ")
                    ));
                }
                let message = crate::message::Message::user(texto);
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
mod tests;
