use super::*;

/// Ejecuta un turno con el guion dado y devuelve el resultado, los mensajes guardados y los avisos.
async fn run_script(
    scripts: Vec<Vec<StreamChunk>>,
) -> (crate::agent::TurnOutcome, Vec<Message>, Vec<String>) {
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("r.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(NamedTool("view")));
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
    let bus = EventBus::new(64);
    let mut events = bus.subscribe();
    let agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        bus,
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    let outcome = agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let mut notices = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let LiveEvent::RuntimeNotice { message } = event {
            notices.push(message);
        }
    }
    (outcome, store.messages(sid).await.unwrap(), notices)
}

fn text_of(message: &Message) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_final_answer_written_only_in_the_reasoning_is_recovered() {
    let (outcome, messages, notices) = run_script(vec![vec![
        StreamChunk::ReasoningDelta("pienso... ".into()),
        StreamChunk::ReasoningDelta("{\"preferred\":\"B\"}".into()),
        StreamChunk::Done(FinishReason::Stop),
    ]])
    .await;
    assert_eq!(outcome.final_text, "pienso... {\"preferred\":\"B\"}");
    let last = messages.last().unwrap();
    assert_eq!(text_of(last), outcome.final_text);
    assert!(
        notices.iter().any(|n| n.contains("razonamiento")),
        "debe avisar de que el texto viene del razonamiento: {notices:?}"
    );
}

#[tokio::test]
async fn the_reasoning_is_not_used_when_there_is_visible_text() {
    let (outcome, _, notices) = run_script(vec![vec![
        StreamChunk::ReasoningDelta("pensando".into()),
        StreamChunk::TextDelta("hecho".into()),
        StreamChunk::Done(FinishReason::Stop),
    ]])
    .await;
    assert_eq!(outcome.final_text, "hecho");
    assert!(notices.iter().all(|n| !n.contains("razonamiento")));
}

#[tokio::test]
async fn the_reasoning_does_not_replace_a_tool_round() {
    let (outcome, messages, _) = run_script(vec![
        vec![
            StreamChunk::ReasoningDelta("voy a leer".into()),
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "view".into(),
                input: serde_json::json!({}),
            },
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        vec![
            StreamChunk::TextDelta("listo".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ])
    .await;
    assert_eq!(outcome.final_text, "listo");
    let first = messages
        .iter()
        .find(|m| m.role == crate::message::Role::Assistant)
        .unwrap();
    assert_eq!(text_of(first), "", "la ronda de herramientas no gana texto");
}

#[tokio::test]
async fn an_empty_answer_with_no_reasoning_stays_empty() {
    let (outcome, _, notices) = run_script(vec![vec![StreamChunk::Done(FinishReason::Stop)]]).await;
    assert_eq!(outcome.final_text, "");
    assert!(notices.iter().all(|n| !n.contains("razonamiento")));
}

#[test]
fn the_recovered_reasoning_keeps_its_tail_within_the_cap() {
    let mut buf = String::new();
    for _ in 0..100 {
        crate::agent::turn::push_tail_capped(&mut buf, "áé", 64);
    }
    crate::agent::turn::push_tail_capped(&mut buf, "FIN", 64);
    assert!(buf.len() <= 64);
    assert!(buf.ends_with("FIN"));
}
