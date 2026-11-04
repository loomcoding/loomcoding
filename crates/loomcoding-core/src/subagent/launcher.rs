//! Estado y construcción del lanzador de subagentes en proceso.

use super::*;

pub struct InProcessSubagentLauncher {
    pub(super) provider: Arc<RwLock<Arc<dyn Provider>>>,
    pub(super) store: Arc<dyn SessionStore>,
    pub(super) tools: Arc<ToolRegistry>,
    pub(super) permission: Arc<PermissionService>,
    pub(super) config: Arc<RwLock<LoomcodingConfig>>,
    pub(super) cwd: PathBuf,
    pub(super) jobs: Arc<dyn JobRegistry>,
    pub(super) active: Arc<Semaphore>,
    /// Tope de subagentes activos, el mismo con que se creó `active`; sirve al mensaje de rechazo.
    pub(super) max_active: usize,
    /// Modo de confirmación al delegar. Con `Always`, cada delegación exige aprobación humana (C3).
    pub(super) confirm: ConfirmDelegation,
    /// Mando de delegación (`[agents] delegation`): la regla de prompt que también reciben los hijos que delegan.
    pub(super) delegation: Option<crate::config::DelegationMode>,
    pub(super) bus: EventBus,
    pub(super) broker: Arc<InputBroker>,
    /// Catálogo compartido con el agente padre; cada hijo lo usa para sus apuntes de consumo.
    pub(super) catalog: Arc<RwLock<Catalog>>,
    /// Registro de agentes. Sin él solo existe el agente `default`, y el lanzador se comporta
    /// exactamente como antes de la tarea 6.
    pub(super) registry: Option<Arc<AgentRegistry>>,
    /// Volcado de salidas recortadas que reciben los hijos; `None` como en el agente padre.
    pub(super) spill: Option<ToolSpill>,
    /// Presupuesto del padre: cada hijo lo recibe y suma su consumo al total de la familia.
    pub(super) budget: BudgetConfig,
    /// Lo máximo que `cancel` espera a que el hijo se asiente; `CANCEL_WAIT` salvo en las pruebas.
    pub(super) cancel_wait: std::time::Duration,
    /// Fábrica de clientes de agentes remotos (A8a). Sin ella, un remoto no se ejecuta.
    pub(super) remote_clients: Option<Arc<dyn RemoteClientFactory>>,
    /// Cada cuánto se consulta una tarea remota, y cuánto se la sigue como máximo.
    pub(super) remote_poll: std::time::Duration,
    pub(super) remote_max_follow: std::time::Duration,
    /// Lo máximo que se espera a que la persona conteste a una pregunta de un remoto.
    pub(super) remote_input_wait: std::time::Duration,
    pub(super) profile_provider_resolver: Option<Arc<ProfileProviderResolver>>,
    /// Enrutador de agentes (A12b) y el modo de confirmación con el que convive. Sin él no se elige
    /// agente automáticamente.
    pub(super) router: Option<(Arc<dyn AgentRouter>, ConfirmDelegation)>,
    /// Niveles restantes de la sesión que usa este lanzador (véase `child_levels`). Por defecto, los de
    /// la raíz con el techo por defecto: la delegación de siempre, sin recursión.
    pub(super) levels_left: u32,
    /// Herramientas de subagentes que el constructor retira de los hijos y que solo reciben los que
    /// pueden delegar. `development_workflow` nunca figura aquí: solo la sesión raíz la gobierna.
    pub(super) delegation_tools: Vec<Arc<dyn Tool>>,
    /// Sesión raíz de la familia de presupuesto. `None` en el lanzador de la raíz (su padre es la raíz);
    /// los lanzadores anidados la heredan, de modo que el gasto de los nietos suma a la misma familia.
    pub(super) family_root: Option<SessionId>,
    /// Perfiles cuyo recorte de profundidad ya se avisó, para avisar una sola vez por nombre.
    pub(super) clipped_notified: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl InProcessSubagentLauncher {
    /// Descriptor vigente de un hijo de `parent`, o `TaskNotFound`.
    pub(super) async fn descriptor_of(
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
        config: Arc<RwLock<LoomcodingConfig>>,
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
                    .without("development_workflow")
                    .without("gauntlet"),
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
    pub(super) fn familia(&self, parent: SessionId) -> SessionId {
        self.family_root.unwrap_or(parent)
    }

    /// Lanzador de la sesión `hijo`, que puede delegar: comparte con este el semáforo de activos, el
    /// buzón de preguntas, el almacén, el bus, los trabajos, el catálogo, el registro y el presupuesto, y
    /// ve el árbol entero como una sola familia. Cambian el proveedor, la configuración, el servicio de
    /// permisos y las herramientas, que son los del hijo, y los niveles restantes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn anidado(
        &self,
        raiz: SessionId,
        provider: Arc<dyn Provider>,
        config: LoomcodingConfig,
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
                    .without("development_workflow")
                    .without("gauntlet"),
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
    pub(super) fn with_cancel_wait(mut self, espera: std::time::Duration) -> Self {
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
        self.enrutar(_parent, label, prompt).await
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
        self.arrancar_seguimiento(parent, label, prompt, agent, follow_up_of, parent_cancel)
            .await
    }
    async fn control(
        &self,
        parent: SessionId,
        child: SessionId,
        action: &str,
        message: Option<String>,
    ) -> Result<()> {
        self.controlar(parent, child, action, message).await
    }
    async fn cancel(&self, parent: SessionId, child: SessionId) -> Result<SubagentDescriptor> {
        self.cancelar(parent, child).await
    }
    async fn fail_lost(&self, parent: SessionId, child: SessionId, reason: String) -> Result<()> {
        self.dar_por_perdido(parent, child, reason).await
    }
    async fn list(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>> {
        self.store.list_subagents(parent).await
    }

    async fn deliver_notices(&self, parent: SessionId) -> Result<Vec<crate::message::Message>> {
        self.avisos_de_resultado(parent).await
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
        self.avisos_de_entrada(parent).await
    }
}
