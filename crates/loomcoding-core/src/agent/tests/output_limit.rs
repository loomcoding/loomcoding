//! Una respuesta cortada por el tope de salida no es una respuesta terminada: la oficina lo dice.

use super::steps::{final_round, run_with_limits};
use super::*;
use crate::agent::stop::StopReason;
use crate::config::LimitsConfig;

fn allow() -> Arc<PermissionService> {
    Arc::new(PermissionService::allow_all())
}

fn cut_tool_round(n: usize) -> Vec<StreamChunk> {
    vec![
        StreamChunk::ToolCallComplete {
            id: format!("c{n}"),
            name: "edit".into(),
            input: serde_json::Value::Null,
        },
        StreamChunk::Done(FinishReason::Length),
    ]
}

#[tokio::test]
async fn a_text_cut_by_the_output_limit_is_an_unfinished_turn() {
    let scripts = vec![vec![
        StreamChunk::TextDelta("I have enough context. I have enough".into()),
        StreamChunk::Done(FinishReason::Length),
    ]];
    let (outcome, _) = run_with_limits(scripts, 5, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.stopped, Some(StopReason::OutputLimit));
    let report = outcome.unfinished_report().expect("turno incompleto");
    assert!(report.contains("output limit"), "{report}");
}

/// Herramienta `edit` que cuenta cuántas veces se ejecuta de verdad.
struct CountingEdit(Arc<std::sync::atomic::AtomicUsize>);

#[async_trait]
impl Tool for CountingEdit {
    fn name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "test tool"
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
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ToolOutput::ok("ok"))
    }
}

#[tokio::test]
async fn a_tool_call_cut_by_the_output_limit_is_not_run_and_the_model_is_told_why() {
    let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(vec![cut_tool_round(1), final_round()].into()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(CountingEdit(runs.clone())));
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
        Arc::new(tools),
        EventBus::new(4096),
        allow(),
        cfg,
        dir.path().to_path_buf(),
    );
    let outcome = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(outcome.final_text, "done");
    assert_eq!(outcome.stopped, None);
    assert_eq!(provider.scripts.lock().unwrap().len(), 0);
    assert_eq!(
        runs.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "una llamada cortada no debe ejecutarse"
    );
    let history = store.recent_messages(sid, 20).await.unwrap();
    let told = history.iter().any(|m| {
        m.parts.iter().any(|p| {
            matches!(p, ContentPart::ToolResult { content, is_error: true, .. }
                if content.contains("output limit"))
        })
    });
    assert!(told, "el modelo debe saber por qué no se ejecutó");
}

#[tokio::test]
async fn repeated_cuts_end_the_turn_instead_of_looping() {
    let scripts = vec![
        cut_tool_round(1),
        cut_tool_round(2),
        cut_tool_round(3),
        final_round(),
    ];
    let (outcome, left) = run_with_limits(scripts, 20, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.stopped, Some(StopReason::OutputLimit));
    assert_eq!(left, 1, "la cuarta ronda no debía consultarse");
}

#[tokio::test]
async fn a_normal_stop_is_not_an_output_limit() {
    let (outcome, _) =
        run_with_limits(vec![final_round()], 5, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.stopped, None);
    assert_eq!(outcome.unfinished_report(), None);
}

/// Proveedor que anota el `max_tokens` de cada petición.
struct CapProvider {
    seen: Mutex<Vec<u32>>,
}

#[async_trait]
impl Provider for CapProvider {
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
        self.seen.lock().unwrap().push(req.max_tokens);
        Ok(Box::pin(futures::stream::iter(final_round())))
    }
}

async fn cap_seen(limits: LimitsConfig) -> Vec<u32> {
    cap_seen_in(limits, None, "hi".into()).await
}

/// Como `cap_seen`, con una ventana de contexto conocida (catálogo) y un mensaje del tamaño que se quiera.
async fn cap_seen_in(limits: LimitsConfig, window: Option<u64>, prompt: String) -> Vec<u32> {
    let provider = Arc::new(CapProvider {
        seen: Mutex::new(Vec::new()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("c.db").to_str().unwrap())
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
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        allow(),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_limits(limits);
    let agent = match window {
        Some(window) => agent.with_catalog(super::support::catalogo_de_ventana(window)),
        None => agent,
    };
    agent
        .run(sid, prompt, CancellationToken::new())
        .await
        .unwrap();
    let seen = provider.seen.lock().unwrap().clone();
    seen
}

#[tokio::test]
async fn the_request_carries_the_default_output_cap() {
    assert_eq!(cap_seen(LimitsConfig::default()).await, vec![4096]);
}

#[tokio::test]
async fn the_request_carries_the_configured_output_cap() {
    let limits = LimitsConfig {
        max_output_tokens: 12345,
        ..LimitsConfig::default()
    };
    assert_eq!(cap_seen(limits).await, vec![12345]);
}

#[tokio::test]
async fn the_output_cap_shrinks_to_the_room_left_in_a_small_window() {
    // Ventana de 6000 tokens y un mensaje de unos 1500: con 4096 de salida la petición no cabría.
    let seen = cap_seen_in(LimitsConfig::default(), Some(6000), "x".repeat(6000)).await;
    assert_eq!(seen.len(), 1);
    assert!(seen[0] < 4096, "el tope debía encogerse, fue {}", seen[0]);
    assert!(
        seen[0] >= 512,
        "nunca por debajo del suelo, fue {}",
        seen[0]
    );
}

#[tokio::test]
async fn a_roomy_window_leaves_the_output_cap_alone() {
    let seen = cap_seen_in(LimitsConfig::default(), Some(1_000_000), "hi".into()).await;
    assert_eq!(seen, vec![4096]);
}
