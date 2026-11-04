use super::*;
use crate::config::LimitsConfig;

pub(super) fn round(tool: &str, n: usize) -> Vec<StreamChunk> {
    vec![
        StreamChunk::ToolCallComplete {
            id: format!("c{n}"),
            name: tool.into(),
            input: serde_json::json!({ "n": n }),
        },
        StreamChunk::Done(FinishReason::ToolCalls),
    ]
}

pub(super) fn final_round() -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta("done".into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

/// Ejecuta un turno con `view` y `edit` registradas; devuelve el resultado y los guiones sin consumir.
pub(super) async fn run_with_limits(
    scripts: Vec<Vec<StreamChunk>>,
    max_steps: usize,
    limits: LimitsConfig,
    permission: Arc<PermissionService>,
) -> (crate::agent::TurnOutcome, usize) {
    run_configured(scripts, max_steps, limits, permission, false).await
}

/// Igual que `run_with_limits`; con `child` el agente cuenta como subagente de su propia sesión.
pub(super) async fn run_configured(
    scripts: Vec<Vec<StreamChunk>>,
    max_steps: usize,
    limits: LimitsConfig,
    permission: Arc<PermissionService>,
    child: bool,
) -> (crate::agent::TurnOutcome, usize) {
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(NamedTool("view")));
    tools.register(Arc::new(NamedTool("edit")));
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps,
        effort: None,
    };
    let agent = AgentLoop::new(
        provider.clone(),
        store,
        Arc::new(tools),
        EventBus::new(4096),
        permission,
        cfg,
        dir.path().to_path_buf(),
    )
    .with_limits(limits);
    let agent = if child {
        agent.with_budget_family(sid)
    } else {
        agent
    };
    let outcome = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let left = provider.scripts.lock().unwrap().len();
    (outcome, left)
}

fn allow() -> Arc<PermissionService> {
    Arc::new(PermissionService::allow_all())
}

#[tokio::test]
async fn read_only_rounds_do_not_spend_the_work_budget() {
    let mut scripts: Vec<_> = (0..5).map(|n| round("view", n)).collect();
    scripts.push(final_round());
    let (outcome, _) = run_with_limits(scripts, 2, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.final_text, "done");
    assert_eq!(
        outcome.steps, 1,
        "solo la ronda final gasta presupuesto de trabajo"
    );
}

#[tokio::test]
async fn read_rounds_are_capped_by_max_read_steps() {
    let mut scripts: Vec<_> = (0..10).map(|n| round("view", n)).collect();
    scripts.push(final_round());
    let limits = LimitsConfig {
        max_read_steps: Some(3),
        ..LimitsConfig::default()
    };
    let (outcome, left) = run_with_limits(scripts, 30, limits, allow()).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 8, "tres lecturas consumidas de once guiones");
}

#[tokio::test]
async fn a_write_round_spends_the_work_budget() {
    let scripts = vec![round("edit", 0), final_round()];
    let (outcome, left) = run_with_limits(scripts, 1, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 1);
}

#[tokio::test]
async fn a_round_mixing_a_read_and_a_write_counts_as_work() {
    let mixed = vec![
        StreamChunk::ToolCallComplete {
            id: "a".into(),
            name: "view".into(),
            input: serde_json::json!({}),
        },
        StreamChunk::ToolCallComplete {
            id: "b".into(),
            name: "edit".into(),
            input: serde_json::json!({}),
        },
        StreamChunk::Done(FinishReason::ToolCalls),
    ];
    let (outcome, left) = run_with_limits(
        vec![mixed, final_round()],
        1,
        LimitsConfig::default(),
        allow(),
    )
    .await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 1);
}

#[tokio::test]
async fn a_single_work_step_with_only_reads_still_terminates() {
    // `max_steps = 1` con lecturas sin fin: manda el tope de lecturas (4 × max_steps), no un bucle eterno.
    let scripts: Vec<_> = (0..30).map(|n| round("view", n)).collect();
    let (outcome, left) = run_with_limits(scripts, 1, LimitsConfig::default(), allow()).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 26);
}
