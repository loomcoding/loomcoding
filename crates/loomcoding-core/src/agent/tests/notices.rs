use super::*;

/// Lanzador de prueba: entrega cada aviso de pregunta una sola vez, como el real.
struct InputNoticeLauncher {
    notices: std::sync::Mutex<Vec<Message>>,
}

#[async_trait]
impl crate::subagent::SubagentLauncher for InputNoticeLauncher {
    async fn start(
        &self,
        _parent: SessionId,
        _label: String,
        _prompt: String,
        _agent: Option<String>,
        _cancel: CancellationToken,
    ) -> Result<(SessionId, crate::job::JobId)> {
        Err(crate::error::CoreError::Tool("not used".into()))
    }

    async fn control(
        &self,
        _parent: SessionId,
        _child: SessionId,
        _action: &str,
        _message: Option<String>,
    ) -> Result<()> {
        Err(crate::error::CoreError::Tool("not used".into()))
    }

    async fn list(&self, _parent: SessionId) -> Result<Vec<crate::session::SubagentDescriptor>> {
        Ok(Vec::new())
    }

    async fn deliver_notices(&self, _parent: SessionId) -> Result<Vec<Message>> {
        Ok(Vec::new())
    }

    async fn pending_input_notices(&self, _parent: SessionId) -> Result<Vec<Message>> {
        Ok(std::mem::take(&mut *self.notices.lock().unwrap()))
    }
}

/// Construye un agente de prueba con un proveedor guionizado y un bus propio.
fn woken_agent(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    scripts: Vec<Vec<StreamChunk>>,
    bus: EventBus,
) -> AgentLoop {
    AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(scripts.into()),
        }),
        store,
        Arc::new(ToolRegistry::new()),
        bus,
        Arc::new(PermissionService::allow_all()),
        LoomcodingConfig {
            provider: crate::config::ProviderConfig {
                kind: crate::config::ProviderKind::OpenAiCompat,
                base_url: "http://x".into(),
                api_key_env: "K".into(),
            },
            model: "m".into(),
            max_steps: 5,
            effort: None,
        },
        dir.to_path_buf(),
    )
}

/// Vacía el bus y devuelve los eventos recibidos hasta ese momento.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>) -> Vec<LiveEvent> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn run_woken_without_notices_runs_no_turn_and_emits_wake_idle() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("idle.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    // Si el proveedor llegara a consultarse, dejaría un mensaje del asistente.
    let agent = woken_agent(
        dir.path(),
        store.clone(),
        vec![vec![
            StreamChunk::TextDelta("no debería".into()),
            StreamChunk::Done(FinishReason::Stop),
        ]],
        bus,
    );

    let outcome = agent
        .run_woken(sid, CancellationToken::new())
        .await
        .unwrap();

    assert!(outcome.is_none());
    assert!(store.messages(sid).await.unwrap().is_empty());
    let events = drain(&mut rx);
    assert!(!events
        .iter()
        .any(|event| matches!(event, LiveEvent::TurnStarted | LiveEvent::TurnEnded)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, LiveEvent::WakeIdle))
            .count(),
        1
    );
}

#[tokio::test]
async fn run_woken_with_a_notice_runs_a_turn_without_inventing_a_user_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("woken.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let mut agent = woken_agent(
        dir.path(),
        store.clone(),
        vec![vec![
            StreamChunk::TextDelta("hecho".into()),
            StreamChunk::Done(FinishReason::Stop),
        ]],
        bus,
    );
    let notice = Message::user("[Subagent obrero needs input]\n1. ¿Cuál?");
    agent.subagents = Some(Arc::new(InputNoticeLauncher {
        notices: std::sync::Mutex::new(vec![notice.clone()]),
    }));

    let outcome = agent
        .run_woken(sid, CancellationToken::new())
        .await
        .unwrap()
        .expect("con un aviso debe ejecutarse un turno");

    assert_eq!(outcome.final_text, "hecho");
    let stored = store.messages(sid).await.unwrap();
    let user_texts: Vec<String> = stored
        .iter()
        .filter(|message| message.role == Role::User)
        .map(|message| message.text())
        .collect();
    assert_eq!(user_texts, vec![notice.text()]);
    let events = drain(&mut rx);
    assert!(events
        .iter()
        .any(|event| matches!(event, LiveEvent::TurnStarted)));
    assert!(events
        .iter()
        .any(|event| matches!(event, LiveEvent::TurnEnded)));
    assert!(!events
        .iter()
        .any(|event| matches!(event, LiveEvent::WakeIdle)));
}

