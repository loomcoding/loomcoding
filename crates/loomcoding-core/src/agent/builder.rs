//! Construcción del bucle del agente y sus ajustes `with_*`.

use super::*;

impl AgentLoop {
    pub fn new(
        provider: Arc<dyn Provider>,
        store: Arc<dyn SessionStore>,
        tools: Arc<ToolRegistry>,
        bus: EventBus,
        permission: Arc<PermissionService>,
        config: LoomcodingConfig,
        cwd: PathBuf,
    ) -> Self {
        let provider = Arc::new(RwLock::new(provider));
        let config = Arc::new(RwLock::new(config));
        let jobs: Arc<dyn crate::job::JobRegistry> =
            Arc::new(crate::job::LocalJobRegistry::default());
        let has_subagents = tools.get("subagent").is_some();
        let mut agent = Self {
            provider,
            store,
            tools,
            subagent_tools: Vec::new(),
            bus,
            permission,
            config,
            changes: crate::agent::changes::ChangeTracker::detect(cwd.clone()),
            jobs,
            subagents: None,
            catalog: Arc::new(RwLock::new(Catalog::default())),
            system_extra: None,
            system_usage_components: Vec::new(),
            registry: None,
            delegation_confirm: ConfirmDelegation::default(),
            max_active_subagents: crate::config::DEFAULT_ACTIVE_SUBAGENTS,
            limits: crate::config::LimitsConfig::default(),
            delegation: None,
            depth_ceiling: crate::config::DEFAULT_DEPTH_CEILING,
            router: None,
            spill: None,
            remote_clients: None,
            profile_provider_resolver: None,
            budget: BudgetConfig::default(),
            budget_family: None,
            compaction: compact::CompactionConfig::default(),
            background: background::BackgroundSlots::default(),
            cwd,
        };
        if has_subagents {
            agent.subagents = Some(agent.subagent_launcher());
        }
        agent
    }

    pub fn with_jobs(mut self, jobs: Arc<dyn crate::job::JobRegistry>) -> Self {
        self.jobs = jobs;
        self
    }

