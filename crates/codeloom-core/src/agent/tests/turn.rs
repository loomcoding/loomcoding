use super::*;

#[test]
fn provider_state_is_reattached_only_to_the_matching_tool_call_message() {
    let mut history = vec![Message::assistant(vec![ContentPart::ToolCall {
        id: "call-1".into(),
        name: "view".into(),
        input: serde_json::json!({}),
    }])];
    let state = crate::message::ProviderState {
        provider: "openai_responses".into(),
        data: serde_json::json!({"opaque":"transient"}),
    };
    history.push(Message::assistant(vec![ContentPart::ToolCall {
        id: "call-2".into(),
        name: "grep".into(),
        input: serde_json::json!({}),
    }]));
    assert!(reattach_provider_state(
        &mut history,
        &["call-1".into()],
        state.clone()
    ));
    let second_state = crate::message::ProviderState {
        provider: "anthropic".into(),
        data: serde_json::json!({"opaque":"second transient"}),
    };
    assert!(reattach_provider_state(
        &mut history,
        &["call-2".into()],
        second_state.clone()
    ));
    assert_eq!(history[0].provider_state, Some(state));
    assert_eq!(history[1].provider_state, Some(second_state));
    assert!(!reattach_provider_state(
        &mut history,
        &["different-call".into()],
        crate::message::ProviderState {
            provider: "openai_responses".into(),
            data: serde_json::json!({}),
        }
    ));
}

/// Proveedor que anota el esfuerzo de cada petición que recibe.
struct EffortSpy {
    seen: Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl Provider for EffortSpy {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: false,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        self.seen.lock().unwrap().push(req.effort.clone());
        Ok(Box::pin(futures::stream::iter(vec![StreamChunk::Done(
            FinishReason::Stop,
        )])))
    }
}

#[tokio::test]
async fn agent_passes_config_effort_to_the_request() {
    let provider = Arc::new(EffortSpy {
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("e.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: Some("high".into()),
    };
    let agent = AgentLoop::new(
        provider.clone(),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        provider.seen.lock().unwrap().as_slice(),
        &[Some("high".to_string())]
    );
}

struct ToolNamesProvider {
    seen: Mutex<Vec<Vec<String>>>,
}

#[async_trait]
impl Provider for ToolNamesProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        self.seen
            .lock()
            .unwrap()
            .push(req.tools.into_iter().map(|tool| tool.name).collect());
        Ok(Box::pin(futures::stream::iter(vec![StreamChunk::Done(
            FinishReason::Stop,
        )])))
    }
}

#[tokio::test]
async fn runs_tool_then_finishes() {
    let scripts = vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "noop".into(),
                input: serde_json::json!({}),
            },
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        vec![
            StreamChunk::TextDelta("done".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ];
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("l.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(Noop));

    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );

    let outcome = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(outcome.steps, 2);
    assert_eq!(outcome.final_text, "done");
    let msgs = store.messages(sid).await.unwrap();
    assert!(msgs.iter().any(|m| m.role == Role::Tool));
}

