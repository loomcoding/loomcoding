//! Construcción y apertura de agentes: entrada, ajustes, capacidades del runtime y perfil de par A2A.

use super::*;

pub(super) struct AgentEntry {
    pub(super) id: String,
    pub(super) card: loomcoding_core::card::AgentCard,
    pub(super) internal: bool,
    pub(super) cwd: PathBuf,
    pub(super) project_key: String,
    pub(super) model: StdRwLock<String>,
    pub(super) settings: StdRwLock<loomcoding_tui::app::ProviderSettings>,
    pub(super) api_key_configured: AtomicBool,
    pub(super) config: StdRwLock<loomcoding_core::config::LoomcodingConfig>,
    pub(super) ui: Arc<AgentUi>,
    pub(super) commands: mpsc::Sender<AgentCommand>,
    pub(super) store: Arc<loomcoding_core::session::sqlite::SqliteStore>,
    pub(super) config_path: PathBuf,
    pub(super) project_env: HashMap<String, String>,
    pub(super) a2a_consumes: bool,
    pub(super) refresher: Option<mpsc::UnboundedSender<()>>,
    pub(super) _watcher: StdMutex<Option<loomcoding_core::registry::watch::WatcherHandle>>,
    pub(super) registry: Arc<loomcoding_core::registry::AgentRegistry>,
    pub(super) runtime_capabilities: StdRwLock<loomcoding_tui::app::RuntimeCapabilities>,
    pub(super) current_session: AtomicI64,
    pub(super) connected: AtomicBool,
    pub(super) working: AtomicBool,
    pub(super) active_cancel: StdMutex<Option<CancellationToken>>,
    pub(super) status: StdRwLock<String>,
    pub(super) activity: StdRwLock<Option<String>>,
    pub(super) jobs: StdMutex<HashSet<JobId>>,
    /// Salud de los subagentes de este agente, para `runtime_snapshot`.
    pub(super) supervisor: Option<loomcoding_core::subagent::supervisor_driver::SupervisorHandle>,
}

pub(super) struct AgentSetup {
    pub(super) agent: Arc<loomcoding_core::agent::AgentLoop>,
    pub(super) cwd: PathBuf,
    pub(super) project: String,
    pub(super) config_path: PathBuf,
    pub(super) cfg: loomcoding_core::config::LoomcodingConfig,
    pub(super) api_key_configured: bool,
    pub(super) project_env: HashMap<String, String>,
    pub(super) a2a_consumes: bool,
    pub(super) refresher: Option<mpsc::UnboundedSender<()>>,
    pub(super) watcher: loomcoding_core::registry::watch::WatcherHandle,
    pub(super) registry: Arc<loomcoding_core::registry::AgentRegistry>,
    pub(super) runtime_capabilities: loomcoding_tui::app::RuntimeCapabilities,
    pub(super) bus_rx: tokio::sync::broadcast::Receiver<LiveEvent>,
    pub(super) supervisor: loomcoding_core::subagent::supervisor_driver::SupervisorHandle,
}

pub(super) fn executable_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let candidates = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_owned(),
        ]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(&path).any(|directory| {
        candidates
            .iter()
            .any(|candidate| directory.join(candidate).is_file())
    })
}

