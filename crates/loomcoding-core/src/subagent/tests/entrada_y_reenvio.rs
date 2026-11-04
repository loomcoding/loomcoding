use super::*;

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
        Arc::new(RwLock::new(LoomcodingConfig {
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