#[tokio::test]
async fn successful_workflow_start_ends_the_parent_turn() {
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                StreamChunk::TextDelta("started".into()),
                StreamChunk::ToolCallComplete {
                    id: "start".into(),
                    name: "development_workflow".into(),
                    input: serde_json::json!({"action":"start"}),
                },
                StreamChunk::Done(FinishReason::ToolCalls),
            ]]
            .into(),
        ),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("workflow-start.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let session = store.create_session("/project").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(NamedTool("development_workflow")));
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(16),
        Arc::new(PermissionService::allow_all()),
        CodeLoomConfig {
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

    let outcome = agent
        .run(session, "start workflow".into(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(outcome.steps, 1);
    assert_eq!(outcome.final_text, "started");
    assert!(store
        .messages(session)
        .await
        .unwrap()
        .iter()
        .any(|message| message.role == Role::Tool));
}

#[tokio::test]
async fn active_workflow_hides_blocked_tools_from_the_provider() {
    let provider = Arc::new(ToolNamesProvider {
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("workflow-tools.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let session = store.create_session("/project").await.unwrap();
    store
        .create_development_workflow(
            crate::workflow::DevelopmentWorkflow {
                id: crate::workflow::WorkflowId(0),
                parent: session,
                task: "test workflow tool scope".into(),
                planner: "planner".into(),
                implementer: "builder".into(),
                reviewer: "reviewer".into(),
                phase: crate::workflow::WorkflowPhase::Specifying,
                version: 0,
                active_child: None,
                planner_child: None,
                implementation_children: Vec::new(),
                reviewer_children: Vec::new(),
                correction_round: 0,
                spec_path: None,
                plan_path: None,
                spec_sha256: None,
                plan_sha256: None,
                last_report: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            },
            "test workflow tool scope",
        )
        .await
        .unwrap();
    let mut tools = ToolRegistry::new();
    for name in [
        "development_workflow",
        "view",
        "job_list",
        "bash",
        "subagent",
    ] {
        tools.register(Arc::new(NamedTool(name)));
    }
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(16),
        Arc::new(PermissionService::allow_all()),
        CodeLoomConfig {
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

    agent
        .run(
            session,
            "continue workflow".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    let seen = provider.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].contains(&"development_workflow".into()));
    assert!(seen[0].contains(&"view".into()));
    assert!(!seen[0].contains(&"job_list".into()));
    assert!(!seen[0].contains(&"bash".into()));
}

#[tokio::test]
async fn stream_error_preserves_partial_text_and_skips_tool_calls() {
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                StreamChunk::TextDelta("partial".into()),
                StreamChunk::ToolCallComplete {
                    id: "c1".into(),
                    name: "noop".into(),
                    input: serde_json::json!({}),
                },
                StreamChunk::Error("response stream timed out".into()),
            ]]
            .into(),
        ),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("stream-error.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(Noop));
    let bus = EventBus::new(64);
    let mut events = bus.subscribe();
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        bus,
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );

    assert!(matches!(
        agent.run(sid, "hi".into(), CancellationToken::new()).await,
        Err(crate::error::CoreError::Provider(error)) if error == "response stream timed out"
    ));
    let messages = store.messages(sid).await.unwrap();
    assert_eq!(
        messages[0].origin,
        MessageOrigin::Human,
        "el prompt que inicia el turno externo conserva su procedencia humana"
    );
    let partial = messages
        .iter()
        .find(|message| message.role == Role::Assistant)
        .unwrap();
    assert_eq!(partial.text(), "partial");
    assert!(partial.parts.iter().any(|part| matches!(
        part,
        ContentPart::Finish {
            reason: FinishReason::Error
        }
    )));
    assert!(!partial
        .parts
        .iter()
        .any(|part| matches!(part, ContentPart::ToolCall { .. })));
    assert!(!messages.iter().any(|message| message.role == Role::Tool));
    let notices: Vec<_> = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            LiveEvent::RuntimeNotice { message } => Some(message),
            _ => None,
        })
        .collect();
    assert!(notices.contains(&"provider request started for `m`".into()));
    assert!(notices.contains(&"provider response received for `m`".into()));
}

#[tokio::test]
async fn denied_tool_stops_and_is_recorded() {
    let scripts = vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "noop".into(),
                input: serde_json::json!({}),
            },
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        vec![
            StreamChunk::TextDelta("after".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ];
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("d.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(Noop));
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(crate::permission::PermissionService::with_responder(
            Box::new(crate::permission::DenyAll),
        )),
        cfg,
        dir.path().to_path_buf(),
    );
    let _ = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let msgs = store.messages(sid).await.unwrap();
    assert!(msgs.iter().any(|m| m
        .parts
        .iter()
        .any(|p| matches!(p, ContentPart::ToolResult { is_error: true, .. }))));
}

#[tokio::test]
async fn cancelled_turn_marks_partial_message_as_error() {
    struct CancelMidStream;

    #[async_trait]
    impl Provider for CancelMidStream {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                tool_use: false,
                reasoning: false,
                streaming: true,
            }
        }

        async fn stream(
            &self,
            _req: ProviderRequest,
            cancel: CancellationToken,
        ) -> Result<ProviderStream> {
            Ok(Box::pin(futures::stream::unfold(
                (false, cancel),
                |(sent_delta, cancel)| async move {
                    if sent_delta {
                        tokio::task::yield_now().await;
                        cancel.cancel();
                        None
                    } else {
                        Some((StreamChunk::TextDelta("partial".into()), (true, cancel)))
                    }
                },
            )))
        }
    }

    let provider = Arc::new(CancelMidStream);
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("c.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    let _ = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let msgs = store.messages(sid).await.unwrap();
    assert!(msgs.iter().any(|m| m.parts.iter().any(|p| matches!(
        p,
        ContentPart::Finish {
            reason: FinishReason::Error
        }
    ))));
}

#[tokio::test]
async fn edit_publishes_a_file_diff() {
    let scripts = vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "edit".into(),
                input: serde_json::json!({"path":"a.txt","old":"world","new":"rust"}),
            },
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        vec![
            StreamChunk::TextDelta("done".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ];
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello world").unwrap();

    let store = Arc::new(
        SqliteStore::open(dir.path().join("l.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();

    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::edit::EditTool));

    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        bus,
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );

    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let mut found = None;
    while let Ok(ev) = rx.try_recv() {
        if let LiveEvent::FileDiff {
            path,
            added,
            removed,
        } = ev
        {
            found = Some((path, added, removed));
        }
    }
    assert_eq!(found, Some(("a.txt".to_string(), 1, 1)));
}
