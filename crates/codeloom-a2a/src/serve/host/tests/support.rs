//! Piezas comunes de los tests.

use super::*;

/// Responde siempre con el mismo texto.
pub(super) struct ConTexto(pub(super) String);

/// No responde nunca: la tarea se queda en `working` hasta que la cancelen.
pub(super) struct Colgado;

pub(super) fn capacidades() -> Capabilities {
    Capabilities {
        tool_use: true,
        reasoning: false,
        streaming: true,
    }
}

#[async_trait]
impl Provider for ConTexto {
    fn capabilities(&self) -> Capabilities {
        capacidades()
    }
    async fn stream(
        &self,
        _req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> CoreResult<ProviderStream> {
        let trozos = vec![
            StreamChunk::Started,
            StreamChunk::TextDelta(self.0.clone()),
            StreamChunk::Done(FinishReason::Stop),
        ];
        Ok(Box::pin(stream::iter(trozos)))
    }
}

#[async_trait]
impl Provider for Colgado {
    fn capabilities(&self) -> Capabilities {
        capacidades()
    }
    async fn stream(
        &self,
        _req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> CoreResult<ProviderStream> {
        Ok(Box::pin(stream::pending::<StreamChunk>()))
    }
}

pub(super) struct Montaje {
    pub(super) host: LauncherHost,
    pub(super) launcher: Arc<InProcessSubagentLauncher>,
    pub(super) otro_padre: SessionId,
    pub(super) bus: EventBus,
    pub(super) _dir: tempfile::TempDir,
}

pub(super) async fn montar(provider: Arc<dyn Provider>, limites: HostLimits) -> Montaje {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("host.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let config = CodeLoomConfig {
        provider: ProviderConfig {
            kind: ProviderKind::OpenAiCompat,
            base_url: "http://localhost".into(),
            api_key_env: "SIN_USO".into(),
        },
        model: "m".into(),
        max_steps: 3,
        effort: None,
    };
    let registro = Arc::new(AgentRegistry::new(vec![Box::new(BuiltInSource)]));
    let bus = EventBus::new(64);
    let launcher = Arc::new(
        InProcessSubagentLauncher::new(
            Arc::new(RwLock::new(provider)),
            store.clone(),
            Arc::new(served_tools()),
            Arc::new(PermissionService::allow_all()),
            Arc::new(RwLock::new(config)),
            dir.path().to_path_buf(),
            Arc::new(LocalJobRegistry::default()),
            bus.clone(),
        )
        .with_registry(registro),
    );
    let host = LauncherHost::new(
        launcher.clone(),
        store.as_ref(),
        "/servidor",
        "default".into(),
        limites,
    )
    .await
    .unwrap()
    .with_bus(bus.clone());
    let otro_padre = store.create_session("/otro").await.unwrap();
    Montaje {
        host,
        launcher,
        otro_padre,
        bus,
        _dir: dir,
    }
}

pub(super) fn limites() -> HostLimits {
    HostLimits {
        max_concurrent: 2,
        max_task: Duration::from_secs(60),
        max_result_bytes: 1024,
    }
}

pub(super) fn id_de(vista: &TaskView) -> String {
    vista.json["id"].as_str().unwrap().to_string()
}

/// Recoge un flujo entero (con un tope de tiempo para que una prueba rota no cuelgue).
pub(super) async fn recoger(mut flujo: EventStream) -> Vec<serde_json::Value> {
    let mut eventos = Vec::new();
    let limite = Instant::now() + Duration::from_secs(10);
    while Instant::now() < limite {
        match tokio::time::timeout(Duration::from_millis(500), flujo.next()).await {
            Ok(Some(evento)) => eventos.push(evento),
            Ok(None) => return eventos,
            Err(_) => {}
        }
    }
    panic!("el flujo no terminó: {eventos:?}");
}
