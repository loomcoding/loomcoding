use crate::ipc;
use crate::Cli;
use async_trait::async_trait;
use codeloom_a2a::serve::rpc::{EventStream, HostError, TaskHost, TaskView};
use codeloom_core::error::CoreError;
use codeloom_core::event::LiveEvent;
use codeloom_core::job::{JobId, JobStatus};
use codeloom_core::message::Message;
use codeloom_core::permission::{Decision, PermissionRequest, PermissionResponder};
use codeloom_core::provider::{Provider, ProviderRequest, StreamChunk};
use codeloom_core::question::{Answer, Question};
use codeloom_core::session::{SessionId, SessionStore};
use futures::StreamExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize)]
pub enum ClientFrame {
    Hello {
        version: u16,
        token: String,
    },
    Open {
        cwd: String,
        attach_agent: Option<String>,
        yolo: bool,
    },
    Print {
        cwd: String,
        prompt: String,
        yolo: bool,
    },
    Command(ClientCommand),
    Disconnect,
}

#[derive(Serialize, Deserialize)]
pub enum ClientCommand {
    Prompt(String),
    Wake,
    Compact,
    CancelTurn,
    RefreshAgents,
    Session(codeloom_tui::run::SessionCommand),
    ProviderSettings {
        id: u64,
        settings: codeloom_tui::app::ProviderSettings,
        api_key: Option<String>,
    },
    PermissionReply {
        id: u64,
        decision: codeloom_core::permission::Decision,
    },
    QuestionReply {
        id: u64,
        answers: Option<Vec<codeloom_core::question::Answer>>,
    },
    Disconnect,
}

#[derive(Serialize, Deserialize)]
pub enum ServerFrame {
    HelloAccepted { version: u16 },
    HelloRejected { message: String },
    Event(Box<ServerEvent>),
}

impl ServerFrame {
    fn event(event: ServerEvent) -> Self {
        Self::Event(Box::new(event))
    }
}

#[derive(Serialize, Deserialize)]
pub enum ServerEvent {
    Ready {
        agent_id: String,
        runtime_generation: String,
        cwd: String,
        model: String,
        settings: codeloom_tui::app::ProviderSettings,
        api_key_configured: bool,
    },
    Live(codeloom_core::event::LiveEvent),
    Session(codeloom_tui::run::SessionUpdate),
    SessionListPage {
        items: Vec<codeloom_core::session::SessionInfo>,
        done: bool,
    },
    SessionLoadedPage {
        session: SessionId,
        messages: Vec<codeloom_core::message::Message>,
        usage: Vec<codeloom_core::session::UsageRecord>,
        subagents: Vec<codeloom_core::session::SubagentDescriptor>,
        done: bool,
    },
    Catalog(codeloom_core::catalog::Catalog),
    PermissionRequest {
        id: u64,
        request: codeloom_core::permission::PermissionRequest,
    },
    Questions {
        id: u64,
        questions: Vec<codeloom_core::question::Question>,
    },
    ProviderSettingsResult {
        id: u64,
        result: Result<bool, String>,
    },
    PrintResult {
        result: Result<String, String>,
    },
    Error {
        message: String,
    },
}

enum AgentCommand {
    Prompt(String),
    Wake,
    Compact,
    Session(codeloom_tui::run::SessionCommand),
    ProviderSettings {
        id: u64,
        settings: codeloom_tui::app::ProviderSettings,
        api_key: Option<String>,
    },
    A2aPrompt {
        task: Arc<RuntimeTask>,
        session: SessionId,
        prompt: String,
        timeout: Duration,
        _permit: tokio::sync::OwnedSemaphorePermit,
    },
}

struct RuntimeTask {
    id: String,
    tenant: String,
    context_id: String,
    cancel: CancellationToken,
    current: StdRwLock<TaskView>,
    updates: broadcast::Sender<TaskView>,
}

#[derive(Deserialize)]
struct RuntimeUserConfig {
    supervisor: Option<SupervisorSettings>,
}

#[derive(Deserialize)]
struct SupervisorSettings {
    provider: codeloom_core::config::ProviderConfig,
    model: String,
}

#[derive(Clone)]
struct SupervisorAgent {
    provider: Arc<dyn Provider>,
    model: String,
    timeout: Duration,
    session: SessionId,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
}

type LoadedSession = (
    SessionId,
    Vec<codeloom_core::message::Message>,
    Vec<codeloom_core::session::UsageRecord>,
    Vec<codeloom_core::session::SubagentDescriptor>,
);

const MAX_RUNTIME_TASK_RESULT_BYTES: usize = 32 * 1024;

impl RuntimeTask {
    fn new(id: String, tenant: String, context_id: String) -> Self {
        let current = runtime_task_view(
            &id,
            &context_id,
            codeloom_core::task::TaskState::Submitted,
            None,
        );
        Self {
            id,
            tenant,
            context_id,
            cancel: CancellationToken::new(),
            current: StdRwLock::new(current),
            updates: broadcast::channel(16).0,
        }
    }

    fn update(&self, state: codeloom_core::task::TaskState, result: Option<&str>) -> TaskView {
        let view = runtime_task_view(&self.id, &self.context_id, state, result);
        *self.current.write().unwrap() = view.clone();
        let _ = self.updates.send(view.clone());
        view
    }

    fn view(&self) -> TaskView {
        self.current.read().unwrap().clone()
    }
}

fn runtime_task_view(
    id: &str,
    context_id: &str,
    state: codeloom_core::task::TaskState,
    result: Option<&str>,
) -> TaskView {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let mut task = json!({
        "id": id,
        "contextId": context_id,
        "status": {
            "state": state.proto_name(),
            "timestamp": codeloom_a2a::wire::timestamp_to_wire(now),
        },
    });
    if let Some(result) = result {
        let result = codeloom_core::tools::limit_bytes(result, MAX_RUNTIME_TASK_RESULT_BYTES - 64);
        task["artifacts"] = json!([{
            "artifactId": "final-result",
            "parts": [{"text": result.as_ref()}],
        }]);
    }
    TaskView {
        json: task,
        terminal: state.is_terminal(),
    }
}

async fn ejecutar_supervisor(
    task: Arc<RuntimeTask>,
    supervisor: SupervisorAgent,
    prompt: String,
    snapshot: String,
    fallback: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let input = format!("Request: {prompt}\n\nCurrent runtime snapshot:\n{snapshot}");
    let user_message = Message::user(input.clone());
    let _ = supervisor
        .store
        .append_message(supervisor.session, &user_message)
        .await;
    let request = ProviderRequest {
        model: supervisor.model.clone(),
        system: "You are the CodeLoom runtime supervisor. Answer using only the supplied runtime snapshot. You may explain status and suggest actions, but you cannot execute tools, alter permissions, or approve operations. If the snapshot lacks information, say so.".into(),
        messages: vec![user_message],
        tools: Vec::new(),
        temperature: 0.1,
        max_tokens: 512,
        effort: None,
        programmatic_tool_calling: false,
    };
    let cancel = task.cancel.clone();
    let result = tokio::select! {
        _ = cancel.cancelled() => {
            task.update(codeloom_core::task::TaskState::Canceled, None);
            return;
        }
        result = tokio::time::timeout(supervisor.timeout, async {
            let mut stream = supervisor.provider.stream(request, cancel.clone()).await?;
            let mut text = String::new();
            let mut usage = None;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    StreamChunk::TextDelta(delta) if text.len() < 8 * 1024 => {
                        text.push_str(&delta);
                    }
                    StreamChunk::Usage(value) => usage = Some(value),
                    _ => {}
                }
            }
            Ok::<_, codeloom_core::error::CoreError>((text, usage))
        }) => result.ok().and_then(|result| result.ok()),
    };
    let answer = match result {
        Some((text, usage)) if !text.trim().is_empty() => {
            let text = codeloom_core::tools::limit_bytes(&text, 8 * 1024).into_owned();
            let assistant = Message::assistant(vec![codeloom_core::message::ContentPart::Text {
                text: text.clone(),
            }]);
            let _ = supervisor
                .store
                .append_message(supervisor.session, &assistant)
                .await;
            if let Some(usage) = usage {
                let created_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let _ = supervisor
                    .store
                    .record_usage(
                        supervisor.session,
                        &codeloom_core::session::UsageRecord {
                            session: supervisor.session,
                            model: supervisor.model.clone(),
                            input_tokens: usage.input_tokens.into(),
                            output_tokens: usage.output_tokens.into(),
                            input_price: None,
                            output_price: None,
                            created_at_ms,
                        },
                    )
                    .await;
            }
            text
        }
        _ => {
            let assistant = Message::assistant(vec![codeloom_core::message::ContentPart::Text {
                text: fallback.clone(),
            }]);
            let _ = supervisor
                .store
                .append_message(supervisor.session, &assistant)
                .await;
            fallback
        }
    };
    task.update(codeloom_core::task::TaskState::Completed, Some(&answer));
}

struct AgentUi {
    events: RwLock<mpsc::Sender<ServerFrame>>,
    connected: AtomicBool,
    decisions: Mutex<HashMap<u64, oneshot::Sender<Decision>>>,
    questions: Mutex<HashMap<u64, oneshot::Sender<Option<Vec<Answer>>>>>,
    next_id: AtomicU64,
}

