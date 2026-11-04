use super::*;

/// Proveedor de prueba con un guion por cada paso de la conversación.
pub(super) struct QueueProvider {
    pub(super) scripts: std::sync::Mutex<std::collections::VecDeque<Vec<StreamChunk>>>,
}

#[async_trait]
impl Provider for QueueProvider {
    fn capabilities(&self) -> crate::provider::Capabilities {
        crate::provider::Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        _req: crate::provider::ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<crate::provider::ProviderStream> {
        let script = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
        Ok(Box::pin(futures::stream::iter(script)))
    }
}

/// Herramienta trivial que devuelve su entrada; sirve para observar sus eventos desde el padre.
pub(super) struct EchoTool;

#[async_trait]
impl crate::tools::Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "devuelve su entrada"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn run(
        &self,
        _ctx: crate::tools::ToolCtx,
        input: serde_json::Value,
        _cancel: CancellationToken,
    ) -> Result<crate::tools::ToolOutput> {
        Ok(crate::tools::ToolOutput::ok(input.to_string()))
    }
}

/// Lanzador con la herramienta `echo` y un guion de pasos para el hijo.
pub(super) fn echo_launcher(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    jobs: Arc<LocalJobRegistry>,
    bus: EventBus,
    script: Vec<Vec<StreamChunk>>,
) -> InProcessSubagentLauncher {
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(EchoTool));
    let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
        scripts: std::sync::Mutex::new(script.into()),
    });
    InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store,
        Arc::new(tools),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://localhost".into(),
                api_key_env: "KEY".into(),
            },
            model: "test".into(),
            max_steps: 4,
            effort: None,
        })),
        dir.to_path_buf(),
        jobs,
        bus,
    )
}

/// Lanzador cuyo hijo pregunta al padre y se queda esperando `input-required`.
pub(super) fn lanzador_que_pregunta(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    jobs: Arc<LocalJobRegistry>,
    bus: EventBus,
) -> InProcessSubagentLauncher {
    let provider: Arc<dyn Provider> = Arc::new(QueueProvider {
        scripts: std::sync::Mutex::new(
            vec![vec![
                StreamChunk::ToolCallComplete {
                    id: "c1".into(),
                    name: "ask_user".into(),
                    input: serde_json::json!({"questions": [{
                        "question": "¿Cuál?",
                        "options": [{"label": "Sí"}, {"label": "No"}]
                    }]}),
                },
                StreamChunk::Done(FinishReason::ToolCalls),
            ]]
            .into(),
        ),
    });
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(crate::tools::ask_user::AskUserTool));
    InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store,
        Arc::new(tools),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(LoomcodingConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://localhost".into(),
                api_key_env: "KEY".into(),
            },
            model: "test".into(),
            max_steps: 4,
            effort: None,
        })),
        dir.to_path_buf(),
        jobs,
        bus,
    )
}

/// Espera, con cota de tiempo, a que el almacén registre el estado pedido para el hijo.
pub(super) async fn espera_estado_del_hijo(
    store: &SqliteStore,
    parent: SessionId,
    child: SessionId,
    estado: TaskState,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let actual = store
                .list_subagents(parent)
                .await
                .unwrap()
                .into_iter()
                .find(|d| d.child == child)
                .map(|d| d.status.state);
            if actual == Some(estado) {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("el hijo debería quedar en {estado:?}"));
}

