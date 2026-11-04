use super::*;

/// Proveedor espía: anota cuántas herramientas lleva cada petición y responde con un guion fijo.
struct HandoffSpy {
    tools_seen: Mutex<Vec<usize>>,
    last_user: Mutex<String>,
    reply: Vec<StreamChunk>,
}

#[async_trait]
impl Provider for HandoffSpy {
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
        self.tools_seen.lock().unwrap().push(req.tools.len());
        if let Some(last) = req.messages.last() {
            *self.last_user.lock().unwrap() = last.text();
        }
        Ok(Box::pin(futures::stream::iter(self.reply.clone())))
    }
}

async fn handoff_with(reply: Vec<StreamChunk>) -> (Result<String>, Arc<HandoffSpy>, Vec<Message>) {
    let spy = Arc::new(HandoffSpy {
        tools_seen: Mutex::new(Vec::new()),
        last_user: Mutex::new(String::new()),
        reply,
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("h.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    store
        .append_message(sid, &Message::user("haz algo"))
        .await
        .unwrap();
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
    let agent = AgentLoop::new(
        spy.clone(),
        store.clone(),
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    let result = agent
        .handoff_report(sid, "step limit", &CancellationToken::new())
        .await;
    (result, spy, store.messages(sid).await.unwrap())
}

#[tokio::test]
async fn the_handoff_is_one_request_without_tools() {
    let (result, spy, messages) = handoff_with(vec![
        StreamChunk::TextDelta("hecho A; falta B".into()),
        StreamChunk::Done(FinishReason::Stop),
    ])
    .await;
    assert_eq!(result.unwrap(), "hecho A; falta B");
    assert_eq!(*spy.tools_seen.lock().unwrap(), vec![0]);
    assert!(spy.last_user.lock().unwrap().contains("step limit"));
    // La petición y la respuesta quedan en el historial del hijo, visibles en el diario.
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2].text(), "hecho A; falta B");
}

#[tokio::test]
async fn the_handoff_recovers_an_answer_left_in_the_reasoning() {
    let (result, _, _) = handoff_with(vec![
        StreamChunk::ReasoningDelta("{\"preferred\":\"B\"}".into()),
        StreamChunk::Done(FinishReason::Stop),
    ])
    .await;
    assert_eq!(result.unwrap(), "{\"preferred\":\"B\"}");
}

#[tokio::test]
async fn the_handoff_report_is_capped() {
    let (result, _, _) = handoff_with(vec![
        StreamChunk::TextDelta("x".repeat(10_000)),
        StreamChunk::Done(FinishReason::Stop),
    ])
    .await;
    assert!(result.unwrap().len() <= crate::agent::HANDOFF_MAX_BYTES + 64);
}

#[tokio::test]
async fn an_empty_handoff_is_an_empty_string_not_an_error() {
    let (result, _, _) = handoff_with(vec![StreamChunk::Done(FinishReason::Stop)]).await;
    assert_eq!(result.unwrap(), "");
}
