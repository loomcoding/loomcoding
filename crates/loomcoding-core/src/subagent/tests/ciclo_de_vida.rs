use super::*;

struct ProveedorQuePublica {
    peticiones: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl Provider for ProveedorQuePublica {
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
        let chunks = if self.peticiones.fetch_add(1, Ordering::SeqCst) == 0 {
            vec![
                StreamChunk::ToolCallComplete {
                    id: "avance".into(),
                    name: "artifact_publish".into(),
                    input: serde_json::json!({"name": "plan", "content": "1. leer"}),
                },
                StreamChunk::Done(FinishReason::ToolCalls),
            ]
        } else {
            vec![
                StreamChunk::TextDelta("resultado final".into()),
                StreamChunk::Done(FinishReason::Stop),
            ]
        };
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

#[tokio::test]
async fn a_child_publishes_progress_while_working_and_the_parent_bus_sees_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("avance.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let provider: Arc<dyn Provider> = Arc::new(ProveedorQuePublica {
        peticiones: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let jobs = Arc::new(LocalJobRegistry::default());
    let bus = EventBus::new(256);
    let mut rx = bus.subscribe();
    let runtime = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
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
        bus,
    );
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
    jobs.wait(parent, &job).await.unwrap();

    // Entre los cambios de tarea hay uno con la tarea en marcha y el avance ya dentro.
    let mut en_marcha = false;
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::SubagentChanged(d) = evento {
            if d.child == child
                && d.status.state == TaskState::Working
                && d.progress_artifacts().count() == 1
            {
                assert_eq!(d.result_text(), None);
                en_marcha = true;
            }
        }
    }
    assert!(
        en_marcha,
        "el avance debía llegar al bus del padre en marcha"
    );
    let hijos = store.list_subagents(parent).await.unwrap();
    assert_eq!(hijos[0].result_text().as_deref(), Some("resultado final"));
    assert_eq!(hijos[0].progress_artifacts().count(), 1);
    // El aviso al padre trae el resultado y solo nombra el avance, sin volcar su contenido.
    let avisos = runtime.deliver_notices(parent).await.unwrap();
    assert_eq!(avisos.len(), 1);
    assert_eq!(
            avisos[0].text(),
            "[Subagent research completed]\nresultado final\n[Progress notes: plan (7 bytes); read with artifact_read]"
        );
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
async fn child_at_step_limit_hands_off_a_partial_report_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("handoff-partial.db").to_str().unwrap())
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
                        name: "echo".into(),
                        input: serde_json::json!({"text": "inspect"}),
                    },
                    StreamChunk::Done(FinishReason::ToolCalls),
                ],
                vec![
                    StreamChunk::TextDelta("leí los ficheros; falta escribir".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ],
            ]
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
    assert_eq!(outcome.status, JobStatus::Completed);
    espera_estado_del_hijo(&store, parent, child, TaskState::Completed).await;
    let descriptor = store
        .list_subagents(parent)
        .await
        .unwrap()
        .into_iter()
        .find(|descriptor| descriptor.child == child)
        .unwrap();
    let result = descriptor.result_text().unwrap();
    assert!(result.starts_with("[PARTIAL: step limit (1/1)"), "{result}");
    assert!(result.contains("leí los ficheros; falta escribir"));
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
        Arc::new(RwLock::new(LoomcodingConfig {
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