impl AgentUi {
    fn new(events: mpsc::Sender<ServerFrame>) -> Self {
        Self {
            events: RwLock::new(events),
            connected: AtomicBool::new(true),
            decisions: Mutex::new(HashMap::new()),
            questions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    async fn replace_client(&self, events: mpsc::Sender<ServerFrame>) {
        *self.events.write().await = events;
        self.connected.store(true, Ordering::Release);
        for (_, reply) in self.decisions.lock().await.drain() {
            let _ = reply.send(Decision::Deny);
        }
        for (_, reply) in self.questions.lock().await.drain() {
            let _ = reply.send(None);
        }
    }

    async fn send(&self, event: ServerEvent) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        let events = self.events.read().await.clone();
        tokio::time::timeout(
            Duration::from_secs(2),
            events.send(ServerFrame::event(bounded_wire(event))),
        )
        .await
        .is_ok_and(|result| result.is_ok())
    }

    async fn disconnect(&self) {
        self.connected.store(false, Ordering::Release);
        for (_, reply) in self.decisions.lock().await.drain() {
            let _ = reply.send(Decision::Deny);
        }
        for (_, reply) in self.questions.lock().await.drain() {
            let _ = reply.send(None);
        }
    }

    async fn decide(&self, request: &PermissionRequest) -> Decision {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.decisions.lock().await.insert(id, tx);
        let bounded = PermissionRequest {
            tool: codeloom_core::tools::limit_bytes(&request.tool, 128).into_owned(),
            action: codeloom_core::tools::limit_bytes(&request.action, 128).into_owned(),
            detail: codeloom_core::tools::limit_bytes(&request.detail, 16_000).into_owned(),
        };
        if !self
            .send(ServerEvent::PermissionRequest {
                id,
                request: bounded,
            })
            .await
        {
            self.decisions.lock().await.remove(&id);
            return Decision::Deny;
        }
        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(decision)) => decision,
            _ => {
                self.decisions.lock().await.remove(&id);
                Decision::Deny
            }
        }
    }

    async fn ask_user(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.questions.lock().await.insert(id, tx);
        if !self
            .send(ServerEvent::Questions {
                id,
                questions: questions
                    .iter()
                    .take(4)
                    .map(|question| Question {
                        header: codeloom_core::tools::limit_bytes(&question.header, 128)
                            .into_owned(),
                        question: codeloom_core::tools::limit_bytes(&question.question, 4096)
                            .into_owned(),
                        options: question
                            .options
                            .iter()
                            .take(6)
                            .map(|option| codeloom_core::question::QuestionOption {
                                label: codeloom_core::tools::limit_bytes(&option.label, 256)
                                    .into_owned(),
                                description: codeloom_core::tools::limit_bytes(
                                    &option.description,
                                    512,
                                )
                                .into_owned(),
                            })
                            .collect(),
                        multi_select: question.multi_select,
                    })
                    .collect(),
            })
            .await
        {
            self.questions.lock().await.remove(&id);
            return None;
        }
        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(answers)) => answers,
            _ => {
                self.questions.lock().await.remove(&id);
                None
            }
        }
    }

    async fn resolve_decision(&self, id: u64, decision: Decision) {
        if let Some(reply) = self.decisions.lock().await.remove(&id) {
            let _ = reply.send(decision);
        }
    }

    async fn resolve_questions(&self, id: u64, answers: Option<Vec<Answer>>) {
        if let Some(reply) = self.questions.lock().await.remove(&id) {
            let _ = reply.send(answers);
        }
    }
}

struct SharedAgentUi(Arc<AgentUi>);

#[async_trait]
impl PermissionResponder for SharedAgentUi {
    async fn decide(&self, request: &PermissionRequest) -> Decision {
        self.0.decide(request).await
    }

    async fn ask(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        self.0.ask_user(questions).await
    }

    fn requires_user(&self) -> bool {
        true
    }
}

struct AgentEntry {
    id: String,
    card: codeloom_core::card::AgentCard,
    internal: bool,
    cwd: PathBuf,
    project_key: String,
    model: StdRwLock<String>,
    settings: StdRwLock<codeloom_tui::app::ProviderSettings>,
    api_key_configured: AtomicBool,
    config: StdRwLock<codeloom_core::config::CodeLoomConfig>,
    ui: Arc<AgentUi>,
    commands: mpsc::Sender<AgentCommand>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    config_path: PathBuf,
    project_env: HashMap<String, String>,
    a2a_consumes: bool,
    refresher: Option<mpsc::UnboundedSender<()>>,
    _watcher: StdMutex<Option<codeloom_core::registry::watch::WatcherHandle>>,
    current_session: AtomicI64,
    connected: AtomicBool,
    working: AtomicBool,
    active_cancel: StdMutex<Option<CancellationToken>>,
    status: StdRwLock<String>,
    activity: StdRwLock<Option<String>>,
    jobs: StdMutex<HashSet<JobId>>,
}

struct AgentSetup {
    agent: Arc<codeloom_core::agent::AgentLoop>,
    cwd: PathBuf,
    project: String,
    config_path: PathBuf,
    cfg: codeloom_core::config::CodeLoomConfig,
    api_key_configured: bool,
    project_env: HashMap<String, String>,
    a2a_consumes: bool,
    refresher: Option<mpsc::UnboundedSender<()>>,
    watcher: codeloom_core::registry::watch::WatcherHandle,
    bus_rx: tokio::sync::broadcast::Receiver<LiveEvent>,
}

async fn build_agent(
    manager: &Arc<AgentManager>,
    cwd: PathBuf,
    id: &str,
    ui: Arc<AgentUi>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    data_dir: &std::path::Path,
    yolo: bool,
    interactive: bool,
) -> anyhow::Result<AgentSetup> {
    let cwd = std::fs::canonicalize(cwd)?;
    if !cwd.is_dir() {
        anyhow::bail!("el directorio del proyecto no existe");
    }
    let config_path = cwd.join("codeloom.toml");
    let project_env = super::load_dotenv(&cwd);
    let cfg = codeloom_core::config::CodeLoomConfig::load_or_create(&config_path)?;
    let agents_cfg = codeloom_core::config::AgentsConfig::load(&config_path)?;
    let budget = codeloom_core::budget::BudgetConfig::load(&config_path)?;
    let compaction = codeloom_core::agent::compact::CompactionConfig::load(&config_path)?;
    let a2a = codeloom_a2a::config::A2aConfig::load(&config_path)?;
    let (provider, api_key_configured) = super::build_provider_state(&cfg, &project_env, None)?;
    let project = format!("{}#codeloom-agent:{id}", cwd.display());
    let bus = codeloom_core::event::EventBus::new(1024);
    let bus_rx = bus.subscribe();
    let home = dirs::home_dir();
    let runtime_peers = a2a
        .consumes()
        .then(|| super::runtime_peer_dir(data_dir, &manager.runtime_generation, &cwd));
    let launch =
        super::arrancar_agentes_con_runtime(home.as_deref(), &cwd, &agents_cfg, runtime_peers);
    let registry = Arc::new(launch.registro);
    let watcher = registry.spawn_watcher(launch.directorios, bus.clone());
    let remote_clients = super::arrancar_remotos(&a2a, &registry, &bus)?;
    let refresher = super::arrancar_refresco_manual(&a2a, &registry, &bus)?;
    let permission = if !interactive {
        if yolo {
            Arc::new(codeloom_core::permission::PermissionService::allow_all())
        } else {
            Arc::new(
                codeloom_core::permission::PermissionService::with_responder(Box::new(
                    codeloom_core::permission::DenyAll,
                )),
            )
        }
    } else if yolo {
        Arc::new(codeloom_core::permission::PermissionService::allow_all())
    } else {
        Arc::new(
            codeloom_core::permission::PermissionService::with_responder(Box::new(SharedAgentUi(
                ui,
            ))),
        )
    };

    let catalog = Arc::new(std::sync::RwLock::new(
        codeloom_core::catalog::Catalog::default(),
    ));
    let (catalog_tx, _catalog_rx) = mpsc::unbounded_channel();
    tokio::spawn(super::load_catalog(catalog_tx, catalog.clone()));
    let tools = super::build_project_tools(super::shell_env(&project_env));
    let mcp_config_path = data_dir.join("mcp.toml");
    let mcp_tools = match codeloom_mcp::McpConfig::load(&mcp_config_path) {
        Ok(config) => {
            let mcp = codeloom_mcp::McpRuntime::start(config, &cwd).await;
            for issue in mcp.issues() {
                bus.publish(LiveEvent::RuntimeNotice {
                    message: format!(
                        "MCP server `{}` unavailable: {}",
                        issue.server_id, issue.reason
                    ),
                });
            }
            mcp.tools()
        }
        Err(_) => {
            bus.publish(LiveEvent::RuntimeNotice {
                message: "MCP configuration rejected; no MCP tools loaded. Check ~/.codeloom/mcp.toml".into(),
            });
            Vec::new()
        }
    };
    let router = super::enrutador_de(&agents_cfg, provider.clone(), &cfg.model);
    let profile_env = project_env.clone();
    let profile_config = cfg.clone();
    let profile_provider_resolver = Arc::new(
        move |profile_provider: &codeloom_core::config::ProviderConfig| {
            let mut config = profile_config.clone();
            config.provider = profile_provider.clone();
            super::build_provider(&config, &profile_env)
                .map_err(|error| CoreError::Provider(error.to_string()))
        },
    );
    let agent = codeloom_core::agent::AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        bus,
        permission,
        cfg.clone(),
        cwd.clone(),
    )
    .with_catalog(catalog)
    .with_subagent_tools(mcp_tools)
    .with_profile_provider_resolver(profile_provider_resolver)
    .with_agent_registry(registry.clone())
    .with_delegation_confirm(agents_cfg.confirm_delegation)
    .with_spill(codeloom_core::tools::spill::ToolSpill::new(
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

    Ok(AgentSetup {
        agent: Arc::new(agent),
        cwd,
        project,
        config_path,
        cfg,
        api_key_configured,
        project_env,
        a2a_consumes: a2a.consumes(),
        refresher,
        watcher,
        bus_rx,
    })
}

fn generate_agent_id() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut id = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(id)
}

fn project_label(cwd: &std::path::Path) -> String {
    let label = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace");
    codeloom_core::tools::limit_bytes(label, 64).into_owned()
}