pub(super) fn runtime_capabilities(
    registry: &loomcoding_core::registry::RegistrySnapshot,
    mcp_config: loomcoding_tui::app::McpConfigState,
    mcp_servers: Vec<loomcoding_tui::app::McpServerCapability>,
    rust_analyzer_available: bool,
) -> loomcoding_tui::app::RuntimeCapabilities {
    use loomcoding_core::registry::{Approval, Execution};
    let mut announced_skills = Vec::new();
    let mut declared_skill_scopes = Vec::new();
    for agent in registry.entries().into_iter().take(128) {
        let available = matches!(
            registry.approval(&agent.name),
            Some(Approval::NotRequired | Approval::Approved)
        );
        for skill in &agent.card.skills {
            if announced_skills.len() < 64 {
                announced_skills.push(loomcoding_tui::app::SkillCapability {
                    agent: agent.name.clone(),
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    available,
                });
            }
        }
        if let Execution::Local(spec) = &agent.execution {
            for skill in &spec.scope.skills {
                if declared_skill_scopes.len() < 64 {
                    declared_skill_scopes.push((agent.name.clone(), skill.clone()));
                }
            }
        }
    }
    let announced_skill_total = registry
        .entries()
        .iter()
        .map(|agent| agent.card.skills.len())
        .sum();
    let available_skill_total = registry
        .entries()
        .iter()
        .filter(|agent| {
            matches!(
                registry.approval(&agent.name),
                Some(Approval::NotRequired | Approval::Approved)
            )
        })
        .map(|agent| agent.card.skills.len())
        .sum();
    let declared_skill_scope_total = registry
        .entries()
        .iter()
        .filter_map(|agent| match &agent.execution {
            Execution::Local(spec) => Some(spec.scope.skills.len()),
            Execution::Remote(_) => None,
        })
        .sum();
    loomcoding_tui::app::RuntimeCapabilities {
        mcp_config,
        mcp_servers,
        lsp_registered: true,
        rust_analyzer_available,
        announced_skills,
        announced_skill_total,
        available_skill_total,
        declared_skill_scopes,
        declared_skill_scope_total,
        local_skill_loader_available: true,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_agent(
    manager: &Arc<AgentManager>,
    cwd: PathBuf,
    ui: Arc<AgentUi>,
    store: Arc<loomcoding_core::session::sqlite::SqliteStore>,
    data_dir: &std::path::Path,
    yolo: bool,
    interactive: bool,
) -> anyhow::Result<AgentSetup> {
    let cwd = std::fs::canonicalize(cwd)?;
    if !cwd.is_dir() {
        anyhow::bail!("el directorio del proyecto no existe");
    }
    let config_path = cwd.join("loomcoding.toml");
    let project_env = crate::load_dotenv(&cwd);
    let cfg = loomcoding_core::config::LoomcodingConfig::load_or_create(&config_path)?;
    let agents_cfg = loomcoding_core::config::AgentsConfig::load(&config_path)?;
    let budget = loomcoding_core::budget::BudgetConfig::load(&config_path)?;
    let compaction = loomcoding_core::agent::compact::CompactionConfig::load(&config_path)?;
    let a2a = loomcoding_a2a::config::A2aConfig::load(&config_path)?;
    let prompt_cache =
        loomcoding_core::config::ProviderCacheConfig::load(&config_path)?.prompt_cache;
    let (provider, api_key_configured) =
        crate::build_provider_state(&cfg, &project_env, None, prompt_cache)?;
    let project = format!("{}#loomcoding-agent", cwd.display());
    let bus = loomcoding_core::event::EventBus::new(1024);
    let bus_rx = bus.subscribe();
    let home = crate::home::user_home();
    let runtime_peers = a2a
        .consumes()
        .then(|| crate::runtime_peer_dir(data_dir, &manager.runtime_generation, &cwd));
    let launch =
        crate::arrancar_agentes_con_runtime(home.as_deref(), &cwd, &agents_cfg, runtime_peers);
    let registry = Arc::new(launch.registro);
    let watcher = registry.spawn_watcher(launch.directorios, bus.clone());
    let remote_clients = crate::arrancar_remotos(&a2a, &registry, &bus)?;
    let refresher = crate::arrancar_refresco_manual(&a2a, &registry, &bus)?;
    let permission = if !interactive {
        if yolo {
            Arc::new(loomcoding_core::permission::PermissionService::allow_all())
        } else {
            Arc::new(
                loomcoding_core::permission::PermissionService::with_responder(Box::new(
                    loomcoding_core::permission::DenyAll,
                )),
            )
        }
    } else if yolo {
        Arc::new(loomcoding_core::permission::PermissionService::allow_all())
    } else {
        Arc::new(
            loomcoding_core::permission::PermissionService::with_responder(Box::new(
                SharedAgentUi(ui),
            )),
        )
    };

    let catalog = Arc::new(std::sync::RwLock::new(
        loomcoding_core::catalog::Catalog::default(),
    ));
    let (catalog_tx, _catalog_rx) = mpsc::unbounded_channel();
    tokio::spawn(crate::load_catalog(catalog_tx, catalog.clone()));
    let project_tool_env = crate::shell_env(&project_env);
    let tools = crate::build_project_tools(project_tool_env);
    let mcp_config_path = data_dir.join("mcp.toml");
    let (mcp_config, mcp_servers, mcp_tools) =
        match loomcoding_mcp::McpConfig::load(&mcp_config_path) {
            Ok(config) => {
                let mcp = loomcoding_mcp::McpRuntime::start(config, &cwd).await;
                for issue in mcp.issues() {
                    bus.publish(LiveEvent::RuntimeNotice {
                        message: format!(
                            "MCP server `{}` unavailable: {}",
                            issue.server_id, issue.reason
                        ),
                    });
                }
                let schema_tokens =
                    loomcoding_core::tools::mcp_schema_tokens_by_server(&mcp.tools());
                let statuses = mcp
                    .server_statuses()
                    .iter()
                    .map(|status| loomcoding_tui::app::McpServerCapability {
                        id: status.server_id.clone(),
                        initialized: status.initialized,
                        tool_count: status.tool_count,
                        schema_tokens: schema_tokens.get(&status.server_id).copied().unwrap_or(0),
                        issue: status.issue.clone(),
                    })
                    .collect();
                (
                    loomcoding_tui::app::McpConfigState::Loaded,
                    statuses,
                    mcp.tools(),
                )
            }
            Err(_) => {
                bus.publish(LiveEvent::RuntimeNotice {
                message:
                    "MCP configuration rejected; no MCP tools loaded. Check ~/.loomcoding/mcp.toml"
                        .into(),
            });
                (
                    loomcoding_tui::app::McpConfigState::Invalid,
                    Vec::new(),
                    Vec::new(),
                )
            }
        };
    let mcp_config =
        if mcp_config == loomcoding_tui::app::McpConfigState::Loaded && !mcp_config_path.exists() {
            loomcoding_tui::app::McpConfigState::NotConfigured
        } else {
            mcp_config
        };
    let runtime_capabilities = runtime_capabilities(
        &registry.snapshot(),
        mcp_config,
        mcp_servers,
        executable_on_path("rust-analyzer"),
    );
    let router = crate::enrutador_de(&agents_cfg, provider.clone(), &cfg.model);
    let profile_env = project_env.clone();
    let profile_config = cfg.clone();
    let profile_provider_resolver = Arc::new(
        move |profile_provider: &loomcoding_core::config::ProviderConfig| {
            let mut config = profile_config.clone();
            config.provider = profile_provider.clone();
            crate::build_provider(&config, &profile_env, prompt_cache)
                .map_err(|error| CoreError::Provider(error.to_string()))
        },
    );
    let agent = loomcoding_core::agent::AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        bus.clone(),
        permission,
        cfg.clone(),
        cwd.clone(),
    )
    .with_catalog(catalog)
    .with_subagent_tools(mcp_tools)
    .with_profile_provider_resolver(profile_provider_resolver)
    .with_agent_registry(registry.clone())
    .with_delegation_confirm(agents_cfg.confirm_delegation)
    .with_delegation_mode(agents_cfg.delegation)
    .with_depth_ceiling(agents_cfg.depth_ceiling())
    .with_max_active_subagents(agents_cfg.active_subagents())
    .with_spill(loomcoding_core::tools::spill::ToolSpill::new(
        data_dir.join("tool-output"),
    ))
    .with_budget(budget)
    .with_compaction(compaction);
    let agent = match remote_clients {
        Some(factory) => agent.with_remote_clients(factory),
        None => agent,
    };
    let agent = match router {
        Some(router) => agent.with_router(router),
        None => agent,
    };

    let agent = Arc::new(agent);
    // Vigila a los subagentes con los umbrales y la acción del `[agents]` de este proyecto. El
    // manejador es débil: la tarea del supervisor no debe mantener vivo al agente.
    let supervisor = loomcoding_core::subagent::supervisor_driver::spawn_supervisor(
        bus.clone(),
        agents_cfg.stall_threshold_ms(),
        agents_cfg.lost_threshold_ms(),
        loomcoding_core::subagent::supervisor_driver::DEFAULT_TICK,
        match agents_cfg.lost_action {
            loomcoding_core::config::LostAction::Notify => {
                loomcoding_core::subagent::supervisor::LostPolicy::MarkAndNotify
            }
            loomcoding_core::config::LostAction::Fail => {
                loomcoding_core::subagent::supervisor::LostPolicy::FailAndStop
            }
        },
        Some(Arc::new(Arc::downgrade(&agent))),
        Some(store.clone()),
    );
    Ok(AgentSetup {
        agent,
        cwd,
        project,
        config_path,
        cfg,
        api_key_configured,
        project_env,
        a2a_consumes: a2a.consumes(),
        refresher,
        watcher,
        registry,
        runtime_capabilities,
        supervisor,
        bus_rx,
    })
}

pub(super) fn generate_agent_id() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut id = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(id)
}