/// Responde a las preguntas con una opción fija; `None` simula que no hay nadie a quien preguntar.
pub(super) struct FixedAnswer(pub(super) Option<&'static str>);

#[async_trait]
impl crate::permission::PermissionResponder for FixedAnswer {
    async fn decide(
        &self,
        _request: &crate::permission::PermissionRequest,
    ) -> crate::permission::Decision {
        crate::permission::Decision::Allow
    }

    async fn ask(
        &self,
        questions: &[crate::question::Question],
    ) -> Option<Vec<crate::question::Answer>> {
        let choice = self.0?;
        Some(
            questions
                .iter()
                .map(|question| crate::question::Answer {
                    question: question.question.clone(),
                    selected: vec![choice.to_string()],
                })
                .collect(),
        )
    }
}

/// Petición grabada: modelo, sistema y esfuerzo.
pub(super) type PeticionGrabada = (String, String, Option<String>);

/// Proveedor que graba cada petición (modelo, sistema y esfuerzo). Si tiene puerta, la
/// respuesta espera a que se abra, para poder cambiar el registro con el hijo en curso.
pub(super) struct ProveedorGrabador {
    pub(super) peticiones: Arc<std::sync::Mutex<Vec<PeticionGrabada>>>,
    pub(super) puerta: Option<Arc<tokio::sync::Notify>>,
}

#[async_trait]
impl Provider for ProveedorGrabador {
    fn capabilities(&self) -> crate::provider::Capabilities {
        crate::provider::Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: crate::provider::ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<crate::provider::ProviderStream> {
        self.peticiones.lock().unwrap().push((
            req.model.clone(),
            req.system.clone(),
            req.effort.clone(),
        ));
        if let Some(puerta) = &self.puerta {
            puerta.notified().await;
        }
        Ok(Box::pin(futures::stream::iter(vec![
            StreamChunk::TextDelta("listo".into()),
            StreamChunk::Done(FinishReason::Stop),
        ])))
    }
}

/// Configuración del padre de las pruebas: modelo `test` y esfuerzo `low`.
pub(super) fn config_de_prueba() -> LoomcodingConfig {
    LoomcodingConfig {
        provider: ProviderConfig {
            kind: ProviderKind::OpenAiCompat,
            base_url: "http://localhost".into(),
            api_key_env: "KEY".into(),
        },
        model: "test".into(),
        max_steps: 4,
        effort: Some("low".into()),
    }
}

/// Lanzador de prueba con el proveedor grabador y, si se pasan, un registro y un catálogo.
pub(super) fn lanzador_grabador(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    jobs: Arc<LocalJobRegistry>,
    proveedor: ProveedorGrabador,
    registro: Option<Arc<AgentRegistry>>,
    catalogo: Option<Catalog>,
) -> InProcessSubagentLauncher {
    let provider: Arc<dyn Provider> = Arc::new(proveedor);
    let lanzador = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store,
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.to_path_buf(),
        jobs,
        EventBus::new(64),
    );
    let lanzador = match catalogo {
        Some(catalogo) => lanzador.with_catalog(Arc::new(RwLock::new(catalogo))),
        None => lanzador,
    };
    match registro {
        Some(registro) => lanzador.with_registry(registro),
        None => lanzador,
    }
}

/// Enrutador de prueba: contesta lo que se le diga.
pub(super) struct EnrutadorFijo(pub(super) Result<Option<RouteDecision>>);

#[async_trait]
impl crate::registry::router::AgentRouter for EnrutadorFijo {
    async fn route(
        &self,
        _request: &RouteRequest,
        _candidates: &[crate::registry::router::RouteCandidate],
    ) -> Result<Option<RouteDecision>> {
        match &self.0 {
            Ok(decision) => Ok(decision.clone()),
            Err(_) => Err(CoreError::Tool("el enrutador falló".into())),
        }
    }
}

/// Lanzador con registro (integrado + `revisor` global) y el enrutador dado.
pub(super) fn lanzador_con_enrutador(
    dir: &std::path::Path,
    store: Arc<SqliteStore>,
    enrutador: Option<(Result<Option<RouteDecision>>, ConfirmDelegation)>,
) -> InProcessSubagentLauncher {
    let global = dir.join("global");
    std::fs::create_dir_all(&global).unwrap();
    escribir_perfil(
        &global,
        "revisor",
        "---\nname: revisor\ndescription: Revisa\n---\n\nCuerpo.\n",
    );
    let proveedor = ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    };
    let lanzador = lanzador_grabador(
        dir,
        store,
        Arc::new(LocalJobRegistry::default()),
        proveedor,
        Some(registro_global(&global)),
        None,
    );
    match enrutador {
        Some((respuesta, modo)) => lanzador.with_router(Arc::new(EnrutadorFijo(respuesta)), modo),
        None => lanzador,
    }
}

