use super::*;

pub(super) struct QueueProvider {
    pub(super) scripts: Mutex<VecDeque<Vec<StreamChunk>>>,
}

#[async_trait]
impl Provider for QueueProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        _req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let script = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
        Ok(Box::pin(futures::stream::iter(script)))
    }
}

pub(super) struct NamedTool(pub(super) &'static str);

#[async_trait]
impl Tool for NamedTool {
    fn name(&self) -> &str {
        self.0
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
        Ok(ToolOutput::ok("ok"))
    }
}

pub(super) struct Noop;

#[async_trait]
impl Tool for Noop {
    fn name(&self) -> &str {
        "noop"
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
        Ok(ToolOutput::ok("ran"))
    }
}

pub(super) fn usage_chunk(input: u32, output: u32) -> StreamChunk {
    StreamChunk::Usage(crate::message::Usage {
        input_tokens: input,
        output_tokens: output,
        reported_cost_usd: None,
    })
}

/// `n` mensajes de usuario de 4 000 bytes, es decir, 1 000 tokens estimados cada uno.
pub(super) fn historial_grande(n: usize) -> Vec<Message> {
    (0..n).map(|_| Message::user("x".repeat(4000))).collect()
}

/// Una petición es de resumen si no lleva herramientas y su prompt de sistema es el de resumir.
pub(super) fn es_peticion_de_resumen(req: &ProviderRequest) -> bool {
    req.tools.is_empty() && req.system == compact::SUMMARY_SYSTEM_PROMPT
}

/// Proveedor simulado de un turno completo. Distingue la petición de resumen de las normales
/// con `es_peticion_de_resumen`: la de resumen responde `resumen` o falla según `falla_resumen`;
/// las normales salen de `normales` en orden y, agotadas, responden un texto final.
pub(super) struct PlanProvider {
    pub(super) resumen: String,
    pub(super) falla_resumen: bool,
    pub(super) normales: Mutex<VecDeque<Vec<StreamChunk>>>,
    pub(super) seen: Mutex<Vec<ProviderRequest>>,
    /// Cuántas peticiones normales (no de resumen) se rechazan aún por exceso de contexto.
    pub(super) desbordes: Mutex<usize>,
}

#[async_trait]
impl Provider for PlanProvider {
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
        let de_resumen = es_peticion_de_resumen(&req);
        self.seen.lock().unwrap().push(req);
        if de_resumen {
            if self.falla_resumen {
                return Err(crate::error::CoreError::Provider("fallo simulado".into()));
            }
            return Ok(Box::pin(futures::stream::iter(vec![
                StreamChunk::TextDelta(self.resumen.clone()),
                StreamChunk::Done(FinishReason::Stop),
            ])));
        }
        {
            let mut desbordes = self.desbordes.lock().unwrap();
            if *desbordes > 0 {
                *desbordes -= 1;
                return Err(crate::error::CoreError::Provider(
                    "http 400 Bad Request: This model's maximum context length is 8192 tokens"
                        .into(),
                ));
            }
        }
        let guion = self
            .normales
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                vec![
                    StreamChunk::TextDelta("hecho".into()),
                    StreamChunk::Done(FinishReason::Stop),
                ]
            });
        Ok(Box::pin(futures::stream::iter(guion)))
    }
}

/// Catálogo con el proveedor de `http://x` y un modelo `m` de ventana `ventana` tokens.
pub(super) fn catalogo_de_ventana(ventana: u64) -> Arc<RwLock<crate::catalog::Catalog>> {
    let json = format!(
        r#"{{"x": {{"id": "x", "npm": "@ai-sdk/openai-compatible", "api": "http://x", "env": ["K"],
            "models": {{"m": {{"id": "m", "tool_call": true,
                "limit": {{"context": {ventana}, "output": 4096}}}}}}}}}}"#
    );
    Arc::new(RwLock::new(
        crate::catalog::Catalog::from_json(&json).expect("el JSON de ventana es válido"),
    ))
}

pub(super) struct EntornoDeTurno {
    pub(super) _dir: tempfile::TempDir,
    pub(super) store: Arc<SqliteStore>,
    pub(super) sid: SessionId,
    pub(super) proveedor: Arc<PlanProvider>,
    pub(super) agente: AgentLoop,
}

pub(super) async fn entorno_de_turno(
    proveedor: PlanProvider,
    catalogo: Arc<RwLock<crate::catalog::Catalog>>,
    historial: &[Message],
    herramientas: ToolRegistry,
) -> EntornoDeTurno {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("t.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sid = store.create_session("/p").await.unwrap();
    for mensaje in historial {
        store.append_message(sid, mensaje).await.unwrap();
    }
    let proveedor = Arc::new(proveedor);
    let cfg = LoomcodingConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 10,
        effort: None,
    };
    let agente = AgentLoop::new(
        proveedor.clone(),
        store.clone(),
        Arc::new(herramientas),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    )
    .with_catalog(catalogo);
    EntornoDeTurno {
        _dir: dir,
        store,
        sid,
        proveedor,
        agente,
    }
}

pub(super) fn proveedor_de(
    resumen: &str,
    falla_resumen: bool,
    normales: Vec<Vec<StreamChunk>>,
) -> PlanProvider {
    PlanProvider {
        resumen: resumen.to_string(),
        falla_resumen,
        normales: Mutex::new(VecDeque::from(normales)),
        seen: Mutex::new(Vec::new()),
        desbordes: Mutex::new(0),
    }
}

/// Presupuesto solo de tokens, con el límite indicado.
pub(super) fn presupuesto_de_tokens(limite: u64) -> BudgetConfig {
    BudgetConfig {
        session_tokens: Some(limite),
        session_cost_usd: None,
        hard_stop: false,
    }
}

/// Respuesta de texto que declara `entrada` y `salida` tokens de consumo.
pub(super) fn respuesta_con_consumo(texto: &str, entrada: u32, salida: u32) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(texto.into()),
        StreamChunk::Usage(Usage {
            input_tokens: entrada,
            output_tokens: salida,
            reported_cost_usd: None,
        }),
        StreamChunk::Done(FinishReason::Stop),
    ]
}