fn runtime_peer_name(cwd: &std::path::Path, duplicate: bool, id: &str) -> String {
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

fn runtime_card_url(interface: &str, tenant: &str) -> String {
    let interface = interface.trim_end_matches('/');
    let base = interface.strip_suffix("/a2a").unwrap_or(interface);
    format!(
        "{}/agents/{tenant}/.well-known/agent-card.json",
        base.trim_end_matches('/')
    )
}

fn runtime_peer_profile(name: &str, description: &str, endpoint: &str, auth_env: &str) -> String {
    let description = serde_json::to_string(description).expect("string serialization cannot fail");
    let endpoint = serde_json::to_string(endpoint).expect("string serialization cannot fail");
    format!(
        "---\nname: {name}\ndescription: {description}\ntransport: a2a-remote\na2a:\n  endpoint: {endpoint}\n  credentials:\n    codeloom: {auth_env}\n---\n"
    )
}

async fn open_agent(
    manager: &Arc<AgentManager>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
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
        &id,
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
    let card = codeloom_core::card::AgentCard::local(
        format!("{}-{}", project, &id[..8]),
        format!("CodeLoom runtime agent for project {project}."),
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
        current_session: AtomicI64::new(initial_session.0),
        connected: AtomicBool::new(true),
        working: AtomicBool::new(false),
        active_cancel: StdMutex::new(None),
        status: StdRwLock::new("idle".into()),
        activity: StdRwLock::new(None),
        jobs: StdMutex::new(HashSet::new()),
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

async fn send_session_list(entry: &AgentEntry) {
    match entry.store.list_sessions(&entry.project_key).await {
        Ok(items) => {
            if items.is_empty() {
                entry
                    .ui
                    .send(ServerEvent::SessionListPage { items, done: true })
                    .await;
            } else {
                let count = items.len();
                for (index, page) in items.chunks(20).enumerate() {
                    entry
                        .ui
                        .send(ServerEvent::SessionListPage {
                            items: page.to_vec(),
                            done: (index + 1) * 20 >= count,
                        })
                        .await;
                }
            }
        }
        Err(error) => {
            entry
                .ui
                .send(ServerEvent::Error {
                    message: error.to_string(),
                })
                .await;
        }
    }
}

async fn send_loaded_session(entry: &AgentEntry, id: SessionId) -> anyhow::Result<()> {
    let sessions = entry.store.list_sessions(&entry.project_key).await?;
    if !sessions.iter().any(|session| session.id == id) {
        anyhow::bail!("la sesión no pertenece a este agente");
    }
    let messages = entry.store.messages(id).await?;
    let usage = entry.store.session_usage(id).await?;
    let subagents = entry.store.list_subagents(id).await?;
    let workflow = entry
        .store
        .latest_development_workflow(id)
        .await?
        .map(|w| w.summary());
    let total = messages.len() + usage.len().div_ceil(10) + subagents.len();
    let mut sent = 0usize;
    for message in messages {
        let message = bounded_wire(message);
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: vec![message],
                usage: Vec::new(),
                subagents: Vec::new(),
                done: sent == total,
            })
            .await;
    }
    for page in usage.chunks(10) {
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: page.to_vec(),
                subagents: Vec::new(),
                done: sent == total,
            })
            .await;
    }
    for item in subagents {
        sent += 1;
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: Vec::new(),
                subagents: vec![bounded_wire(item)],
                done: sent == total,
            })
            .await;
    }
    if total == 0 {
        entry
            .ui
            .send(ServerEvent::SessionLoadedPage {
                session: id,
                messages: Vec::new(),
                usage: Vec::new(),
                subagents: Vec::new(),
                done: true,
            })
            .await;
    }
    entry
        .ui
        .send(ServerEvent::Session(
            codeloom_tui::run::SessionUpdate::Workflow(id, workflow),
        ))
        .await;
    Ok(())
}

fn bounded_wire<T: Serialize + DeserializeOwned>(value: T) -> T {
    let Ok(mut json) = serde_json::to_value(&value) else {
        return value;
    };
    for (max_text, max_items) in [(4096, 16), (2048, 8), (1024, 4), (512, 2), (128, 1)] {
        trim_wire_value(&mut json, 16, max_text, max_items);
        if serde_json::to_vec(&json).is_ok_and(|bytes| bytes.len() < ipc::MAX_FRAME_BYTES - 1024) {
            return serde_json::from_value(json).unwrap_or(value);
        }
    }
    value
}

fn trim_wire_value(value: &mut serde_json::Value, depth: usize, max_text: usize, max_items: usize) {
    if depth == 0 {
        *value = serde_json::Value::Null;
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            if text.len() > max_text {
                *text = codeloom_core::tools::limit_bytes(text, max_text).into_owned();
            }
        }
        serde_json::Value::Array(items) => {
            items.truncate(max_items);
            for item in items {
                trim_wire_value(item, depth - 1, max_text, max_items);
            }
        }
        serde_json::Value::Object(items) => {
            let old = std::mem::take(items);
            let mut bounded = serde_json::Map::new();
            for (key, mut item) in old.into_iter().take(max_items) {
                let key = codeloom_core::tools::limit_bytes(&key, 128).into_owned();
                trim_wire_value(&mut item, depth - 1, max_text, max_items);
                bounded.entry(key).or_insert(item);
            }
            *items = bounded;
        }
        _ => {}
    }
}

async fn run_agent_worker(
    entry: Arc<AgentEntry>,
    agent: Arc<codeloom_core::agent::AgentLoop>,
    mut commands: mpsc::Receiver<AgentCommand>,
    mut session: SessionId,
) {
    while let Some(command) = commands.recv().await {
        match command {
            AgentCommand::Prompt(prompt) => {
                if let Err(error) = run_turn(&entry, &agent, session, prompt).await {
                    entry
                        .ui
                        .send(ServerEvent::Live(LiveEvent::Error {
                            message: error.to_string(),
                        }))
                        .await;
                }
            }
            AgentCommand::Wake => {
                let cancel = CancellationToken::new();
                set_active_cancel(&entry, Some(cancel.clone()));
                entry.working.store(true, Ordering::Release);
                let _ = agent.run_woken(session, cancel).await;
                entry.working.store(false, Ordering::Release);
                set_active_cancel(&entry, None);
            }
            AgentCommand::Compact => {
                agent.compact_now(session, CancellationToken::new()).await;
            }
            AgentCommand::Session(command) => match command {
                codeloom_tui::run::SessionCommand::Refresh => send_session_list(&entry).await,
                codeloom_tui::run::SessionCommand::Select(choice) => {
                    let selected = match choice {
                        codeloom_tui::app::SessionChoice::New => {
                            entry.store.create_session(&entry.project_key).await
                        }
                        codeloom_tui::app::SessionChoice::Resume(id) => {
                            match entry.store.list_sessions(&entry.project_key).await {
                                Ok(sessions) if sessions.iter().any(|item| item.id == id) => Ok(id),
                                Ok(_) => {
                                    Err(CoreError::Store("session is outside this agent".into()))
                                }
                                Err(error) => Err(error),
                            }
                        }
                    };
                    match selected {
                        Ok(id) => {
                            session = id;
                            entry.current_session.store(id.0, Ordering::Release);
                            if let Err(error) = send_loaded_session(&entry, id).await {
                                entry
                                    .ui
                                    .send(ServerEvent::Session(
                                        codeloom_tui::run::SessionUpdate::Error(error.to_string()),
                                    ))
                                    .await;
                            }
                        }
                        Err(error) => {
                            entry
                                .ui
                                .send(ServerEvent::Session(
                                    codeloom_tui::run::SessionUpdate::Error(error.to_string()),
                                ))
                                .await;
                        }
                    }
                }
            },
            AgentCommand::ProviderSettings {
                id,
                settings,
                api_key,
            } => {
                let current = entry.config.read().unwrap().clone();
                let result = super::apply_provider_settings(
                    settings,
                    api_key.as_deref(),
                    &current,
                    &entry.config_path,
                    &agent,
                    &entry.project_env,
                );
                if let Ok((config, configured)) = &result {
                    *entry.model.write().unwrap() = config.model.clone();
                    *entry.settings.write().unwrap() = config.into();
                    *entry.config.write().unwrap() = config.clone();
                    entry
                        .api_key_configured
                        .store(*configured, Ordering::Release);
                }
                entry
                    .ui
                    .send(ServerEvent::ProviderSettingsResult {
                        id,
                        result: result
                            .map(|(_, configured)| configured)
                            .map_err(|error| error.to_string()),
                    })
                    .await;
            }
            AgentCommand::A2aPrompt {
                task,
                session,
                prompt,
                timeout,
                _permit,
            } => {
                if task.cancel.is_cancelled() {
                    task.update(codeloom_core::task::TaskState::Canceled, None);
                    continue;
                }
                task.update(codeloom_core::task::TaskState::Working, None);
                let cancel = task.cancel.clone();
                let turn = run_turn_with_cancel(&entry, &agent, session, prompt, cancel.clone());
                tokio::pin!(turn);
                let result = tokio::select! {
                    result = &mut turn => Some(result),
                    _ = tokio::time::sleep(timeout) => {
                        cancel.cancel();
                        set_active_cancel(&entry, None);
                        entry.working.store(false, Ordering::Release);
                        None
                    }
                };
                match result {
                    None => {
                        task.update(
                            codeloom_core::task::TaskState::Failed,
                            Some("agent task timed out"),
                        );
                    }
                    Some(Ok(_)) if task.cancel.is_cancelled() => {
                        task.update(codeloom_core::task::TaskState::Canceled, None);
                    }
                    Some(Ok(answer)) => {
                        task.update(codeloom_core::task::TaskState::Completed, Some(&answer));
                    }
                    Some(Err(_)) if task.cancel.is_cancelled() => {
                        task.update(codeloom_core::task::TaskState::Canceled, None);
                    }
                    Some(Err(_)) => {
                        task.update(
                            codeloom_core::task::TaskState::Failed,
                            Some("agent task failed"),
                        );
                    }
                }
            }
        }
    }
}

fn set_active_cancel(entry: &AgentEntry, token: Option<CancellationToken>) {
    *entry.active_cancel.lock().unwrap() = token;
}

async fn run_turn(
    entry: &AgentEntry,
    agent: &codeloom_core::agent::AgentLoop,
    session: SessionId,
    prompt: String,
) -> codeloom_core::error::Result<String> {
    let cancel = CancellationToken::new();
    run_turn_with_cancel(entry, agent, session, prompt, cancel).await
}

async fn run_turn_with_cancel(
    entry: &AgentEntry,
    agent: &codeloom_core::agent::AgentLoop,
    session: SessionId,
    prompt: String,
    cancel: CancellationToken,
) -> codeloom_core::error::Result<String> {
    set_active_cancel(entry, Some(cancel.clone()));
    entry.working.store(true, Ordering::Release);
    let outcome = agent.run(session, prompt, cancel).await;
    set_active_cancel(entry, None);
    entry.working.store(false, Ordering::Release);
    outcome.map(|outcome| outcome.final_text)
}