pub(super) async fn tienda_de_prueba(
    dir: &std::path::Path,
    nombre: &str,
) -> (Arc<SqliteStore>, SessionId) {
    let store = Arc::new(
        SqliteStore::open(dir.join(nombre).to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    (store, parent)
}

/// Registro con el integrado y una carpeta global de perfiles.
pub(super) fn registro_global(global: &std::path::Path) -> Arc<AgentRegistry> {
    Arc::new(AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(global.to_path_buf(), ProfileLevel::Global)),
    ]))
}

pub(super) fn escribir_perfil(dir: &std::path::Path, nombre: &str, texto: &str) {
    std::fs::write(dir.join(format!("{nombre}.md")), texto).unwrap();
}

pub(super) const PERFIL_CON_PERSONA: &str = "---\nname: frontend\ndescription: UI\npersona:\n  role: Ingeniero\nmodel: modelo-perfil\neffort: high\n---\n\nCuerpo del agente.\n";

/// Fuente de prueba con un agente remoto. Nada lo produce todavía (A8).
pub(super) struct FuenteRemotaDePrueba;

impl AgentSource for FuenteRemotaDePrueba {
    fn id(&self) -> &str {
        "remota-de-prueba"
    }
    fn load(&self) -> SourceLoad {
        SourceLoad {
            entries: vec![AgentEntry {
                name: "remoto".into(),
                origin: AgentOrigin::Remote {
                    card_url: "https://agente.example/card".into(),
                },
                card: crate::card::AgentCard::local("remoto", "Agente remoto de prueba", "1.0.0"),
                execution: Execution::Remote(crate::registry::RemoteSpec {
                    endpoint: "https://agente.example".into(),
                    fetched: None,
                    credentials: Default::default(),
                }),
                content_hash: None,
            }],
            problems: Vec::new(),
        }
    }
}