#[tokio::test]
async fn flush_subagent_notices_injects_input_notices() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("n.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let mut agent = AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(VecDeque::new()),
        }),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        bus,
        Arc::new(PermissionService::allow_all()),
        LoomcodingConfig {
            provider: crate::config::ProviderConfig {
                kind: crate::config::ProviderKind::OpenAiCompat,
                base_url: "http://x".into(),
                api_key_env: "K".into(),
            },
            model: "m".into(),
            max_steps: 5,
            effort: None,
        },
        dir.path().to_path_buf(),
    );
    let notice = Message::user(
        "[Subagent obrero needs input]
1. ¿Cuál?",
    );
    agent.subagents = Some(Arc::new(InputNoticeLauncher {
        notices: std::sync::Mutex::new(vec![notice.clone()]),
    }));

    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 1);

    let stored = store.messages(sid).await.unwrap();
    assert!(stored.iter().any(|m| m.text() == notice.text()));
    let mut published = false;
    while let Ok(event) = rx.try_recv() {
        if let LiveEvent::SessionMessageAdded { session, message } = event {
            published |= session == sid && message.text() == notice.text();
        }
    }
    assert!(
        published,
        "el aviso debería publicarse como SessionMessageAdded"
    );
}

#[tokio::test]
async fn job_notice_detail_is_capped() {
    use crate::job::{JobRegistry, JobSpec, LocalJobRegistry};
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("tope_job.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let jobs = Arc::new(LocalJobRegistry::default());
    let agent = AgentLoop::new(
        Arc::new(QueueProvider {
            scripts: Mutex::new(VecDeque::new()),
        }),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        LoomcodingConfig {
            provider: crate::config::ProviderConfig {
                kind: crate::config::ProviderKind::OpenAiCompat,
                base_url: "http://x".into(),
                api_key_env: "K".into(),
            },
            model: "m".into(),
            max_steps: 5,
            effort: None,
        },
        dir.path().to_path_buf(),
    )
    .with_jobs(jobs.clone());

    // El job falla con un mensaje de 50 000 bytes: ese texto es el `detail` del aviso.
    let job = jobs
        .start(JobSpec {
            kind: "test".into(),
            label: "tarea larga".into(),
            owner: sid,
            output_limit: 20,
            run: Box::new(|_, _| {
                Box::pin(async { Err(crate::error::CoreError::Tool("e".repeat(50_000))) })
            }),
        })
        .unwrap();
    jobs.wait(sid, job.id()).await.unwrap();

    assert_eq!(agent.flush_subagent_notices(sid).await.unwrap(), 1);
    let stored = store.messages(sid).await.unwrap();
    let aviso = stored
        .iter()
        .map(|m| m.text())
        .find(|texto| texto.starts_with("[Job "))
        .expect("el aviso del job debería quedar en el historial");
    assert!(aviso.starts_with(&format!("[Job {} failed] tarea larga\n", job.id().0)));
    // Tope de 8 000 bytes de detalle más el aviso de recorte; lejos de los 50 000 originales.
    assert!(
        aviso.len() < crate::tools::MAX_NOTICE_BYTES + 300,
        "medía {}",
        aviso.len()
    );
    // ADAPTADO: el aviso se recorta ahora con cabeza y cola y el marcador «bytes omitted».
    assert!(aviso.contains("bytes omitted ...]"));
}
