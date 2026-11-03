use super::*;

/// Almacén que cuenta las lecturas del consumo acumulado y delega todo lo demás en el real.
struct AlmacenContador {
    dentro: Arc<SqliteStore>,
    lecturas_de_consumo: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl SessionStore for AlmacenContador {
    async fn create_session(&self, project: &str) -> Result<SessionId> {
        self.dentro.create_session(project).await
    }
    async fn list_sessions(&self, project: &str) -> Result<Vec<crate::session::SessionInfo>> {
        self.dentro.list_sessions(project).await
    }
    async fn session_belongs_to_project(&self, session: SessionId, project: &str) -> Result<bool> {
        self.dentro
            .session_belongs_to_project(session, project)
            .await
    }
    async fn append_message(&self, sid: SessionId, msg: &Message) -> Result<()> {
        self.dentro.append_message(sid, msg).await
    }
    async fn messages(&self, sid: SessionId) -> Result<Vec<Message>> {
        self.dentro.messages(sid).await
    }
    async fn save_subagent(&self, descriptor: &crate::session::SubagentDescriptor) -> Result<()> {
        self.dentro.save_subagent(descriptor).await
    }
    async fn update_subagent_status(
        &self,
        parent: SessionId,
        child: SessionId,
        status: crate::task::TaskStatus,
    ) -> Result<()> {
        self.dentro
            .update_subagent_status(parent, child, status)
            .await
    }
    async fn list_subagents(
        &self,
        parent: SessionId,
    ) -> Result<Vec<crate::session::SubagentDescriptor>> {
        self.dentro.list_subagents(parent).await
    }
    async fn complete_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        artifacts: &[crate::a2a::Artifact],
    ) -> Result<()> {
        self.dentro
            .complete_subagent(parent, child, artifacts)
            .await
    }
    async fn append_subagent_notice(
        &self,
        parent: SessionId,
        child: SessionId,
        message: &Message,
    ) -> Result<bool> {
        self.dentro
            .append_subagent_notice(parent, child, message)
            .await
    }
    async fn record_usage(&self, session: SessionId, record: &UsageRecord) -> Result<()> {
        self.dentro.record_usage(session, record).await
    }
    async fn session_usage(&self, session: SessionId) -> Result<Vec<UsageRecord>> {
        self.lecturas_de_consumo
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.dentro.session_usage(session).await
    }
}

/// Niveles y consumo de los avisos de presupuesto que hay en el bus, en orden de llegada.
fn avisos_del_bus(rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>) -> Vec<(u8, f64)> {
    let mut avisos = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::BudgetWarning {
            level_percent,
            used,
            ..
        } = evento
        {
            avisos.push((level_percent, used));
        }
    }
    avisos
}

#[tokio::test]
async fn usage_crossing_the_budget_publishes_a_warning() {
    let proveedor = proveedor_de("", false, vec![respuesta_con_consumo("hola", 700, 100)]);
    let mut e = entorno_de_turno(
        proveedor,
        catalogo_de_ventana(100_000),
        &[],
        ToolRegistry::new(),
    )
    .await;
    e.agente = e.agente.with_budget(presupuesto_de_tokens(1000));
    let mut rx = e.agente.bus.subscribe();

    e.agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    // 800 tokens sobre un límite de 1000: se cruza el 80 % y no el 100 %.
    assert_eq!(avisos_del_bus(&mut rx), vec![(80, 800.0)]);
}

#[tokio::test]
async fn each_request_publishes_its_context_breakdown_once() {
    let proveedor = proveedor_de("", false, vec![respuesta_con_consumo("hola", 10, 1)]);
    let e = entorno_de_turno(
        proveedor,
        catalogo_de_ventana(100_000),
        &[],
        ToolRegistry::new(),
    )
    .await;
    let mut rx = e.agente.bus.subscribe();
    e.agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    let mut desgloses = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::ContextBreakdown { breakdown, .. } = evento {
            desgloses.push(breakdown);
        }
    }
    assert_eq!(desgloses.len(), 1, "una petición, un desglose");
    let peticion = e.proveedor.seen.lock().unwrap()[0].clone();
    let esperado =
        compact::breakdown_request(&peticion.system, &peticion.messages, &peticion.tools);
    assert_eq!(desgloses[0], esperado);
    assert!(desgloses[0].messages > 0);
}

