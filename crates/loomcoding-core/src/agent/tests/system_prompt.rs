use super::*;

/// Proveedor que anota el prompt de sistema de cada petición que recibe.
struct SystemSpy {
    seen: Mutex<Vec<String>>,
}

#[async_trait]
impl Provider for SystemSpy {
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
        self.seen.lock().unwrap().push(req.system.clone());
        Ok(Box::pin(futures::stream::iter(vec![StreamChunk::Done(
            FinishReason::Stop,
        )])))
    }
}

#[tokio::test]
async fn the_model_is_told_which_agents_it_can_delegate_to() {
    let dir = tempfile::tempdir().unwrap();
    let agentes = dir.path().join("agentes");
    std::fs::create_dir(&agentes).unwrap();
    std::fs::write(
        agentes.join("rust.md"),
        "---
name: rust
description: Experto en Rust
---

Cuerpo.
",
    )
    .unwrap();
    let registro = Arc::new(crate::registry::AgentRegistry::new(vec![
        Box::new(crate::registry::BuiltInSource),
        Box::new(crate::registry::FileSource::new(
            agentes,
            crate::registry::ProfileLevel::Global,
        )),
    ]));
    let provider = Arc::new(SystemSpy {
        seen: Mutex::new(Vec::new()),
    });
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::subagent::SubagentTool));
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_agent_registry(registro);
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let vistos = provider.seen.lock().unwrap();
    assert!(
        vistos[0].contains("- rust (global): Experto en Rust"),
        "el modelo debe ver el agente: {}",
        vistos[0]
    );
}

#[tokio::test]
async fn the_system_prompt_sections_are_capped() {
    let dir = tempfile::tempdir().unwrap();
    let agentes = dir.path().join("agentes");
    std::fs::create_dir(&agentes).unwrap();
    // Cien agentes con descripción larga: la lista, sin tope, pasaría de los 20 KB.
    for n in 0..100 {
        std::fs::write(
            agentes.join(format!("a{n}.md")),
            format!(
                "---\nname: a{n}\ndescription: {}\n---\n\nCuerpo.\n",
                "d".repeat(200)
            ),
        )
        .unwrap();
    }
    std::fs::write(
        dir.path().join("AGENTS.md"),
        "c".repeat(crate::config::context::MAX_CONTEXT_FILE_BYTES * 5),
    )
    .unwrap();
    let registro = Arc::new(crate::registry::AgentRegistry::new(vec![
        Box::new(crate::registry::BuiltInSource),
        Box::new(crate::registry::FileSource::new(
            agentes,
            crate::registry::ProfileLevel::Global,
        )),
    ]));
    let provider = Arc::new(SystemSpy {
        seen: Mutex::new(Vec::new()),
    });
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::subagent::SubagentTool));
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_agent_registry(registro)
    .with_system_extra(Some("e".repeat(MAX_SYSTEM_EXTRA_BYTES * 5)));
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let vistos = provider.seen.lock().unwrap();
    // Tres secciones, cada una con su tope más el marcador y los separadores.
    let techo = crate::config::context::MAX_CONTEXT_FILE_BYTES
        + MAX_AGENT_SECTION_BYTES
        + MAX_SYSTEM_EXTRA_BYTES
        + 600;
    assert!(
        vistos[0].len() <= techo,
        "el prompt de sistema mide {} y el techo es {techo}",
        vistos[0].len()
    );
    assert!(vistos[0].contains("AGENTS.md recortado"));
    assert!(vistos[0].contains("bytes omitted"));
}

