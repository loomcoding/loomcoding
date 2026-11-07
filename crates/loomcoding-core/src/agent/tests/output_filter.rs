//! Enganche del filtro de salida en el bucle: lo que recibe el modelo es lo que se guarda (R9).

use super::*;
use crate::output_filter::FilterConfig;

/// Herramienta llamada `bash` cuya salida tiene un bloque de líneas en blanco.
struct BashRuidosa;

#[async_trait]
impl Tool for BashRuidosa {
    fn name(&self) -> &str {
        "bash"
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
        Ok(ToolOutput::ok("inicio\n\n\n\nfin"))
    }
}

/// Un turno en el que el modelo llama a `bash`. Devuelve el contenido del resultado tal como queda
/// en el historial, con el filtro dado.
async fn resultado_de_bash(cfg: FilterConfig, dir: &std::path::Path) -> String {
    let scripts = vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "x"}),
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
    tools.register(Arc::new(BashRuidosa));
    let cfg_loom = LoomcodingConfig {
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
        cfg_loom,
        dir.to_path_buf(),
    )
    .with_spill(crate::tools::spill::ToolSpill::new(dir.join("spill")))
    .with_output_filter(cfg);
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
async fn con_el_filtro_encendido_el_modelo_recibe_la_salida_compactada() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap();
    let contenido = resultado_de_bash(cfg, dir.path()).await;

    assert!(
        contenido.starts_with("[filtrado: 2 líneas omitidas de 5"),
        "{contenido}"
    );
    assert!(contenido.ends_with("inicio\n\nfin"), "{contenido}");
}

#[tokio::test]
async fn con_el_filtro_apagado_el_modelo_recibe_la_salida_de_siempre() {
    let dir = tempfile::tempdir().unwrap();
    let contenido = resultado_de_bash(FilterConfig::default(), dir.path()).await;

    assert_eq!(contenido, "inicio\n\n\n\nfin");
}