/// Hijo cuyo trabajo ignora la orden de parar: nunca termina. Sirve para ver qué hace `cancel`
/// cuando el éxito no llega.
pub(super) async fn hijo_que_no_se_detiene(
    store: &Arc<SqliteStore>,
    jobs: &Arc<LocalJobRegistry>,
    parent: SessionId,
) -> SessionId {
    let child = store.create_session("/project").await.unwrap();
    let job = jobs
        .start(JobSpec {
            kind: "subagent".into(),
            label: "terco".into(),
            owner: parent,
            output_limit: 1024,
            run: Box::new(|_handle, _cancel| {
                Box::pin(async move {
                    std::future::pending::<()>().await;
                    Ok(None)
                })
            }),
        })
        .unwrap();
    store
        .save_subagent(&SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child,
            job_id: job.id().0.clone(),
            label: "terco".into(),
            status: TaskStatus::new(TaskState::Working),
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

/// Cómo responde `watch` el cliente guionizado.
pub(super) enum GuionFlujo {
    /// Un flujo que entrega estos elementos y se cierra.
    Elementos(Vec<std::result::Result<RemoteTask, RemoteTaskError>>),
    /// No se puede abrir.
    Falla(RemoteTaskError),
    /// Un flujo que se abre y no dice nunca nada.
    Pendiente,
}

/// Cliente remoto guionizado: contesta lo que se le diga y anota lo que recibe.
pub(super) struct ClienteGuionizado {
    /// `None`: el remoto no ofrece flujo (`watch` falla con «no soportado»).
    pub(super) flujo: std::sync::Mutex<Option<GuionFlujo>>,
    /// Veces que se ha sondeado con `get`.
    pub(super) sondeos: std::sync::Mutex<usize>,
    pub(super) envio: std::sync::Mutex<Option<std::result::Result<RemoteReply, RemoteTaskError>>>,
    /// Respuestas a los envíos posteriores al primero (las respuestas a una pregunta).
    pub(super) respuestas:
        std::sync::Mutex<VecDeque<std::result::Result<RemoteReply, RemoteTaskError>>>,
    pub(super) consultas:
        std::sync::Mutex<VecDeque<std::result::Result<RemoteTask, RemoteTaskError>>>,
    /// Respuesta cuando la cola de consultas se agota.
    pub(super) consulta_final: std::sync::Mutex<std::result::Result<RemoteTask, RemoteTaskError>>,
    pub(super) cancelacion: std::sync::Mutex<std::result::Result<RemoteTask, RemoteTaskError>>,
    pub(super) enviados: std::sync::Mutex<Vec<Message>>,
    pub(super) canceladas: std::sync::Mutex<Vec<String>>,
}

pub(super) fn tarea(
    id: &str,
    estado: TaskState,
    mensaje: Option<&str>,
    resultado: &[&str],
) -> RemoteTask {
    RemoteTask {
        id: id.to_string(),
        context_id: None,
        status: TaskStatus {
            message: mensaje.map(Message::agent_text),
            ..TaskStatus::new(estado)
        },
        artifacts: resultado
            .iter()
            .map(|t| Artifact::text_artifact(*t))
            .collect(),
    }
}

impl ClienteGuionizado {
    pub(super) fn nuevo(
        envio: std::result::Result<RemoteReply, RemoteTaskError>,
        consultas: Vec<std::result::Result<RemoteTask, RemoteTaskError>>,
    ) -> Arc<Self> {
        Arc::new(ClienteGuionizado {
            flujo: std::sync::Mutex::new(None),
            sondeos: std::sync::Mutex::new(0),
            envio: std::sync::Mutex::new(Some(envio)),
            respuestas: std::sync::Mutex::new(VecDeque::new()),
            consultas: std::sync::Mutex::new(consultas.into()),
            consulta_final: std::sync::Mutex::new(Ok(tarea("r1", TaskState::Working, None, &[]))),
            cancelacion: std::sync::Mutex::new(Ok(tarea("r1", TaskState::Canceled, None, &[]))),
            enviados: std::sync::Mutex::new(Vec::new()),
            canceladas: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub(super) fn trabajando() -> Arc<Self> {
        Self::nuevo(
            Ok(RemoteReply::Task(tarea(
                "r1",
                TaskState::Working,
                None,
                &[],
            ))),
            vec![],
        )
    }
}

#[async_trait]
impl RemoteTaskClient for ClienteGuionizado {
    async fn send(&self, message: Message) -> std::result::Result<RemoteReply, RemoteTaskError> {
        self.enviados.lock().unwrap().push(message);
        if let Some(primero) = self.envio.lock().unwrap().take() {
            return primero;
        }
        self.respuestas
            .lock()
            .unwrap()
            .pop_front()
            .expect("envío no previsto")
    }
    async fn watch(
        &self,
        _id: &str,
    ) -> std::result::Result<
        futures::stream::BoxStream<'static, std::result::Result<RemoteTask, RemoteTaskError>>,
        RemoteTaskError,
    > {
        use futures::StreamExt;
        match self.flujo.lock().unwrap().take() {
            None => Err(RemoteTaskError::Unsupported("sin flujo".into())),
            Some(GuionFlujo::Falla(error)) => Err(error),
            Some(GuionFlujo::Elementos(elementos)) => Ok(futures::stream::iter(elementos).boxed()),
            Some(GuionFlujo::Pendiente) => Ok(futures::stream::pending::<
                std::result::Result<RemoteTask, RemoteTaskError>,
            >()
            .boxed()),
        }
    }
    async fn get(&self, _id: &str) -> std::result::Result<RemoteTask, RemoteTaskError> {
        *self.sondeos.lock().unwrap() += 1;
        match self.consultas.lock().unwrap().pop_front() {
            Some(respuesta) => respuesta,
            None => self.consulta_final.lock().unwrap().clone(),
        }
    }
    async fn cancel(&self, id: &str) -> std::result::Result<RemoteTask, RemoteTaskError> {
        self.canceladas.lock().unwrap().push(id.to_string());
        self.cancelacion.lock().unwrap().clone()
    }
}

/// Fábrica de prueba: entrega siempre el mismo cliente; con motivo, dice que el remoto no se
/// puede usar (como haría la real si faltase una credencial).
pub(super) struct FabricaFija(pub(super) Arc<ClienteGuionizado>, Option<String>);

impl RemoteClientFactory for FabricaFija {
    fn client_for(
        &self,
        _name: &str,
        _spec: &crate::registry::RemoteSpec,
    ) -> Option<Arc<dyn RemoteTaskClient>> {
        Some(self.0.clone())
    }

    fn unavailable_reason(
        &self,
        _name: &str,
        _spec: &crate::registry::RemoteSpec,
    ) -> Option<String> {
        self.1.clone()
    }
}

pub(super) struct Entorno {
    pub(super) lanzador: InProcessSubagentLauncher,
    pub(super) store: Arc<SqliteStore>,
    pub(super) parent: SessionId,
    pub(super) bus: EventBus,
    pub(super) cliente: Arc<ClienteGuionizado>,
    // Mantiene vivos los directorios temporales.
    pub(super) _dirs: (tempfile::TempDir, tempfile::TempDir),
}

pub(super) async fn entorno_remoto(
    cliente: Arc<ClienteGuionizado>,
    aprobar: bool,
    requires_auth: bool,
    max_follow: Duration,
) -> Entorno {
    let dir = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "remoto.db").await;
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FuenteRemotaDePrueba),
    ])
    .with_trust(
        TrustStore::open(datos.path().to_path_buf()).unwrap(),
        datos.path().to_path_buf(),
        false,
    );
    // Igual que en producción: la tarjeta llega después de cargar el perfil, y solo entonces
    // se puede aprobar.
    registro.set_remote_card(
        &EventBus::new(8),
        "remoto",
        "https://agente.example",
        Ok((
            crate::card::AgentCard::local("remoto", "Agente remoto", "1.0.0"),
            RemoteFetched {
                card_hash: "abc".into(),
                jsonrpc_url: "https://agente.example/a2a/v1".into(),
                tenant: None,
                requires_auth,
                security: None,
                auth_problem: None,
                streaming: false,
            },
        )),
    );
    if aprobar {
        registro.approve("remoto").unwrap();
    }
    let bus = EventBus::new(256);
    let provider: Arc<dyn Provider> = Arc::new(ProveedorGrabador {
        peticiones: Arc::new(std::sync::Mutex::new(Vec::new())),
        puerta: None,
    });
    let lanzador = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        Arc::new(LocalJobRegistry::default()),
        bus.clone(),
    )
    .with_registry(Arc::new(registro))
    .with_remote_clients(Arc::new(FabricaFija(
        cliente.clone(),
        requires_auth.then(|| "falta la variable de entorno AGENTE_KEY".to_string()),
    )))
    .with_remote_timing(Duration::from_millis(5), max_follow);
    Entorno {
        lanzador,
        store,
        parent,
        bus,
        cliente,
        _dirs: (dir, datos),
    }
}