pub(super) fn project_label(cwd: &std::path::Path) -> String {
    let label = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace");
    loomcoding_core::tools::limit_bytes(label, 64).into_owned()
}

pub(super) fn runtime_peer_name(cwd: &std::path::Path, duplicate: bool, id: &str) -> String {
    let folder = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let mut name = String::new();
    let mut separator = false;
    for byte in folder.bytes() {
        if byte.is_ascii_alphanumeric() {
            if separator && !name.is_empty() {
                name.push('-');
            }
            separator = false;
            if name.len() < 48 {
                name.push((byte as char).to_ascii_lowercase());
            }
        } else {
            separator = true;
        }
    }
    if name.is_empty() {
        name.push_str("project");
    }
    if duplicate {
        name.push('-');
        name.push_str(&id[..8]);
    }
    name
}

pub(super) fn runtime_card_url(interface: &str, tenant: &str) -> String {
    let interface = interface.trim_end_matches('/');
    let base = interface.strip_suffix("/a2a").unwrap_or(interface);
    format!(
        "{}/agents/{tenant}/.well-known/agent-card.json",
        base.trim_end_matches('/')
    )
}

pub(super) fn runtime_peer_profile(
    name: &str,
    description: &str,
    endpoint: &str,
    auth_env: &str,
) -> String {
    let description = serde_json::to_string(description).expect("string serialization cannot fail");
    let endpoint = serde_json::to_string(endpoint).expect("string serialization cannot fail");
    format!(
        "---\nname: {name}\ndescription: {description}\ntransport: a2a-remote\na2a:\n  endpoint: {endpoint}\n  credentials:\n    loomcoding: {auth_env}\n---\n"
    )
}

