use super::*;

/// Catálogo con `acme` (`https://acme.example/v1`) y dos modelos con precios distintos.
struct Voluminosa;

#[async_trait]
impl Tool for Voluminosa {
    fn name(&self) -> &str {
        "voluminosa"
    }
    fn description(&self) -> &str {
        "n"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object"})
    }
    fn execution_mode(&self, _i: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        _c: ToolCtx,
        _i: serde_json::Value,
        _x: CancellationToken,
    ) -> Result<ToolOutput> {
        Ok(ToolOutput::ok("x".repeat(50_000)))
    }
}

/// Un turno en el que el modelo llama a `voluminosa`. Devuelve el contenido del resultado
/// de herramienta tal como queda en el historial.
async fn resultado_de_voluminosa(spill: Option<ToolSpill>, dir: &std::path::Path) -> String {
    let scripts = vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "voluminosa".into(),
                input: serde_json::json!({}),
            },
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        vec![
            StreamChunk::TextDelta("fin".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    ];
    let provider = Arc::new(QueueProvider {
        scripts: Mutex::new(scripts.into()),
    });
    let store = Arc::new(
        SqliteStore::open(dir.join("l.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(Voluminosa));
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
    let mut agent = AgentLoop::new(
        provider,
        store.clone(),
        Arc::new(tools),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.to_path_buf(),
    );
    if let Some(spill) = spill {
        agent = agent.with_spill(spill);
    }
    agent
        .run(sid, "hi".into(), CancellationToken::new())
        .await
        .unwrap();
    let msgs = store.messages(sid).await.unwrap();
    msgs.iter()
        .flat_map(|m| m.parts.iter())
        .find_map(|p| match p {
            ContentPart::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("debe haber un resultado de herramienta")
}

#[tokio::test]
async fn oversized_tool_result_is_spilled_and_the_path_is_in_the_message() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path().join("spill"));
    let contenido = resultado_de_voluminosa(Some(spill), dir.path()).await;

    assert!(
        contenido.contains("[full output saved to: "),
        "sin ruta en el recorte"
    );
    assert!(contenido.len() <= MAX_TOOL_OUTPUT_BYTES);
    assert!(contenido.ends_with(']'));
    let inicio =
        contenido.find("[full output saved to: ").unwrap() + "[full output saved to: ".len();
    let ruta = &contenido[inicio..contenido.len() - 1];
    let guardado = std::fs::read_to_string(ruta).unwrap();
    assert_eq!(guardado, "x".repeat(50_000));
}

#[tokio::test]
async fn truncated_tool_result_opens_with_a_structured_header() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path().join("spill"));
    let con_volcado = resultado_de_voluminosa(Some(spill), dir.path()).await;
    let sin_volcado = resultado_de_voluminosa(None, dir.path()).await;
    for contenido in [&con_volcado, &sin_volcado] {
        // La señal va al principio: el modelo la lee antes que el prefijo que podría tomar por entero.
        assert!(
            contenido.starts_with("[truncated: showing "),
            "sin cabecera: {}",
            &contenido[..contenido.len().min(120)]
        );
        let cabecera = contenido.lines().next().unwrap();
        assert!(cabecera.contains("of 50000 bytes"), "{cabecera}");
        assert!(cabecera.contains("omitted"), "{cabecera}");
        assert!(contenido.len() <= MAX_TOOL_OUTPUT_BYTES);
    }
}

#[test]
fn a_tool_result_that_fits_is_delivered_without_a_header() {
    let contenido = contenido_de_herramienta(None, "call-1", "corto");
    assert_eq!(contenido, "corto");
}

#[tokio::test]
async fn without_spill_the_tool_result_has_no_path() {
    let dir = tempfile::tempdir().unwrap();
    let contenido = resultado_de_voluminosa(None, dir.path()).await;
    assert!(!contenido.contains("saved to"));
    assert!(contenido.contains("bytes omitted"));
}