impl Entorno {
    pub(super) async fn lanzar(&self) -> SessionId {
        self.lanzador
            .start(
                self.parent,
                "lejano".into(),
                "haz lo tuyo".into(),
                Some("remoto".into()),
                CancellationToken::new(),
            )
            .await
            .unwrap()
            .0
    }

    pub(super) async fn descriptor(&self, child: SessionId) -> SubagentDescriptor {
        self.store
            .list_subagents(self.parent)
            .await
            .unwrap()
            .into_iter()
            .find(|d| d.child == child)
            .unwrap()
    }

    /// Espera a que el aviso de cierre llegue al bus y devuelve el estado que lleva.
    pub(super) async fn estado_del_aviso(
        &self,
        rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
    ) -> TaskState {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(LiveEvent::SubagentSettled { status, .. }) = rx.recv().await {
                    return status;
                }
            }
        })
        .await
        .expect("debería llegar el aviso de cierre")
    }
}

/// Espera a que el hijo tenga una pregunta pendiente, la contesta y devuelve su texto.
pub(super) async fn responder(entorno: &Entorno, hijo: SessionId, texto: &str) -> String {
    let pregunta = esperar_pregunta(entorno, hijo).await;
    entorno
        .lanzador
        .answer(entorno.parent, hijo, vec![texto.to_string()])
        .await
        .unwrap();
    pregunta
}

pub(super) async fn esperar_pregunta(entorno: &Entorno, hijo: SessionId) -> String {
    let limite = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(pendiente) = entorno
            .lanzador
            .pending_inputs(entorno.parent)
            .into_iter()
            .find(|p| p.child == hijo)
        {
            return pendiente.questions[0].question.clone();
        }
        assert!(
            std::time::Instant::now() < limite,
            "no llegó ninguna pregunta"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

pub(super) fn con_flujo(cliente: &Arc<ClienteGuionizado>, guion: GuionFlujo) {
    *cliente.flujo.lock().unwrap() = Some(guion);
}