#[tokio::test]
async fn the_model_receives_the_configured_rule() {
    let dir = tempfile::tempdir().unwrap();
    let agentes = dir.path().join("agentes");
    std::fs::create_dir(&agentes).unwrap();
    std::fs::write(
        agentes.join("rust.md"),
        "---
name: rust
description: Experto en Rust
---

Cuerpo.
",
    )
    .unwrap();
    let registro = Arc::new(crate::registry::AgentRegistry::new(vec![
        Box::new(crate::registry::BuiltInSource),
        Box::new(crate::registry::FileSource::new(
            agentes,
            crate::registry::ProfileLevel::Global,
        )),
    ]));
    let provider = Arc::new(SystemSpy {
        seen: Mutex::new(Vec::new()),
    });
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::subagent::SubagentTool));
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_agent_registry(registro)
    .with_delegation_confirm(ConfirmDelegation::Always);
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let vistos = provider.seen.lock().unwrap();
    assert!(
        vistos[0].contains("Do not start any subagent until"),
        "el modelo debe recibir la regla configurada: {}",
        vistos[0]
    );
}

/// Proveedor que guarda el historial de cada petición recibida.
struct HistorySpy {
    seen: Mutex<Vec<Vec<crate::message::Message>>>,
}

#[async_trait]
impl Provider for HistorySpy {
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
        self.seen.lock().unwrap().push(req.messages);
        Ok(Box::pin(futures::stream::iter(vec![StreamChunk::Done(
            FinishReason::Stop,
        )])))
    }
}

#[tokio::test]
async fn request_clears_old_tool_results_but_store_keeps_them() {
    let provider = Arc::new(HistorySpy {
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("c.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    for i in 0..10 {
        let call_id = format!("c{i}");
        store
            .append_message(
                sid,
                // El input varía en cada llamada: con firmas iguales, `should_stop` cortaría el turno.
                &Message::assistant(vec![ContentPart::ToolCall {
                    id: call_id.clone(),
                    name: "noop".into(),
                    input: serde_json::json!({ "i": i }),
                }]),
            )
            .await
            .unwrap();
        store
            .append_message(
                sid,
                &Message::new(
                    Role::Tool,
                    vec![ContentPart::ToolResult {
                        call_id,
                        content: "x".repeat(3000),
                        is_error: false,
                    }],
                ),
            )
            .await
            .unwrap();
    }

    let cfg = LoomcodingConfig {
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

    let contents_of = |messages: &[Message]| -> Vec<String> {
        messages
            .iter()
            .flat_map(|m| m.parts.iter())
            .filter_map(|p| match p {
                ContentPart::ToolResult { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect()
    };

    let sent = {
        let seen = provider.seen.lock().unwrap();
        contents_of(&seen[0])
    };
    assert_eq!(sent.len(), 10);
    for content in &sent[..4] {
        assert_eq!(
            content,
            "[tool result cleared: 3000 bytes; run the tool again if you need it]"
        );
    }
    for content in &sent[4..] {
        assert_eq!(content.len(), 3000);
    }

    let stored = contents_of(&store.messages(sid).await.unwrap());
    assert_eq!(stored.len(), 10);
    assert!(stored.iter().all(|c| c.len() == 3000));
}

/// Prompt de sistema de la primera petición de un agente con registro (puede delegar) y el modo dado.
async fn system_with_delegation(mode: Option<crate::config::DelegationMode>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let registro = Arc::new(crate::registry::AgentRegistry::new(vec![Box::new(
        crate::registry::BuiltInSource,
    )]));
    let provider = Arc::new(SystemSpy {
        seen: Mutex::new(Vec::new()),
    });
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::subagent::SubagentTool));
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_agent_registry(registro)
    .with_delegation_mode(mode);
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let vistos = provider.seen.lock().unwrap();
    vistos[0].clone()
}

#[tokio::test]
async fn prompt_tells_the_model_to_batch_independent_reads() {
    let system = system_with_delegation(None).await;
    assert!(
        system.contains(crate::agent::PARALLEL_GUIDANCE),
        "falta la guía de paralelismo: {system}"
    );
    assert!(system.contains("same response"));
}

#[tokio::test]
async fn delegation_hint_is_absent_when_delegation_is_off() {
    let system = system_with_delegation(Some(crate::config::DelegationMode::Off)).await;
    assert!(
        !system.contains(crate::agent::PARALLEL_GUIDANCE),
        "{system}"
    );
}

#[test]
fn parallel_guidance_stays_under_its_cap() {
    assert!(crate::agent::PARALLEL_GUIDANCE.len() <= 400);
}