async fn forward_agent_events(
    _manager: Arc<AgentManager>,
    entry: Arc<AgentEntry>,
    mut events: tokio::sync::broadcast::Receiver<LiveEvent>,
) {
    while let Ok(event) = events.recv().await {
        match &event {
            LiveEvent::TurnStarted => {
                *entry.status.write().unwrap() = "working".into();
                entry.activity.write().unwrap().take();
            }
            LiveEvent::TurnEnded | LiveEvent::WakeIdle => {
                *entry.status.write().unwrap() = "idle".into();
                entry.activity.write().unwrap().take();
            }
            LiveEvent::ToolStarted { name, .. } => {
                *entry.activity.write().unwrap() =
                    Some(codeloom_core::tools::limit_bytes(name, 64).into_owned());
            }
            LiveEvent::ToolFinished { .. } => {
                entry.activity.write().unwrap().take();
            }
            LiveEvent::JobChanged(job) => {
                let mut jobs = entry.jobs.lock().unwrap();
                if matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Killed
                ) {
                    jobs.remove(&job.id);
                } else {
                    jobs.insert(job.id.clone());
                }
            }
            LiveEvent::Error { .. } => {
                *entry.status.write().unwrap() = "error".into();
            }
            _ => {}
        }
        let bounded = bounded_wire(event);
        entry.ui.send(ServerEvent::Live(bounded)).await;
    }
}

struct AgentManager {
    agents: RwLock<HashMap<String, Arc<AgentEntry>>>,
    a2a_tasks: RwLock<HashMap<String, Arc<RuntimeTask>>>,
    a2a_task_slots: Arc<Semaphore>,
    a2a_cards: Arc<StdRwLock<HashMap<String, codeloom_a2a::serve::card::ServedCard>>>,
    a2a_interface: Option<String>,
    a2a_auth_scheme: Option<codeloom_a2a::serve::config::ServeAuthScheme>,
    a2a_task_timeout: Duration,
    runtime_generation: String,
    data_dir: PathBuf,
    a2a_auth_env: Option<String>,
    peer_profile_lock: Mutex<()>,
    supervisor: Option<SupervisorAgent>,
    connected: AtomicUsize,
    last_disconnected: StdMutex<Instant>,
    keep_alive: AtomicBool,
}

impl AgentManager {
    fn new(
        a2a_cards: Arc<StdRwLock<HashMap<String, codeloom_a2a::serve::card::ServedCard>>>,
        a2a_interface: Option<String>,
        a2a_auth_scheme: Option<codeloom_a2a::serve::config::ServeAuthScheme>,
        runtime_generation: String,
        data_dir: PathBuf,
        a2a_auth_env: Option<String>,
        a2a_max_tasks: usize,
        a2a_task_timeout: Duration,
        supervisor: Option<SupervisorAgent>,
    ) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            a2a_tasks: RwLock::new(HashMap::new()),
            a2a_task_slots: Arc::new(Semaphore::new(a2a_max_tasks)),
            a2a_cards,
            a2a_interface,
            a2a_auth_scheme,
            a2a_task_timeout,
            runtime_generation,
            data_dir,
            a2a_auth_env,
            peer_profile_lock: Mutex::new(()),
            supervisor,
            connected: AtomicUsize::new(0),
            last_disconnected: StdMutex::new(Instant::now()),
            keep_alive: AtomicBool::new(false),
        }
    }

    fn client_connected(&self) {
        self.connected.fetch_add(1, Ordering::AcqRel);
        *self.last_disconnected.lock().unwrap() = Instant::now();
    }

    fn client_disconnected(&self) {
        self.connected.fetch_sub(1, Ordering::AcqRel);
        *self.last_disconnected.lock().unwrap() = Instant::now();
    }

    fn publish_card(&self, entry: &AgentEntry) {
        let (Some(interface), Some(scheme)) =
            (self.a2a_interface.as_deref(), self.a2a_auth_scheme.as_ref())
        else {
            return;
        };
        let card = codeloom_a2a::serve::card::served_card_for_tenant(
            &entry.card,
            interface,
            Some(&entry.id),
            scheme,
        );
        self.a2a_cards
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(entry.id.clone(), card);
    }

    async fn refresh_peer_profiles(&self) -> anyhow::Result<()> {
        let (Some(interface), Some(_), Some(auth_env)) = (
            self.a2a_interface.as_deref(),
            self.a2a_auth_scheme.as_ref(),
            self.a2a_auth_env.as_deref(),
        ) else {
            return Ok(());
        };
        let _lock = self.peer_profile_lock.lock().await;
        let agents = self
            .agents
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for recipient in agents.iter().filter(|agent| agent.a2a_consumes) {
            let dir =
                super::runtime_peer_dir(&self.data_dir, &self.runtime_generation, &recipient.cwd);
            std::fs::create_dir_all(&dir)?;
            let peers = agents
                .iter()
                .filter(|peer| peer.cwd != recipient.cwd)
                .collect::<Vec<_>>();
            for peer in &peers {
                let Some(_) = self
                    .a2a_cards
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get(&peer.id)
                else {
                    continue;
                };
                let folder_name = runtime_peer_name(&peer.cwd, false, &peer.id);
                let duplicate = peers.iter().any(|other| {
                    other.id != peer.id
                        && runtime_peer_name(&other.cwd, false, &other.id) == folder_name
                });
                let name = runtime_peer_name(&peer.cwd, duplicate, &peer.id);
                let profile = runtime_peer_profile(
                    &name,
                    &peer.card.description,
                    &runtime_card_url(interface, &peer.id),
                    auth_env,
                );
                let path = dir.join(format!("{name}.md"));
                if std::fs::read_to_string(&path).ok().as_deref() == Some(profile.as_str()) {
                    continue;
                }
                let temporary = dir.join(format!(".{name}.tmp"));
                std::fs::write(&temporary, profile)?;
                std::fs::rename(temporary, path)?;
            }
        }
        for agent in agents.iter().filter(|agent| agent.a2a_consumes) {
            if let Some(refresh) = &agent.refresher {
                let _ = refresh.send(());
            }
        }
        Ok(())
    }

    fn should_stop(&self) -> bool {
        if self.keep_alive.load(Ordering::Acquire) || self.connected.load(Ordering::Acquire) > 0 {
            return false;
        }
        if self
            .agents
            .try_read()
            .map(|agents| {
                agents.values().any(|agent| {
                    agent.working.load(Ordering::Acquire) || !agent.jobs.lock().unwrap().is_empty()
                })
            })
            .unwrap_or(true)
        {
            return false;
        }
        self.last_disconnected.lock().unwrap().elapsed() >= Duration::from_secs(300)
    }

    async fn get(&self, id: &str) -> Option<Arc<AgentEntry>> {
        let agent = self.agents.read().await.get(id).cloned()?;
        debug_assert!(agent.internal);
        debug_assert!(!agent.card.name.is_empty());
        Some(agent)
    }

    async fn task(&self, tenant: &str, id: &str) -> Result<Arc<RuntimeTask>, HostError> {
        let task = self
            .a2a_tasks
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or(HostError::NotFound)?;
        if task.tenant != tenant {
            return Err(HostError::NotFound);
        }
        Ok(task)
    }

    async fn runtime_snapshot(&self) -> String {
        let agents = self.agents.read().await;
        let mut rows = agents
            .values()
            .map(|agent| {
                format!(
                    "{}: {}, connected={}, working={}",
                    agent.card.name,
                    agent.status.read().unwrap(),
                    agent.connected.load(Ordering::Acquire),
                    agent.working.load(Ordering::Acquire)
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        codeloom_core::tools::limit_bytes(&rows.join("\n"), 8 * 1024).into_owned()
    }
}

#[async_trait]
impl TaskHost for AgentManager {
    async fn start(&self, _prompt: String) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn start_for(&self, tenant: Option<&str>, prompt: String) -> Result<TaskView, HostError> {
        let tenant = tenant
            .filter(|tenant| !tenant.is_empty())
            .ok_or(HostError::NotFound)?;
        if tenant == "supervisor" {
            let permit = self
                .a2a_task_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| HostError::Busy)?;
            let snapshot = self.runtime_snapshot().await;
            let task = Arc::new(RuntimeTask::new(
                generate_agent_id().map_err(|error| HostError::Other(error.to_string()))?,
                tenant.to_string(),
                "supervisor".into(),
            ));
            let mut tasks = self.a2a_tasks.write().await;
            if tasks.len() >= 256 {
                tasks.retain(|_, task| !task.view().terminal);
            }
            if tasks.len() >= 256 {
                return Err(HostError::Busy);
            }
            tasks.insert(task.id.clone(), task.clone());
            drop(tasks);
            let fallback = format!(
                "Deterministic runtime snapshot:\n{snapshot}\nModel request: {}\nLLM supervisor unavailable or not configured; no runtime action was taken.",
                codeloom_core::tools::limit_bytes(&prompt, 1024)
            );
            task.update(codeloom_core::task::TaskState::Working, None);
            if let Some(supervisor) = self.supervisor.clone() {
                let task_copy = task.clone();
                let prompt = codeloom_core::tools::limit_bytes(&prompt, 8 * 1024).into_owned();
                tokio::spawn(async move {
                    ejecutar_supervisor(task_copy, supervisor, prompt, snapshot, fallback, permit)
                        .await;
                });
            } else {
                task.update(codeloom_core::task::TaskState::Completed, Some(&fallback));
                drop(permit);
            }
            return Ok(task.view());
        }
        let entry = self.get(tenant).await.ok_or(HostError::NotFound)?;
        if !entry.connected.load(Ordering::Acquire) {
            return Err(HostError::NotFound);
        }
        let permit = self
            .a2a_task_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| HostError::Busy)?;
        let session = entry
            .store
            .create_session(&entry.project_key)
            .await
            .map_err(|error| HostError::Other(error.to_string()))?;
        let task = Arc::new(RuntimeTask::new(
            generate_agent_id().map_err(|error| HostError::Other(error.to_string()))?,
            tenant.to_string(),
            session.0.to_string(),
        ));
        {
            let mut tasks = self.a2a_tasks.write().await;
            if tasks.len() >= 256 {
                tasks.retain(|_, task| !task.view().terminal);
            }
            if tasks.len() >= 256 {
                return Err(HostError::Busy);
            }
            tasks.insert(task.id.clone(), task.clone());
        }
        if entry
            .commands
            .send(AgentCommand::A2aPrompt {
                task: task.clone(),
                session,
                prompt,
                timeout: self.a2a_task_timeout,
                _permit: permit,
            })
            .await
            .is_err()
        {
            self.a2a_tasks.write().await.remove(&task.id);
            return Err(HostError::NotFound);
        }
        Ok(task.view())
    }

    async fn get(&self, _id: &str) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn get_for(&self, tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        Ok(self.task(tenant, id).await?.view())
    }

    async fn cancel(&self, _id: &str) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn cancel_for(&self, tenant: Option<&str>, id: &str) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let current = task.view();
        if current.terminal {
            return Err(HostError::NotCancelable);
        }
        task.cancel.cancel();
        Ok(current)
    }

    async fn wait_terminal(&self, _id: &str, _max: Duration) -> Result<TaskView, HostError> {
        Err(HostError::NotFound)
    }

    async fn wait_terminal_for(
        &self,
        tenant: Option<&str>,
        id: &str,
        max: Duration,
    ) -> Result<TaskView, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let deadline = Instant::now() + max;
        loop {
            let view = task.view();
            if view.terminal || Instant::now() >= deadline {
                return Ok(view);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn subscribe(&self, _id: &str) -> Result<EventStream, HostError> {
        Err(HostError::NotFound)
    }

    async fn subscribe_for(
        &self,
        tenant: Option<&str>,
        id: &str,
    ) -> Result<EventStream, HostError> {
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let task = self.task(tenant, id).await?;
        let initial = Some(task.view());
        let updates = task.updates.subscribe();
        Ok(futures::stream::unfold(
            (task, updates, initial, false),
            |(task, mut updates, initial, done)| async move {
                if done {
                    return None;
                }
                if let Some(view) = initial {
                    let terminal = view.terminal;
                    let event = json!({"task": view.json});
                    return Some((event, (task, updates, None, terminal)));
                }
                loop {
                    match updates.recv().await {
                        Ok(view) => {
                            let event = json!({
                                "statusUpdate": {
                                    "taskId": task.id,
                                    "contextId": task.context_id,
                                    "status": view.json["status"],
                                }
                            });
                            return Some((event, (task, updates, None, view.terminal)));
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                }
            },
        )
        .boxed())
    }

    async fn start_streaming(&self, _prompt: String) -> Result<EventStream, HostError> {
        Err(HostError::NotFound)
    }

    async fn start_streaming_for(
        &self,
        tenant: Option<&str>,
        prompt: String,
    ) -> Result<EventStream, HostError> {
        let view = self.start_for(tenant, prompt).await?;
        let tenant = tenant.ok_or(HostError::NotFound)?;
        let id = view
            .json
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| HostError::Other("runtime task has no id".into()))?;
        self.subscribe_for(Some(tenant), id).await
    }
}

pub async fn connect_or_start() -> anyhow::Result<ipc::Stream> {
    let token = ipc::load_or_create_token()?;
    let mut child_started = false;
    let mut last_error = None;
    for _ in 0..200 {
        if let Ok(mut stream) = ipc::connect().await {
            ipc::send(
                &mut stream,
                &ClientFrame::Hello {
                    version: ipc::PROTOCOL_VERSION,
                    token: token.clone(),
                },
            )
            .await?;
            match ipc::receive::<ServerFrame>(&mut stream).await? {
                ServerFrame::HelloAccepted { version } if version == ipc::PROTOCOL_VERSION => {
                    return Ok(stream)
                }
                ServerFrame::HelloAccepted { version } => {
                    anyhow::bail!("servidor CodeLoom incompatible (protocolo {version})")
                }
                ServerFrame::HelloRejected { message } => anyhow::bail!("{message}"),
                ServerFrame::Event(_) => {
                    anyhow::bail!("respuesta IPC inesperada durante el saludo")
                }
            }
        }
        if !child_started {
            start_server_process()?;
            child_started = true;
        }
        last_error = Some("el servidor local no está disponible".to_string());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    anyhow::bail!(last_error.unwrap_or_else(|| "no se pudo iniciar el servidor CodeLoom".into()))
}

fn start_server_process() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.as_std_mut().pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command.spawn()?;
    Ok(())
}

pub async fn run_server() -> anyhow::Result<()> {
    let _token = ipc::load_or_create_token()?;
    let mut listener = ipc::Listener::bind()?;
    let data_dir = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".codeloom");
    std::fs::create_dir_all(&data_dir)?;
    let store = Arc::new(
        codeloom_core::session::sqlite::SqliteStore::open(
            data_dir.join("sessions.db").to_str().unwrap(),
        )
        .await?,
    );
    let runtime_path = data_dir.join("runtime.toml");
    let runtime_text = if runtime_path.exists() {
        std::fs::read_to_string(&runtime_path)?
    } else {
        String::new()
    };
    let a2a = codeloom_a2a::config::A2aConfig::from_toml(&runtime_text)?;
    let supervisor = toml::from_str::<RuntimeUserConfig>(&runtime_text)
        .ok()
        .and_then(|config| config.supervisor)
        .and_then(|settings| {
            let config = codeloom_core::config::CodeLoomConfig {
                provider: settings.provider,
                model: settings.model,
                max_steps: 1,
                effort: None,
            };
            super::build_provider(&config, &HashMap::new())
                .ok()
                .map(|provider| (provider, config.model))
        });
    let supervisor = if let Some((provider, model)) = supervisor {
        store
            .create_session(".codeloom-supervisor")
            .await
            .ok()
            .map(|session| SupervisorAgent {
                provider,
                model,
                timeout: Duration::from_secs(15),
                session,
                store: store.clone(),
            })
    } else {
        None
    };
    if a2a.serves() {
        if let Some(auth) = &a2a.serve.auth {
            if super::resolve_credential(&auth.env).is_none() {
                super::credentials::get_or_create(&auth.env)?;
            }
        }
    }
    let runtime_a2a = resolve_runtime_a2a(&a2a, &super::resolve_credential)?;
    let (authenticator, interface, auth_scheme) = match runtime_a2a {
        Some((authenticator, interface, scheme)) => {
            (Some(authenticator), Some(interface), Some(scheme))
        }
        None => (None, None, None),
    };
    let cards = Arc::new(StdRwLock::new(HashMap::new()));
    let runtime_generation = generate_agent_id()?;
    let manager = Arc::new(AgentManager::new(
        cards.clone(),
        interface.clone(),
        auth_scheme.clone(),
        runtime_generation,
        data_dir.clone(),
        a2a.serve.auth.as_ref().map(|auth| auth.env.clone()),
        a2a.serve.max_concurrent_tasks,
        Duration::from_secs(a2a.serve.max_task_secs),
        supervisor,
    ));
    if let (Some(auth), Some(interface), Some(scheme)) = (authenticator, interface, auth_scheme) {
        let supervisor = codeloom_core::card::AgentCard::local(
            "CodeLoom Runtime Supervisor",
            "Runtime status and deterministic supervision.",
            env!("CARGO_PKG_VERSION"),
        );
        let card = codeloom_a2a::serve::card::served_card_for_tenant(
            &supervisor,
            &interface,
            Some("supervisor"),
            &scheme,
        );
        cards
            .write()
            .unwrap()
            .insert("supervisor".into(), card.clone());
        let state = Arc::new(codeloom_a2a::serve::rpc::ServeState {
            host: manager.clone(),
            auth,
            card,
            expected_tenant: None,
            agent_cards: cards,
            max_wait: Duration::from_secs(a2a.serve.max_wait_secs),
        });
        codeloom_a2a::serve::serve(state, a2a.serve.listen_addr()?, CancellationToken::new())
            .await?;
        manager.keep_alive.store(true, Ordering::Release);
    }
    let clients = Arc::new(Semaphore::new(32));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let stream = accepted?;
                let permit = clients.clone().try_acquire_owned();
                if let Ok(permit) = permit {
                    tokio::spawn(serve_connection(stream, permit, manager.clone(), store.clone(), data_dir.clone(), _token.clone()));
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if manager.should_stop() { break; }
            }
        }
    }
    drop(store);
    Ok(())
}

