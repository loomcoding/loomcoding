use super::*;

fn priced_catalog() -> Arc<RwLock<crate::catalog::Catalog>> {
    let catalog = crate::catalog::Catalog::from_json(
        r#"{"acme": {"id": "acme", "npm": "@ai-sdk/openai-compatible",
            "api": "https://acme.example/v1", "env": ["ACME_KEY"],
            "models": {
                "m-caro": {"id": "m-caro", "tool_call": true, "cost": {"input": 3, "output": 15}},
                "m-barato": {"id": "m-barato", "tool_call": true, "cost": {"input": 0.5, "output": 1}}
            }}}"#,
    )
    .expect("el JSON de precios es válido");
    Arc::new(RwLock::new(catalog))
}

/// Configuración del proveedor `acme` con el modelo indicado.
fn acme_config(model: &str) -> CodeLoomConfig {
    CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "https://acme.example/v1".into(),
            api_key_env: "K".into(),
        },
        model: model.into(),
        max_steps: 5,
        effort: None,
    }
}

#[tokio::test]
async fn agent_records_one_usage_row_per_response_with_current_model_and_price() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("a.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    // Como Anthropic: dos fragmentos de uso en una misma respuesta.
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                StreamChunk::TextDelta("hola".into()),
                StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: Some(10),
                    output_tokens: Some(1),
                    reported_cost_usd: None,
                    final_update: false,
                }),
                StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: None,
                    output_tokens: Some(6),
                    reported_cost_usd: None,
                    final_update: true,
                }),
                StreamChunk::Done(FinishReason::Stop),
            ]]
            .into(),
        ),
    });
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        acme_config("m-caro"),
        dir.path().to_path_buf(),
    )
    .with_catalog(priced_catalog());
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 1, "una respuesta, un apunte");
    assert_eq!(rows[0].model, "m-caro");
    assert_eq!(rows[0].input_tokens, 10);
    assert_eq!(rows[0].output_tokens, 6);
    assert_eq!(
        (rows[0].input_price, rows[0].output_price),
        (Some(3.0), Some(15.0))
    );
}

#[tokio::test]
async fn agent_records_cache_tokens_reported_by_the_provider() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("cache.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    // La caché llega repetida y acumulada, como en Anthropic: vale el máximo.
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                StreamChunk::TextDelta("hola".into()),
                StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: Some(10),
                    output_tokens: Some(1),
                    reported_cost_usd: None,
                    final_update: false,
                }),
                StreamChunk::CacheUsage(crate::provider::CacheUsage {
                    read_tokens: 900,
                    write_tokens: 40,
                }),
                StreamChunk::UsageUpdate(crate::provider::UsageUpdate {
                    input_tokens: None,
                    output_tokens: Some(6),
                    reported_cost_usd: None,
                    final_update: true,
                }),
                StreamChunk::CacheUsage(crate::provider::CacheUsage {
                    read_tokens: 900,
                    write_tokens: 40,
                }),
                StreamChunk::Done(FinishReason::Stop),
            ]]
            .into(),
        ),
    });
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        acme_config("m-caro"),
        dir.path().to_path_buf(),
    )
    .with_catalog(priced_catalog());
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].cache_read_tokens, rows[0].cache_write_tokens),
        (900, 40)
    );
    // La entrada no se reescribe: sigue siendo la que comunicó el proveedor.
    assert_eq!(rows[0].input_tokens, 10);
}

#[tokio::test]
async fn agent_records_null_prices_without_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("sin.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                usage_chunk(7, 3),
                StreamChunk::Done(FinishReason::Stop),
            ]]
            .into(),
        ),
    });
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        acme_config("m-caro"),
        dir.path().to_path_buf(),
    );
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].input_tokens, rows[0].output_tokens), (7, 3));
    assert_eq!((rows[0].input_price, rows[0].output_price), (None, None));
    assert_eq!(rows[0].cost(), None);
}

#[tokio::test]
async fn agent_usage_record_carries_agent_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("sesion-agente.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![vec![
                StreamChunk::TextDelta("hola".into()),
                usage_chunk(4, 2),
                StreamChunk::Done(FinishReason::Stop),
            ]]
            .into(),
        ),
    });
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        acme_config("m-caro"),
        dir.path().to_path_buf(),
    )
    .with_catalog(priced_catalog());
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();

    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 1, "una respuesta, un apunte");
    assert_eq!(
        rows[0].session, sid,
        "el apunte lleva la sesión del propio agente"
    );
}

#[tokio::test]
async fn model_switch_mid_session_keeps_each_price() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("cambio.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(
            vec![
                vec![usage_chunk(100, 10), StreamChunk::Done(FinishReason::Stop)],
                vec![usage_chunk(200, 20), StreamChunk::Done(FinishReason::Stop)],
            ]
            .into(),
        ),
    });
    let agent = AgentLoop::new(
        provider.clone(),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        acme_config("m-caro"),
        dir.path().to_path_buf(),
    )
    .with_catalog(priced_catalog());
    agent
        .run(sid, "uno".into(), CancellationToken::new())
        .await
        .unwrap();
    agent.update_provider(provider.clone(), acme_config("m-barato"));
    agent
        .run(sid, "dos".into(), CancellationToken::new())
        .await
        .unwrap();

    let rows = store.session_usage(sid).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].model, "m-caro");
    assert_eq!(
        (rows[0].input_price, rows[0].output_price),
        (Some(3.0), Some(15.0))
    );
    assert_eq!(rows[1].model, "m-barato");
    assert_eq!(
        (rows[1].input_price, rows[1].output_price),
        (Some(0.5), Some(1.0))
    );
    // 100·3 + 10·15 + 200·0.5 + 20·1, todo por millón: 0.00057 dólares.
    let total: f64 = rows.iter().filter_map(|row| row.cost()).sum();
    assert!(
        (total - 0.00057).abs() < 1e-12,
        "total de la sesión: {total}"
    );
}