    /// Comparte el catálogo de modelos con el agente y con sus subagentes, que apuntan el
    /// precio vigente de cada respuesta. Sin catálogo, los apuntes se guardan con precio `NULL`.
    pub fn with_catalog(mut self, catalog: Arc<RwLock<Catalog>>) -> Self {
        self.catalog = catalog;
        // El lanzador guarda su propia copia del `Arc`: si ya existía, se reconstruye con el nuevo.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Añade un prompt de sistema propio (el del perfil de un subagente) tras el contexto del
    /// proyecto. `None` deja el sistema tal cual, como antes de existir esta opción.
    pub fn with_system_extra(mut self, texto: Option<String>) -> Self {
        self.system_extra = texto;
        self
    }

    pub fn with_system_usage_components(
        mut self,
        components: Vec<crate::session::UsageComponent>,
    ) -> Self {
        self.system_usage_components = components;
        self
    }

    /// Comparte el registro de agentes con los subagentes que este agente lance. Como
    /// `with_catalog`, reconstruye el lanzador si ya existía.
    pub fn with_agent_registry(mut self, registry: Arc<crate::registry::AgentRegistry>) -> Self {
        self.registry = Some(registry);
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Provides MCP tools to delegated agents; the root agent never receives them implicitly.
    pub fn with_subagent_tools(mut self, tools: Vec<Arc<dyn crate::tools::Tool>>) -> Self {
        self.subagent_tools = tools;
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Resuelve proveedores opcionales de perfiles locales sin cambiar el del agente padre.
    pub fn with_profile_provider_resolver(
        mut self,
        resolver: Arc<ProfileProviderResolver>,
    ) -> Self {
        self.profile_provider_resolver = Some(resolver);
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Activa la elección automática de agente al delegar (`[agents] routing`, A12b).
    pub fn with_router(mut self, router: Arc<dyn crate::registry::router::AgentRouter>) -> Self {
        self.router = Some(router);
        self
    }

    /// Fija cuándo pregunta la persona usuaria antes de delegar (`[agents] confirm_delegation`).
    /// Por defecto, `Doubt`. Solo cambia la regla del prompt; no garantiza el comportamiento.
    pub fn with_delegation_confirm(mut self, confirm: ConfirmDelegation) -> Self {
        self.delegation_confirm = confirm;
        // El lanzador lleva el modo para aprobar cada delegación: si ya existía, se reconstruye.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Sustituye el lanzador de subagentes del agente. Lo usa el lanzador para dar al hijo que puede
    /// delegar uno anidado, que comparte con el árbol el tope de activos y el presupuesto.
    pub(crate) fn with_subagent_launcher(
        mut self,
        launcher: Arc<dyn crate::subagent::SubagentLauncher>,
    ) -> Self {
        self.subagents = Some(launcher);
        self
    }

    /// Fija el techo de profundidad de la delegación (`[agents] max_depth`). Por defecto, 1: los hijos no
    /// delegan.
    pub fn with_depth_ceiling(mut self, ceiling: u32) -> Self {
        self.depth_ceiling = ceiling;
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Fija el mando de delegación (`[agents] delegation`). `Off` retira de verdad las herramientas
    /// de subagentes; los otros dos modos añaden una regla al prompt.
    pub fn with_delegation_mode(mut self, mode: Option<crate::config::DelegationMode>) -> Self {
        self.delegation = mode;
        self
    }

    /// Fija los topes de `[limits]`. Por defecto, `LimitsConfig::default()`.
    pub fn with_limits(mut self, limits: crate::config::LimitsConfig) -> Self {
        self.limits = limits;
        self
    }

    /// Fija cuántos subagentes pueden estar activos a la vez. Por defecto, 8.
    pub fn with_max_active_subagents(mut self, maximo: usize) -> Self {
        self.max_active_subagents = maximo;
        // Igual que `with_spill`: si el lanzador ya existía, se reconstruye con el nuevo tope.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Activa el volcado a disco de las salidas de herramienta que superan el tope. Sin él, el
    /// recorte no lleva ruta.
    pub fn with_spill(mut self, spill: ToolSpill) -> Self {
        self.spill = Some(spill);
        // Igual que `with_catalog`: si el lanzador ya existía, se reconstruye con el volcado.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Fija la fábrica de clientes de agentes remotos (A8a). Sin ella, el lanzador rechaza los
    /// agentes remotos con un error claro. Los subagentes que lance la heredan.
    pub fn with_remote_clients(
        mut self,
        fabrica: Arc<dyn crate::remote_task::RemoteClientFactory>,
    ) -> Self {
        self.remote_clients = Some(fabrica);
        // Igual que `with_spill`: si el lanzador ya existía, se reconstruye con la fábrica.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Fija el presupuesto de gasto de la sesión (`[budget]`). Sin él, o vacío, no hay avisos.
    /// Los subagentes que lance heredan el mismo presupuesto y suman al total de la familia.
    pub fn with_budget(mut self, budget: BudgetConfig) -> Self {
        self.budget = budget;
        // Igual que `with_catalog`: el lanzador guarda su copia y, si ya existía, se reconstruye.
        if self.subagents.is_some() {
            self.subagents = Some(self.subagent_launcher());
        }
        self
    }

    /// Hace que el consumo de este agente cuente en el acumulado de la familia de `root`: es lo que
    /// usa el lanzador para que un hijo y sus hermanos sumen al presupuesto del padre.
    pub fn with_budget_family(mut self, root: SessionId) -> Self {
        self.budget_family = Some(root);
        self
    }

    /// Fija la configuración de la compactación (`[compaction]`). Solo afecta a este agente: el
    /// lanzador de subagentes no lo hereda, así que un hijo usa siempre la vía síncrona.
    pub fn with_compaction(mut self, compaction: compact::CompactionConfig) -> Self {
        self.compaction = compaction;
        self
    }

    pub fn jobs(&self) -> Arc<dyn crate::job::JobRegistry> {
        self.jobs.clone()
    }

    pub fn update_provider(&self, provider: Arc<dyn Provider>, config: LoomcodingConfig) {
        *self.provider.write().unwrap() = provider;
        *self.config.write().unwrap() = config;
    }
}