fn resolve_runtime_a2a(
    config: &codeloom_a2a::config::A2aConfig,
    env: &dyn Fn(&str) -> Option<String>,
) -> anyhow::Result<
    Option<(
        codeloom_a2a::serve::auth::Authenticator,
        String,
        codeloom_a2a::serve::config::ServeAuthScheme,
    )>,
> {
    if !matches!(
        config.role,
        codeloom_a2a::config::A2aRole::Serve | codeloom_a2a::config::A2aRole::Both
    ) {
        return Ok(None);
    }
    let resolved = config.serve.validate(env)?;
    let scheme = resolved.scheme.clone();
    Ok(Some((
        codeloom_a2a::serve::auth::Authenticator::new(resolved),
        config.serve.interface_url()?,
        scheme,
    )))
}

async fn serve_connection(
    stream: ipc::Stream,
    _permit: tokio::sync::OwnedSemaphorePermit,
    manager: Arc<AgentManager>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    data_dir: PathBuf,
    token: String,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let hello = tokio::time::timeout(
        Duration::from_secs(5),
        ipc::receive::<ClientFrame>(&mut reader),
    )
    .await;
    let Ok(Ok(ClientFrame::Hello {
        version,
        token: supplied,
    })) = hello
    else {
        return;
    };
    if version != ipc::PROTOCOL_VERSION || !constant_time_eq(&token, &supplied) {
        let message = if version != ipc::PROTOCOL_VERSION {
            "protocolo local incompatible"
        } else {
            "autenticación local rechazada"
        };
        let _ = ipc::send(
            &mut writer,
            &ServerFrame::HelloRejected {
                message: message.into(),
            },
        )
        .await;
        return;
    }
    if ipc::send(&mut writer, &ServerFrame::HelloAccepted { version })
        .await
        .is_err()
    {
        return;
    }
    manager.client_connected();
    let result =
        serve_authenticated(&mut reader, &mut writer, manager.clone(), store, &data_dir).await;
    if let Some(agent_id) = result {
        disconnect_agent(&manager, &agent_id).await;
    }
    manager.client_disconnected();
}