pub(super) async fn open_agent(
    manager: &Arc<AgentManager>,
    store: Arc<loomcoding_core::session::sqlite::SqliteStore>,
    data_dir: &std::path::Path,
    cwd: PathBuf,
    attach_agent: Option<&str>,
    yolo: bool,
    events: mpsc::Sender<ServerFrame>,
) -> anyhow::Result<Arc<AgentEntry>> {
    if let Some(id) = attach_agent {
        if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!("identificador de agente local no válido");
        }
        let entry = manager
            .get(id)
            .await
            .ok_or_else(|| anyhow::anyhow!("agente local no encontrado"))?;
        let cwd = std::fs::canonicalize(cwd)?;
        if cwd != entry.cwd {
            anyhow::bail!("el agente solo se puede adjuntar desde su directorio de proyecto")
        }
        if entry.connected.swap(true, Ordering::AcqRel) {
            anyhow::bail!("el agente ya tiene una TUI conectada")
        }
        entry.ui.replace_client(events).await;
        manager.publish_card(&entry);
        return Ok(entry);
    }

    if manager.agents.read().await.len() >= 32 {
        anyhow::bail!("el servidor ya aloja el máximo de 32 agentes locales");
    }

    let id = generate_agent_id()?;
    let ui = Arc::new(AgentUi::new(events));
    let setup = build_agent(
        manager,
        cwd,
        ui.clone(),
        store.clone(),
        data_dir,
        yolo,
        true,
    )
    .await?;
    let (commands, receiver) = mpsc::channel(32);
    let initial_session = store.create_session(&setup.project).await?;
    let project = project_label(&setup.cwd);
    let card = loomcoding_core::card::AgentCard::local(
        format!("{}-{}", project, &id[..8]),
        format!("Loomcoding runtime agent for project {project}."),
        env!("CARGO_PKG_VERSION"),
    );
    let entry = Arc::new(AgentEntry {
        id: id.clone(),
        card,
        internal: true,
        cwd: setup.cwd.clone(),
        project_key: setup.project.clone(),
        model: StdRwLock::new(setup.cfg.model.clone()),
        settings: StdRwLock::new((&setup.cfg).into()),
        api_key_configured: AtomicBool::new(setup.api_key_configured),
        config: StdRwLock::new(setup.cfg.clone()),
        ui,
        commands,
        store,
        config_path: setup.config_path.clone(),
        project_env: setup.project_env.clone(),
        a2a_consumes: setup.a2a_consumes,
        refresher: setup.refresher.clone(),
        _watcher: StdMutex::new(Some(setup.watcher)),
        registry: setup.registry,
        runtime_capabilities: StdRwLock::new(setup.runtime_capabilities),
        current_session: AtomicI64::new(initial_session.0),
        connected: AtomicBool::new(true),
        working: AtomicBool::new(false),
        active_cancel: StdMutex::new(None),
        status: StdRwLock::new("idle".into()),
        activity: StdRwLock::new(None),
        jobs: StdMutex::new(HashSet::new()),
        supervisor: Some(setup.supervisor),
    });
    manager
        .agents
        .write()
        .await
        .insert(id.clone(), entry.clone());
    manager.publish_card(&entry);
    if let Err(error) = manager.refresh_peer_profiles().await {
        entry
            .ui
            .send(ServerEvent::Error {
                message: format!("no se pudieron actualizar los peers A2A: {error}"),
            })
            .await;
    }
    tokio::spawn(run_agent_worker(
        entry.clone(),
        setup.agent,
        receiver,
        initial_session,
    ));
    tokio::spawn(forward_agent_events(
        manager.clone(),
        entry.clone(),
        setup.bus_rx,
    ));
    Ok(entry)
}