/// Dos respuestas: la primera llama a `noop` y consume 1 100 tokens; la segunda es texto.
fn dos_pasos_que_rebasan_el_limite() -> Vec<Vec<StreamChunk>> {
    vec![
        vec![
            StreamChunk::ToolCallComplete {
                id: "c1".into(),
                name: "noop".into(),
                input: serde_json::json!({}),
            },
            StreamChunk::Usage(Usage {
                input_tokens: 700,
                output_tokens: 400,
                reported_cost_usd: None,
            }),
            StreamChunk::Done(FinishReason::ToolCalls),
        ],
        respuesta_con_consumo("listo", 10, 1),
    ]
}

async fn peticiones_con_presupuesto(presupuesto: BudgetConfig) -> usize {
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(NamedTool("noop")));
    let proveedor = proveedor_de("", false, dos_pasos_que_rebasan_el_limite());
    let mut e = entorno_de_turno(proveedor, catalogo_de_ventana(100_000), &[], herramientas).await;
    e.agente = e.agente.with_budget(presupuesto);
    e.agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    let peticiones = e.proveedor.seen.lock().unwrap().len();
    peticiones
}

#[tokio::test]
async fn without_hard_stop_an_exceeded_budget_only_warns() {
    let peticiones = peticiones_con_presupuesto(presupuesto_de_tokens(1000)).await;
    assert_eq!(peticiones, 2, "sin hard_stop el turno sigue");
}

#[tokio::test]
async fn hard_stop_ends_the_turn_once_the_budget_is_exhausted() {
    let presupuesto = BudgetConfig {
        hard_stop: true,
        ..presupuesto_de_tokens(1000)
    };
    let peticiones = peticiones_con_presupuesto(presupuesto).await;
    assert_eq!(
        peticiones, 1,
        "agotado el presupuesto no se pide otra respuesta"
    );
}

/// Registra `child` como hijo de `parent` en el almacén, como hace el lanzador.
async fn registrar_hijo(store: &SqliteStore, parent: SessionId) -> SessionId {
    let child = store.create_session("/p").await.unwrap();
    store
        .save_subagent(&crate::session::SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child,
            job_id: "subagent-1".into(),
            label: "hijo".into(),
            status: crate::task::TaskStatus::new(crate::task::TaskState::Working),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        })
        .await
        .unwrap();
    child
}