async fn disconnect_agent(manager: &AgentManager, id: &str) {
    if let Some(agent) = manager.get(id).await {
        agent.connected.store(false, Ordering::Release);
        agent.ui.disconnect().await;
    }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != 64 || right.len() != 64 {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

async fn serve_authenticated<R, W>(
    reader: &mut R,
    writer: &mut W,
    manager: Arc<AgentManager>,
    store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    data_dir: &std::path::Path,
) -> Option<String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let frame =
        match tokio::time::timeout(Duration::from_secs(5), ipc::receive::<ClientFrame>(reader))
            .await
        {
            Ok(Ok(frame)) => frame,
            _ => return None,
        };
    match frame {
        ClientFrame::Print { cwd, prompt, yolo } => {
            let result = async {
                if prompt.len() > 32_000 {
                    anyhow::bail!("prompt exceeds 32000 bytes");
                }
                let cwd = validate_cwd(cwd)?;
                let id = generate_agent_id()?;
                let ui = Arc::new(AgentUi::new(mpsc::channel(1).0));
                let setup =
                    build_agent(&manager, cwd, &id, ui, store.clone(), data_dir, yolo, false)
                        .await?;
                let session = store.create_session(&setup.project).await?;
                let outcome = setup
                    .agent
                    .run(session, prompt, CancellationToken::new())
                    .await?;
                Ok::<String, anyhow::Error>(outcome.final_text)
            }
            .await;
            let response = result
                .map(|text| codeloom_core::tools::limit_bytes(&text, 32_000).into_owned())
                .map_err(|error| error.to_string());
            let _ = ipc::send(
                writer,
                &ServerFrame::event(ServerEvent::PrintResult { result: response }),
            )
            .await;
            None
        }
        ClientFrame::Open {
            cwd,
            attach_agent,
            yolo,
        } => {
            let event_capacity = mpsc::channel::<ServerFrame>(128);
            let (event_tx, mut event_rx) = event_capacity;
            let entry = match validate_cwd(cwd).map(|cwd| (cwd, attach_agent)) {
                Ok((cwd, attach)) => match open_agent(
                    &manager,
                    store,
                    data_dir,
                    cwd,
                    attach.as_deref(),
                    yolo,
                    event_tx,
                )
                .await
                {
                    Ok(entry) => entry,
                    Err(error) => {
                        let _ = ipc::send(
                            writer,
                            &ServerFrame::event(ServerEvent::Error {
                                message: error.to_string(),
                            }),
                        )
                        .await;
                        return None;
                    }
                },
                Err(error) => {
                    let _ = ipc::send(
                        writer,
                        &ServerFrame::event(ServerEvent::Error {
                            message: error.to_string(),
                        }),
                    )
                    .await;
                    return None;
                }
            };
            let ready = ServerFrame::event(ServerEvent::Ready {
                agent_id: entry.id.clone(),
                runtime_generation: manager.runtime_generation.clone(),
                cwd: entry.cwd.display().to_string(),
                model: entry.model.read().unwrap().clone(),
                settings: entry.settings.read().unwrap().clone(),
                api_key_configured: entry.api_key_configured.load(Ordering::Acquire),
            });
            if ipc::send(writer, &ready).await.is_err() {
                entry.connected.store(false, Ordering::Release);
                entry.ui.disconnect().await;
                return Some(entry.id.clone());
            }
            let session = SessionId(entry.current_session.load(Ordering::Acquire));
            if entry
                .commands
                .send(AgentCommand::Session(
                    codeloom_tui::run::SessionCommand::Refresh,
                ))
                .await
                .is_err()
                || entry
                    .commands
                    .send(AgentCommand::Session(
                        codeloom_tui::run::SessionCommand::Select(
                            codeloom_tui::app::SessionChoice::Resume(session),
                        ),
                    ))
                    .await
                    .is_err()
            {
                return Some(entry.id.clone());
            }
            loop {
                tokio::select! {
                    event = event_rx.recv() => match event {
                        Some(frame) => if ipc::send(writer, &frame).await.is_err() { break; },
                        None => break,
                    },
                    frame = ipc::receive::<ClientFrame>(reader) => match frame {
                        Ok(ClientFrame::Command(command)) => {
                            if handle_client_command(&entry, command).await.is_err() { break; }
                        }
                        Ok(ClientFrame::Disconnect) | Err(_) => break,
                        _ => break,
                    },
                }
            }
            entry.connected.store(false, Ordering::Release);
            entry.ui.disconnect().await;
            Some(entry.id.clone())
        }
        _ => {
            let _ = ipc::send(
                writer,
                &ServerFrame::HelloRejected {
                    message: "se esperaba Open o Print".into(),
                },
            )
            .await;
            None
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use codeloom_core::message::{FinishReason, Usage};
    use codeloom_core::provider::{Capabilities, ProviderStream};

    async fn test_entry(
        cwd: &std::path::Path,
        store: Arc<codeloom_core::session::sqlite::SqliteStore>,
    ) -> Arc<AgentEntry> {
        let cwd = std::fs::canonicalize(cwd).unwrap();
        let id = "a".repeat(32);
        let project = cwd
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("test-project");
        let project_key = format!("{}#codeloom-agent:{id}", cwd.display());
        let config = codeloom_core::config::CodeLoomConfig {
            provider: codeloom_core::config::ProviderConfig {
                kind: codeloom_core::config::ProviderKind::OpenAiCompat,
                base_url: "http://127.0.0.1:8000/v1".into(),
                api_key_env: "KEY".into(),
            },
            model: project.to_string(),
            max_steps: 1,
            effort: None,
        };
        let registry = Arc::new(codeloom_core::registry::AgentRegistry::new(Vec::new()));
        let watcher = registry.spawn_watcher(Vec::new(), codeloom_core::event::EventBus::new(8));
        let session = store.create_session(&project_key).await.unwrap();
        Arc::new(AgentEntry {
            id: id.clone(),
            card: codeloom_core::card::AgentCard::local("test-agent", "test", "1"),
            internal: true,
            cwd: cwd.clone(),
            project_key,
            model: StdRwLock::new(config.model.clone()),
            settings: StdRwLock::new((&config).into()),
            api_key_configured: AtomicBool::new(false),
            config: StdRwLock::new(config),
            ui: Arc::new(AgentUi::new(mpsc::channel(1).0)),
            commands: mpsc::channel(1).0,
            store,
            config_path: cwd.join("codeloom.toml"),
            project_env: crate::load_dotenv(&cwd),
            a2a_consumes: false,
            refresher: None,
            _watcher: StdMutex::new(Some(watcher)),
            current_session: AtomicI64::new(session.0),
            connected: AtomicBool::new(true),
            working: AtomicBool::new(false),
            active_cancel: StdMutex::new(None),
            status: StdRwLock::new("idle".into()),
            activity: StdRwLock::new(None),
            jobs: StdMutex::new(HashSet::new()),
        })
    }

    #[derive(Clone, Copy)]
    enum ProviderBehavior {
        Success,
        Failure,
        Hang,
    }

    struct MockProvider(ProviderBehavior);

    #[async_trait]
    impl Provider for MockProvider {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                tool_use: false,
                reasoning: false,
                streaming: true,
            }
        }

        async fn stream(
            &self,
            _request: ProviderRequest,
            _cancel: CancellationToken,
        ) -> codeloom_core::error::Result<ProviderStream> {
            match self.0 {
                ProviderBehavior::Success => Ok(Box::pin(futures::stream::iter([
                    StreamChunk::TextDelta("supervisor answer".into()),
                    StreamChunk::Usage(Usage {
                        input_tokens: 12,
                        output_tokens: 4,
                    }),
                    StreamChunk::Done(FinishReason::Stop),
                ]))),
                ProviderBehavior::Failure => Err(CoreError::Provider("mock failure".into())),
                ProviderBehavior::Hang => Ok(Box::pin(futures::stream::pending::<StreamChunk>())),
            }
        }
    }

    async fn run_supervisor(
        behavior: ProviderBehavior,
        timeout: Duration,
    ) -> (
        TaskView,
        Vec<Message>,
        Vec<codeloom_core::session::UsageRecord>,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            codeloom_core::session::sqlite::SqliteStore::open(
                directory.path().join("supervisor.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        let session = store.create_session(".codeloom-supervisor").await.unwrap();
        let supervisor = SupervisorAgent {
            provider: Arc::new(MockProvider(behavior)),
            model: "mock-model".into(),
            timeout,
            session,
            store: store.clone(),
        };
        let task = Arc::new(RuntimeTask::new(
            "task-1".into(),
            "supervisor".into(),
            "supervisor".into(),
        ));
        let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
        ejecutar_supervisor(
            task.clone(),
            supervisor,
            "status?".into(),
            "agent a: idle".into(),
            "deterministic fallback".into(),
            permit,
        )
        .await;
        (
            task.view(),
            store.messages(session).await.unwrap(),
            store.session_usage(session).await.unwrap(),
        )
    }

    #[test]
    fn runtime_task_view_is_protocol_shaped_and_bounds_results() {
        let result = "x".repeat(40 * 1024);
        let view = runtime_task_view(
            "task-1",
            "context-1",
            codeloom_core::task::TaskState::Completed,
            Some(&result),
        );

        assert_eq!(view.json["id"], "task-1");
        assert_eq!(view.json["contextId"], "context-1");
        assert_eq!(view.json["status"]["state"], "TASK_STATE_COMPLETED");
        assert!(
            view.json["artifacts"][0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .len()
                <= MAX_RUNTIME_TASK_RESULT_BYTES
        );
        assert!(view.terminal);
    }

    #[test]
    fn supervisor_configuration_is_optional_and_independent() {
        assert!(toml::from_str::<RuntimeUserConfig>("")
            .unwrap()
            .supervisor
            .is_none());

        let config = toml::from_str::<RuntimeUserConfig>(
            "[supervisor]\nmodel = 'local-model'\n[supervisor.provider]\nkind = 'openai_compat'\nbase_url = 'http://127.0.0.1:8000/v1'\napi_key_env = 'SUPERVISOR_KEY'\n",
        )
        .unwrap();
        assert_eq!(config.supervisor.unwrap().model, "local-model");
    }

    #[tokio::test]
    async fn supervisor_provider_success_is_saved_in_its_own_session_and_usage() {
        let (task, messages, usage) =
            run_supervisor(ProviderBehavior::Success, Duration::from_secs(1)).await;

        assert_eq!(
            task.json["artifacts"][0]["parts"][0]["text"],
            "supervisor answer"
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].input_tokens, 12);
        assert_eq!(usage[0].output_tokens, 4);
    }

    #[tokio::test]
    async fn supervisor_provider_error_falls_back_without_losing_runtime_status() {
        let (task, messages, usage) =
            run_supervisor(ProviderBehavior::Failure, Duration::from_secs(1)).await;

        assert_eq!(
            task.json["artifacts"][0]["parts"][0]["text"],
            "deterministic fallback"
        );
        assert_eq!(messages.len(), 2);
        assert!(usage.is_empty());
    }

    #[tokio::test]
    async fn supervisor_provider_timeout_falls_back_without_blocking() {
        let (task, messages, usage) =
            run_supervisor(ProviderBehavior::Hang, Duration::from_millis(1)).await;

        assert_eq!(
            task.json["artifacts"][0]["parts"][0]["text"],
            "deterministic fallback"
        );
        assert_eq!(messages.len(), 2);
        assert!(usage.is_empty());
    }

    #[tokio::test]
    async fn disconnected_agent_keeps_work_and_only_reattaches_from_its_project() {
        let directory = tempfile::tempdir().unwrap();
        let first_project = directory.path().join("first");
        let second_project = directory.path().join("second");
        std::fs::create_dir_all(&first_project).unwrap();
        std::fs::create_dir_all(&second_project).unwrap();
        let store = Arc::new(
            codeloom_core::session::sqlite::SqliteStore::open(
                directory.path().join("sessions.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        let manager = Arc::new(AgentManager::new(
            Arc::new(StdRwLock::new(HashMap::new())),
            None,
            None,
            "generation".into(),
            directory.path().to_path_buf(),
            None,
            4,
            Duration::from_secs(10),
            None,
        ));
        let entry = test_entry(&first_project, store.clone()).await;
        let id = entry.id.clone();
        let cancel = CancellationToken::new();
        *entry.active_cancel.lock().unwrap() = Some(cancel.clone());
        manager
            .agents
            .write()
            .await
            .insert(id.clone(), entry.clone());

        let (events, mut received) = mpsc::channel(4);
        entry.ui.replace_client(events).await;
        let ui = entry.ui.clone();
        let permission = tokio::spawn(async move {
            ui.decide(&PermissionRequest {
                tool: "edit".into(),
                action: "write".into(),
                detail: "file".into(),
            })
            .await
        });
        let _pending_event = received.recv().await.unwrap();
        disconnect_agent(&manager, &id).await;

        assert_eq!(permission.await.unwrap(), Decision::Deny);
        assert!(!cancel.is_cancelled());
        assert!(!entry.connected.load(Ordering::Acquire));

        let (events, _receiver) = mpsc::channel(4);
        let wrong_directory = open_agent(
            &manager,
            store.clone(),
            directory.path(),
            second_project,
            Some(&id),
            false,
            events,
        )
        .await;
        assert!(wrong_directory.is_err());
        assert!(!entry.connected.load(Ordering::Acquire));

        let (events, _receiver) = mpsc::channel(4);
        let attached = open_agent(
            &manager,
            store,
            directory.path(),
            first_project,
            Some(&id),
            false,
            events,
        )
        .await
        .unwrap();
        assert!(Arc::ptr_eq(&attached, &entry));
        assert!(entry.connected.load(Ordering::Acquire));
        assert!(!cancel.is_cancelled());
    }

    #[tokio::test]
    async fn two_project_agents_keep_configuration_environment_and_sessions_separate() {
        let directory = tempfile::tempdir().unwrap();
        let first_project = directory.path().join("first");
        let second_project = directory.path().join("second");
        std::fs::create_dir_all(&first_project).unwrap();
        std::fs::create_dir_all(&second_project).unwrap();
        std::fs::write(first_project.join(".env"), "KEY=first-secret\n").unwrap();
        std::fs::write(second_project.join(".env"), "KEY=second-secret\n").unwrap();
        let store = Arc::new(
            codeloom_core::session::sqlite::SqliteStore::open(
                directory.path().join("sessions.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        let first = test_entry(&first_project, store.clone()).await;
        let second = test_entry(&second_project, store.clone()).await;
        store
            .append_message(
                SessionId(first.current_session.load(Ordering::Acquire)),
                &Message::user("first project"),
            )
            .await
            .unwrap();
        store
            .append_message(
                SessionId(second.current_session.load(Ordering::Acquire)),
                &Message::user("second project"),
            )
            .await
            .unwrap();

        assert_eq!(first.model.read().unwrap().as_str(), "first");
        assert_eq!(second.model.read().unwrap().as_str(), "second");
        assert_eq!(
            first.project_env.get("KEY").map(String::as_str),
            Some("first-secret")
        );
        assert_eq!(
            second.project_env.get("KEY").map(String::as_str),
            Some("second-secret")
        );
        assert_eq!(first.config.read().unwrap().provider.api_key_env, "KEY");
        assert_eq!(second.config.read().unwrap().provider.api_key_env, "KEY");
        assert_ne!(first.project_key, second.project_key);
        assert_ne!(first.config_path, second.config_path);
        assert_eq!(
            store.list_sessions(&first.project_key).await.unwrap().len(),
            1
        );
        assert_eq!(
            store
                .list_sessions(&second.project_key)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}

fn validate_cwd(value: String) -> anyhow::Result<PathBuf> {
    if value.is_empty() || value.len() > 32_000 {
        anyhow::bail!("invalid project path");
    }
    let path = std::fs::canonicalize(value)?;
    if !path.is_dir() {
        anyhow::bail!("project path is not a directory");
    }
    Ok(path)
}

async fn handle_client_command(entry: &AgentEntry, command: ClientCommand) -> anyhow::Result<()> {
    match command {
        ClientCommand::Prompt(prompt) => {
            if prompt.is_empty() || prompt.len() > 32_000 {
                anyhow::bail!("prompt must be 1 to 32000 bytes");
            }
            entry.commands.send(AgentCommand::Prompt(prompt)).await?;
        }
        ClientCommand::Wake => entry.commands.send(AgentCommand::Wake).await?,
        ClientCommand::Compact => entry.commands.send(AgentCommand::Compact).await?,
        ClientCommand::CancelTurn => {
            if let Some(cancel) = entry.active_cancel.lock().unwrap().as_ref() {
                cancel.cancel();
            }
        }
        ClientCommand::RefreshAgents => {
            if let Some(refresh) = &entry.refresher {
                let _ = refresh.send(());
            }
        }
        ClientCommand::Session(command) => {
            entry.commands.send(AgentCommand::Session(command)).await?
        }
        ClientCommand::ProviderSettings {
            id,
            settings,
            api_key,
        } => {
            entry
                .commands
                .send(AgentCommand::ProviderSettings {
                    id,
                    settings,
                    api_key,
                })
                .await?
        }
        ClientCommand::PermissionReply { id, decision } => {
            entry.ui.resolve_decision(id, decision).await
        }
        ClientCommand::QuestionReply { id, answers } => {
            entry.ui.resolve_questions(id, answers).await
        }
        ClientCommand::Disconnect => anyhow::bail!("client disconnected"),
    }
    Ok(())
}

pub async fn run_client(cli: Cli) -> anyhow::Result<()> {
    let mut stream = connect_or_start().await?;
    if let Some(prompt) = cli.print {
        ipc::send(
            &mut stream,
            &ClientFrame::Print {
                cwd: client_cwd()?,
                prompt,
                yolo: cli.yolo,
            },
        )
        .await?;
        return match ipc::receive::<ServerFrame>(&mut stream).await? {
            ServerFrame::Event(event) => match *event {
                ServerEvent::PrintResult { result: Ok(text) } => {
                    println!("{text}");
                    Ok(())
                }
                ServerEvent::PrintResult { result: Err(error) } => anyhow::bail!("{error}"),
                ServerEvent::Error { message } => anyhow::bail!("{message}"),
                _ => anyhow::bail!("respuesta inesperada del servidor local"),
            },
            _ => anyhow::bail!("respuesta inesperada del servidor local"),
        };
    }
    ipc::send(
        &mut stream,
        &ClientFrame::Open {
            cwd: client_cwd()?,
            attach_agent: cli.attach,
            yolo: cli.yolo,
        },
    )
    .await?;
    run_client_tui(stream).await
}

fn client_cwd() -> anyhow::Result<String> {
    let herdr = codeloom_herdr::HerdrContext::from_env();
    let cwd = herdr
        .as_ref()
        .and_then(|context| context.workspace_cwd.as_ref())
        .filter(|path| path.is_dir())
        .cloned()
        .unwrap_or(std::env::current_dir()?);
    Ok(cwd.to_string_lossy().into_owned())
}

type PendingSettings = Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<bool, String>>>>>;

async fn run_client_tui(stream: ipc::Stream) -> anyhow::Result<()> {
    use codeloom_tui::run::{ProviderSettingsCommand, SessionCommand, SessionUpdate};
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (frame_tx, mut frame_rx) = mpsc::channel::<ClientFrame>(128);
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            if ipc::send(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    let (prompt_tx, mut prompt_rx) = mpsc::unbounded_channel::<String>();
    let (wake_tx, mut wake_rx) = mpsc::unbounded_channel::<()>();
    let (compact_tx, mut compact_rx) = mpsc::unbounded_channel::<()>();
    let (settings_tx, mut settings_rx) = mpsc::unbounded_channel::<ProviderSettingsCommand>();
    let (session_command_tx, mut session_command_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (session_update_tx, session_update_rx) = mpsc::unbounded_channel::<SessionUpdate>();
    let (perm_tx, perm_rx) = mpsc::channel(8);
    let (ask_tx, ask_rx) = mpsc::channel(4);
    let (catalog_tx, catalog_rx) = mpsc::unbounded_channel();
    let catalog = Arc::new(std::sync::RwLock::new(
        codeloom_core::catalog::Catalog::default(),
    ));
    tokio::spawn(super::load_catalog(catalog_tx, catalog));
    let pending_settings: PendingSettings = Arc::new(StdMutex::new(HashMap::new()));
    let settings_pending = pending_settings.clone();
    let frame_commands = frame_tx.clone();
    tokio::spawn(async move {
        let mut next_settings_id = 1u64;
        loop {
            tokio::select! {
                value = prompt_rx.recv() => match value {
                    Some(prompt) => if frame_commands.send(ClientFrame::Command(ClientCommand::Prompt(prompt))).await.is_err() { break; },
                    None => break,
                },
                value = wake_rx.recv() => match value {
                    Some(()) => if frame_commands.send(ClientFrame::Command(ClientCommand::Wake)).await.is_err() { break; },
                    None => break,
                },
                value = compact_rx.recv() => match value {
                    Some(()) => if frame_commands.send(ClientFrame::Command(ClientCommand::Compact)).await.is_err() { break; },
                    None => break,
                },
                value = session_command_rx.recv() => match value {
                    Some(command) => if frame_commands.send(ClientFrame::Command(ClientCommand::Session(command))).await.is_err() { break; },
                    None => break,
                },
                value = settings_rx.recv() => match value {
                    Some(command) => {
                        let id = next_settings_id;
                        next_settings_id = next_settings_id.wrapping_add(1).max(1);
                        settings_pending.lock().unwrap().insert(id, command.reply);
                        if frame_commands.send(ClientFrame::Command(ClientCommand::ProviderSettings { id, settings: command.settings, api_key: command.api_key })).await.is_err() { break; }
                    }
                    None => break,
                },
            }
        }
    });

    let (ready_tx, ready_rx) = oneshot::channel();
    let reader_frames = frame_tx.clone();
    let active_turn: codeloom_tui::run::ActiveTurn = Arc::new(std::sync::Mutex::new(None));
    let reader_active = active_turn.clone();
    let reader_pending_settings = pending_settings.clone();
    let bus = codeloom_core::event::EventBus::new(1024);
    let bus_rx = bus.subscribe();
    let reader_bus = bus.clone();
    let reader_task = tokio::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        let mut session_items = Vec::new();
        let mut loaded: Option<LoadedSession> = None;
        let mut turn_end: Option<oneshot::Sender<()>> = None;
        while let Ok(frame) = ipc::receive::<ServerFrame>(&mut reader).await {
            let ServerFrame::Event(event) = frame else {
                continue;
            };
            match *event {
                ServerEvent::Ready {
                    agent_id,
                    runtime_generation,
                    cwd,
                    model,
                    settings,
                    api_key_configured,
                } => {
                    if let Some(reply) = ready_tx.take() {
                        let _ = reply.send(Ok((
                            agent_id,
                            runtime_generation,
                            cwd,
                            model,
                            settings,
                            api_key_configured,
                        )));
                    }
                }
                ServerEvent::Error { message } if ready_tx.is_some() => {
                    if let Some(reply) = ready_tx.take() {
                        let _ = reply.send(Err(message));
                    }
                    break;
                }
                ServerEvent::Error { message } => {
                    let _ = session_update_tx.send(SessionUpdate::Error(message));
                }
                ServerEvent::Live(event) => {
                    match &event {
                        LiveEvent::TurnStarted => {
                            if let Some(end) = turn_end.take() {
                                let _ = end.send(());
                            }
                            let cancel = CancellationToken::new();
                            *reader_active.lock().unwrap() = Some(cancel.clone());
                            let (end_tx, end_rx) = oneshot::channel();
                            let frame_tx = reader_frames.clone();
                            tokio::spawn(async move {
                                tokio::select! {
                                    _ = cancel.cancelled() => { let _ = frame_tx.send(ClientFrame::Command(ClientCommand::CancelTurn)).await; }
                                    _ = end_rx => {}
                                }
                            });
                            turn_end = Some(end_tx);
                        }
                        LiveEvent::TurnEnded => {
                            if let Some(end) = turn_end.take() {
                                let _ = end.send(());
                            }
                            *reader_active.lock().unwrap() = None;
                        }
                        _ => {}
                    }
                    reader_bus.publish(event);
                }
                ServerEvent::Session(update) => {
                    let _ = session_update_tx.send(update);
                }
                ServerEvent::SessionListPage { items, done } => {
                    session_items.extend(items);
                    if done {
                        let _ = session_update_tx
                            .send(SessionUpdate::List(std::mem::take(&mut session_items)));
                    }
                }
                ServerEvent::SessionLoadedPage {
                    session,
                    messages,
                    usage,
                    subagents,
                    done,
                } => {
                    let state =
                        loaded.get_or_insert_with(|| (session, Vec::new(), Vec::new(), Vec::new()));
                    if state.0 != session {
                        loaded = Some((session, Vec::new(), Vec::new(), Vec::new()));
                    }
                    let state = loaded.as_mut().unwrap();
                    state.1.extend(messages);
                    state.2.extend(usage);
                    state.3.extend(subagents);
                    if done {
                        if let Some((id, messages, usage, subagents)) = loaded.take() {
                            let _ = session_update_tx.send(SessionUpdate::Loaded(id, messages));
                            let _ = session_update_tx.send(SessionUpdate::Usage(id, usage));
                            let _ = session_update_tx.send(SessionUpdate::Subagents(id, subagents));
                        }
                    }
                }
                ServerEvent::PermissionRequest { id, request } => {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    if perm_tx.send((request, reply_tx)).await.is_ok() {
                        let frame_tx = reader_frames.clone();
                        tokio::spawn(async move {
                            if let Ok(decision) = reply_rx.await {
                                let _ = frame_tx
                                    .send(ClientFrame::Command(ClientCommand::PermissionReply {
                                        id,
                                        decision,
                                    }))
                                    .await;
                            }
                        });
                    }
                }
                ServerEvent::Questions { id, questions } => {
                    let (reply_tx, reply_rx) = oneshot::channel();
                    if ask_tx.send((questions, reply_tx)).await.is_ok() {
                        let frame_tx = reader_frames.clone();
                        tokio::spawn(async move {
                            if let Ok(answers) = reply_rx.await {
                                let _ = frame_tx
                                    .send(ClientFrame::Command(ClientCommand::QuestionReply {
                                        id,
                                        answers,
                                    }))
                                    .await;
                            }
                        });
                    }
                }
                ServerEvent::ProviderSettingsResult { id, result } => {
                    if let Some(reply) = reader_pending_settings.lock().unwrap().remove(&id) {
                        let _ = reply.send(result);
                    }
                }
                ServerEvent::PrintResult { .. } | ServerEvent::Catalog(_) => {}
            }
        }
    });

    let (agent_id, runtime_generation, cwd, model, settings, api_key_configured) = ready_rx
        .await
        .map_err(|_| anyhow::anyhow!("local server closed before opening the agent"))?
        .map_err(anyhow::Error::msg)?;
    let cwd_path = PathBuf::from(&cwd);
    let config_path = cwd_path.join("codeloom.toml");
    let agents_config = codeloom_core::config::AgentsConfig::load(&config_path)?;
    let a2a_config = codeloom_a2a::config::A2aConfig::load(&config_path)?;
    let data_dir = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".codeloom");
    let runtime_peers = a2a_config
        .consumes()
        .then(|| super::runtime_peer_dir(&data_dir, &runtime_generation, &cwd_path));
    let startup = super::arrancar_agentes_con_runtime(
        dirs::home_dir().as_deref(),
        &cwd_path,
        &agents_config,
        runtime_peers,
    );
    let registry = Arc::new(startup.registro);
    let registry_bus = codeloom_core::event::EventBus::new(32);
    let _watcher = registry.spawn_watcher(startup.directorios, registry_bus.clone());
    let _remote_clients = super::arrancar_remotos(&a2a_config, &registry, &registry_bus)?;
    let local_refresher = super::arrancar_refresco_manual(&a2a_config, &registry, &registry_bus)?;
    if let Some(refresher) = local_refresher.clone() {
        let mut events = registry_bus.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if matches!(event, LiveEvent::AgentRegistryChanged { .. }) {
                    let _ = refresher.send(());
                }
            }
        });
    }
    let mut app = codeloom_tui::app::App::with_context(cwd, model);
    app.use_product_default_view();
    app.set_provider_settings(settings);
    app.settings_api_key_configured = api_key_configured;
    if !api_key_configured {
        app.status = "API key missing · /settings".into();
    }
    app.set_prefs_path(data_dir.join("ui.toml"));
    app.set_agent_registry(registry);
    if matches!(
        a2a_config.role,
        codeloom_a2a::config::A2aRole::Consume | codeloom_a2a::config::A2aRole::Both
    ) {
        let refresh = frame_tx.clone();
        let local_refresh = local_refresher.clone();
        app.set_refresh_agents_hook(Arc::new(move || {
            let _ = refresh.try_send(ClientFrame::Command(ClientCommand::RefreshAgents));
            if let Some(local_refresh) = &local_refresh {
                let _ = local_refresh.send(());
            }
        }));
    }
    codeloom_tui::run::run(
        app,
        prompt_tx,
        wake_tx,
        compact_tx,
        settings_tx,
        bus_rx,
        perm_rx,
        ask_rx,
        active_turn,
        session_command_tx,
        session_update_rx,
        catalog_rx,
    )
    .await?;
    let _ = frame_tx.send(ClientFrame::Disconnect).await;
    drop(frame_tx);
    let _ = reader_task.await;
    let _ = writer_task.await;
    let _ = agent_id;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_peer_card_url_uses_a_direct_standard_card_path() {
        assert_eq!(
            runtime_card_url("http://127.0.0.1:7077/a2a", "tenant"),
            "http://127.0.0.1:7077/agents/tenant/.well-known/agent-card.json"
        );
    }

    #[test]
    fn runtime_peer_name_uses_project_folder_and_disambiguates_duplicates() {
        assert_eq!(
            runtime_peer_name(std::path::Path::new("E:/work/My Animation Lab"), false, "0123456789abcdef"),
            "my-animation-lab"
        );
        assert_eq!(
            runtime_peer_name(std::path::Path::new("E:/work/Animation"), true, "0123456789abcdef"),
            "animation-01234567"
        );
    }

    #[test]
    fn runtime_peer_profile_is_a_remote_profile_without_secrets_or_paths() {
        let profile = runtime_peer_profile(
            "runtime-0123456789ab",
            "Project animation agent",
            "http://127.0.0.1:7077/agents/0123456789abcdef/.well-known/agent-card.json",
            "CODELOOM_A2A_TOKEN",
        );
        let (parsed, _) =
            codeloom_core::registry::parse_profile("runtime-0123456789ab", &profile).unwrap();
        assert_eq!(
            parsed.remote_endpoint.as_deref(),
            Some("http://127.0.0.1:7077/agents/0123456789abcdef/.well-known/agent-card.json")
        );
        assert!(!profile.contains("E:\\workspace"));
        assert!(!profile.contains("secret-value"));
    }

    #[test]
    fn local_token_comparison_requires_a_full_matching_token() {
        let token = "ab".repeat(32);
        assert!(constant_time_eq(&token, &token));
        assert!(!constant_time_eq(&token, &"ac".repeat(32)));
        assert!(!constant_time_eq(&token, "short"));
    }

    #[test]
    fn ipc_events_are_bounded_before_serializing() {
        let event = LiveEvent::Error {
            message: "x".repeat(1_000_000),
        };
        let bounded = bounded_wire(event);
        let encoded = serde_json::to_vec(&bounded).unwrap();
        assert!(encoded.len() < ipc::MAX_FRAME_BYTES);
    }
}
