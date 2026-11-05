use super::*;
use loomcoding_core::agent::TurnOutcome;
use loomcoding_core::message::{FinishReason, Usage};
use loomcoding_core::provider::{Capabilities, ProviderStream};

async fn test_entry(
    cwd: &std::path::Path,
    store: Arc<loomcoding_core::session::sqlite::SqliteStore>,
) -> Arc<AgentEntry> {
    let cwd = std::fs::canonicalize(cwd).unwrap();
    let id = "a".repeat(32);
    let project = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("test-project");
    let project_key = format!("{}#loomcoding-agent:{id}", cwd.display());
    let config = loomcoding_core::config::LoomcodingConfig {
        provider: loomcoding_core::config::ProviderConfig {
            kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
            base_url: "http://127.0.0.1:8000/v1".into(),
            api_key_env: "KEY".into(),
        },
        model: project.to_string(),
        max_steps: 1,
        effort: None,
    };
    let registry = Arc::new(loomcoding_core::registry::AgentRegistry::new(Vec::new()));
    let watcher = registry.spawn_watcher(Vec::new(), loomcoding_core::event::EventBus::new(8));
    let session = store.create_session(&project_key).await.unwrap();
    Arc::new(AgentEntry {
        id: id.clone(),
        card: loomcoding_core::card::AgentCard::local("test-agent", "test", "1"),
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
        config_path: cwd.join("loomcoding.toml"),
        project_env: crate::load_dotenv(&cwd),
        a2a_consumes: false,
        refresher: None,
        _watcher: StdMutex::new(Some(watcher)),
        registry: registry.clone(),
        runtime_capabilities: StdRwLock::new(loomcoding_tui::app::RuntimeCapabilities::default()),
        current_session: AtomicI64::new(session.0),
        connected: AtomicBool::new(true),
        working: AtomicBool::new(false),
        active_cancel: StdMutex::new(None),
        status: StdRwLock::new("idle".into()),
        activity: StdRwLock::new(None),
        jobs: StdMutex::new(HashSet::new()),
        supervisor: None,
    })
}

#[tokio::test]
async fn runtime_loads_its_empty_session_and_rejects_another_agents_session() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let store = Arc::new(
        loomcoding_core::session::sqlite::SqliteStore::open(
            directory.path().join("sessions.db").to_str().unwrap(),
        )
        .await
        .unwrap(),
    );
    let entry = test_entry(&project, store.clone()).await;
    let (events, mut received) = mpsc::channel(4);
    entry.ui.replace_client(events).await;

    send_loaded_session(
        &entry,
        SessionId(entry.current_session.load(Ordering::Acquire)),
    )
    .await
    .unwrap();
    assert!(received.try_recv().is_ok());

    let foreign = store.create_session("another-agent").await.unwrap();
    assert!(send_loaded_session(&entry, foreign).await.is_err());
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
    ) -> loomcoding_core::error::Result<ProviderStream> {
        match self.0 {
            ProviderBehavior::Success => Ok(Box::pin(futures::stream::iter([
                StreamChunk::TextDelta("supervisor answer".into()),
                StreamChunk::Usage(Usage {
                    input_tokens: 12,
                    output_tokens: 4,
                    reported_cost_usd: None,
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
    Vec<loomcoding_core::session::UsageRecord>,
) {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(
        loomcoding_core::session::sqlite::SqliteStore::open(
            directory.path().join("supervisor.db").to_str().unwrap(),
        )
        .await
        .unwrap(),
    );
    let session = store
        .create_session(".loomcoding-supervisor")
        .await
        .unwrap();
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
        loomcoding_core::task::TaskState::Completed,
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
        loomcoding_core::session::sqlite::SqliteStore::open(
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
        loomcoding_core::session::sqlite::SqliteStore::open(
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

#[test]
fn the_snapshot_names_phase_and_health_of_each_subagent() {
    use loomcoding_core::subagent::supervisor::ChildHealth;
    use loomcoding_core::subagent_phase::SubagentPhase;
    let child = |id: i64, phase| ChildHealth {
        parent: SessionId(1),
        child: SessionId(id),
        phase,
        last_activity_ms: 0,
    };
    let text = manager::subagent_lines(vec![
        child(7, SubagentPhase::Streaming),
        child(8, SubagentPhase::Stalled),
        child(9, SubagentPhase::Lost),
    ]);
    assert!(
        text.contains("subagent #7 of #1: streaming, health=ok"),
        "{text}"
    );
    assert!(
        text.contains("subagent #8 of #1: stalled, health=stalled"),
        "{text}"
    );
    assert!(
        text.contains("subagent #9 of #1: lost, health=lost"),
        "{text}"
    );
    assert_eq!(manager::subagent_lines(Vec::new()), "");
}

#[tokio::test]
async fn a_kill_all_command_cancels_the_active_turn_without_waiting_for_the_queue() {
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let store = Arc::new(
        loomcoding_core::session::sqlite::SqliteStore::open(
            directory.path().join("sessions.db").to_str().unwrap(),
        )
        .await
        .unwrap(),
    );
    let entry = test_entry(&project, store).await;
    let cancel = CancellationToken::new();
    *entry.active_cancel.lock().unwrap() = Some(cancel.clone());

    // La cola de comandos de la prueba no tiene lector: el envío falla, pero la cancelación ya se hizo.
    let _ = crate::server::client::handle_client_command(
        &entry,
        ClientCommand::Session(loomcoding_tui::run::SessionCommand::KillAll),
    )
    .await;

    assert!(cancel.is_cancelled());
}

fn outcome(
    final_text: &str,
    stopped: Option<loomcoding_core::agent::stop::StopReason>,
) -> TurnOutcome {
    TurnOutcome {
        session: SessionId(1),
        final_text: final_text.into(),
        steps: 4,
        read_steps: 2,
        stopped,
    }
}

#[test]
fn print_returns_the_final_text_when_there_is_one() {
    let text = daemon::print_text(outcome("hecho", None)).unwrap();
    assert_eq!(text, "hecho");
}

#[test]
fn print_fails_loudly_when_the_turn_ended_without_an_answer() {
    use loomcoding_core::agent::stop::StopReason;
    let error = daemon::print_text(outcome("  ", Some(StopReason::StepBudget)))
        .expect_err("un turno sin respuesta no es un éxito");
    let text = error.to_string();
    assert!(text.contains("step limit"), "{text}");
    assert!(text.contains("4 work steps"), "{text}");
}

#[tokio::test]
async fn the_print_turn_is_cancelled_when_the_client_disconnects() {
    let (client, mut server) = tokio::io::duplex(64);
    let cancel = CancellationToken::new();
    let watcher = cancel.clone();
    tokio::spawn(async move { daemon::cancel_on_disconnect(&mut server, watcher).await });
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), cancel.cancelled())
        .await
        .expect("al cerrarse el cliente el turno debía cancelarse");
}

#[tokio::test]
async fn a_connected_client_does_not_cancel_the_print_turn() {
    let (_client, mut server) = tokio::io::duplex(64);
    let cancel = CancellationToken::new();
    let watcher = cancel.clone();
    tokio::spawn(async move { daemon::cancel_on_disconnect(&mut server, watcher).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !cancel.is_cancelled(),
        "con el cliente conectado no se cancela"
    );
}