#[tokio::test]
async fn a_child_spending_crosses_the_family_budget_once() {
    let proveedor = proveedor_de("", false, vec![respuesta_con_consumo("hijo", 100, 50)]);
    let mut e = entorno_de_turno(
        proveedor,
        catalogo_de_ventana(100_000),
        &[],
        ToolRegistry::new(),
    )
    .await;
    let hijo = registrar_hijo(&e.store, e.sid).await;
    // El padre ya lleva 700 de 1000. El hijo gasta 150: la familia llega a 850 y cruza el 80 %.
    e.store
        .record_usage(
            e.sid,
            &UsageRecord {
                session: e.sid,
                model: "m".into(),
                input_tokens: 700,
                output_tokens: 0,
                input_price: None,
                output_price: None,
                reported_cost_usd: None,
                source: crate::message::UsageSource::Provider,
                is_final: true,
                components: Vec::new(),
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                created_at_ms: 0,
            },
        )
        .await
        .unwrap();
    e.agente = e
        .agente
        .with_budget(presupuesto_de_tokens(1000))
        .with_budget_family(e.sid);
    let mut rx = e.agente.bus.subscribe();

    e.agente
        .run(hijo, "tarea".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(avisos_del_bus(&mut rx), vec![(80, 850.0)]);
}

#[tokio::test]
async fn the_warning_is_not_repeated_on_the_next_response() {
    let proveedor = proveedor_de(
        "",
        false,
        vec![
            respuesta_con_consumo("primera", 700, 100),
            respuesta_con_consumo("segunda", 200, 100),
        ],
    );
    let mut e = entorno_de_turno(
        proveedor,
        catalogo_de_ventana(100_000),
        &[],
        ToolRegistry::new(),
    )
    .await;
    e.agente = e.agente.with_budget(presupuesto_de_tokens(1000));
    let mut rx = e.agente.bus.subscribe();

    e.agente
        .run(e.sid, "uno".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(avisos_del_bus(&mut rx), vec![(80, 800.0)]);

    // 800 -> 1100: cruza el 100 %, pero el 80 % ya se dio y no vuelve a salir.
    e.agente
        .run(e.sid, "dos".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(avisos_del_bus(&mut rx), vec![(100, 1100.0)]);
}

#[tokio::test]
async fn no_budget_means_no_store_read_and_no_warning() {
    let dir = tempfile::tempdir().unwrap();
    let dentro = Arc::new(
        SqliteStore::open(dir.path().join("sin.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let almacen = Arc::new(AlmacenContador {
        dentro: dentro.clone(),
        lecturas_de_consumo: std::sync::atomic::AtomicUsize::new(0),
    });
    let sid = dentro.create_session("/p").await.unwrap();
    let proveedor = Arc::new(proveedor_de(
        "",
        false,
        vec![respuesta_con_consumo("hola", 5000, 500)],
    ));
    let cfg = CodeLoomConfig {
        provider: crate::config::ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "http://x".into(),
            api_key_env: "K".into(),
        },
        model: "m".into(),
        max_steps: 10,
        effort: None,
    };
    // Sin `with_budget`: el presupuesto es el vacío por defecto.
    let agente = AgentLoop::new(
        proveedor,
        almacen.clone(),
        Arc::new(ToolRegistry::new()),
        EventBus::new(64),
        Arc::new(PermissionService::allow_all()),
        cfg,
        dir.path().to_path_buf(),
    );
    let mut rx = agente.bus.subscribe();

    agente
        .run(sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(
        almacen
            .lecturas_de_consumo
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "sin presupuesto no se lee el consumo acumulado"
    );
    assert!(avisos_del_bus(&mut rx).is_empty());
    assert_eq!(
        dentro.session_usage(sid).await.unwrap().len(),
        1,
        "el apunte se guarda igual"
    );
}

#[tokio::test]
async fn the_turn_is_not_stopped_by_the_warning() {
    // El primer paso pide una herramienta y declara un consumo que supera el 100 %. El turno
    // debe seguir y el asistente responder en el segundo paso.
    let proveedor = proveedor_de(
        "",
        false,
        vec![
            vec![
                StreamChunk::TextDelta("voy a mirar".into()),
                StreamChunk::Usage(Usage {
                    input_tokens: 500,
                    output_tokens: 0,
                    reported_cost_usd: None,
                }),
                StreamChunk::ToolCallComplete {
                    id: "c1".into(),
                    name: "noop".into(),
                    input: serde_json::json!({}),
                },
                StreamChunk::Done(FinishReason::ToolCalls),
            ],
            respuesta_con_consumo("listo", 10, 5),
        ],
    );
    let mut herramientas = ToolRegistry::new();
    herramientas.register(Arc::new(Noop));
    let mut e = entorno_de_turno(proveedor, catalogo_de_ventana(100_000), &[], herramientas).await;
    e.agente = e.agente.with_budget(presupuesto_de_tokens(100));
    let mut rx = e.agente.bus.subscribe();

    let resultado = e
        .agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(resultado.final_text, "listo");
    // 500 sobre 100: cruza el 80 % y el 100 % en el primer paso.
    assert_eq!(avisos_del_bus(&mut rx), vec![(80, 500.0), (100, 500.0)]);
}
