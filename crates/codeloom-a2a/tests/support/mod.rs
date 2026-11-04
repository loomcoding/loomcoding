//! Arnés común de las pruebas de extremo a extremo con un agente remoto: perfil `.md` → tarjeta
//! descargada → aprobación → lanzador → espejo → servidor simulado con `wiremock`.
//!
//! Cabe señalar que no hay red real: el «remoto» es un servidor de bucle local que habla JSON-RPC
//! de A2A y devuelve a cada petición el `id` que trae.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use codeloom_a2a::client::A2aClientFactory;
use codeloom_a2a::config::{A2aConfig, A2aRole};
use codeloom_a2a::remote::http_client;
use codeloom_a2a::sync::refresh_remote_cards_with_env;
use codeloom_core::config::{CodeLoomConfig, ProviderConfig, ProviderKind};
use codeloom_core::error::Result as CoreResult;
use codeloom_core::event::EventBus;
use codeloom_core::job::LocalJobRegistry;
use codeloom_core::permission::PermissionService;
use codeloom_core::provider::{Capabilities, Provider, ProviderRequest, ProviderStream};
use codeloom_core::registry::{AgentRegistry, BuiltInSource, FileSource, ProfileLevel, TrustStore};
use codeloom_core::session::sqlite::SqliteStore;
use codeloom_core::session::{SessionId, SessionStore};
use codeloom_core::subagent::InProcessSubagentLauncher;
use codeloom_core::task::TaskState;
use codeloom_core::tools::ToolRegistry;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

pub const TARJETA: &str = include_str!("../fixtures/agent-card.sample.json");

/// Credencial de los remotos protegidos de estas pruebas.
pub const SECRETO: &str = "s3cr3t0-de-prueba-muy-largo";

/// El proveedor no se usa: el agente es remoto.
pub struct ProveedorInerte;

#[async_trait]
impl Provider for ProveedorInerte {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: false,
            reasoning: false,
            streaming: false,
        }
    }
    async fn stream(
        &self,
        _req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> CoreResult<ProviderStream> {
        unreachable!("un agente remoto no llama al proveedor local")
    }
}

/// Remoto simulado: contesta a cada método según un guion y devuelve el `id` de la petición.
pub struct RemotoSimulado {
    pub consultas: AtomicUsize,
    /// A partir de esta consulta (1-based) la tarea figura como completada; 0 = nunca.
    pub completa_en: usize,
    /// A partir de esta consulta (1-based) `GetTask` no contesta (se queda colgada); 0 = nunca.
    pub cuelga_desde: usize,
}

impl Respond for RemotoSimulado {
    fn respond(&self, peticion: &Request) -> ResponseTemplate {
        let cuerpo: Value = serde_json::from_slice(&peticion.body).unwrap();
        let id = cuerpo["id"].clone();
        let resultado = match cuerpo["method"].as_str().unwrap() {
            "SendMessage" => json!({"task": {"id": "t-remota", "contextId": "c",
                "status": {"state": "TASK_STATE_WORKING"}}}),
            "GetTask" => {
                let n = self.consultas.fetch_add(1, Ordering::SeqCst) + 1;
                if self.cuelga_desde != 0 && n >= self.cuelga_desde {
                    // El remoto acepta la conexión pero no contesta: el cliente agota su plazo.
                    return ResponseTemplate::new(200).set_delay(Duration::from_secs(30));
                }
                if self.completa_en != 0 && n >= self.completa_en {
                    json!({"id": "t-remota", "status": {"state": "TASK_STATE_COMPLETED"},
                        "artifacts": [{"artifactId": "a", "parts": [{"text": "hecho en remoto"}]}]})
                } else {
                    json!({"id": "t-remota", "status": {"state": "TASK_STATE_WORKING"}})
                }
            }
            "CancelTask" => {
                json!({"id": "t-remota", "status": {"state": "TASK_STATE_CANCELED"}})
            }
            // La tarjeta de ejemplo declara streaming, pero este remoto simulado no lo ofrece: el
            // cliente debe caer al sondeo (A8c).
            "SubscribeToTask" => {
                return ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32004, "message": "streaming no soportado"}}));
            }
            otro => panic!("método inesperado: {otro}"),
        };
        ResponseTemplate::new(200)
            .set_body_json(json!({"jsonrpc": "2.0", "id": id, "result": resultado}))
    }
}

pub async fn servidor(completa_en: usize, cuelga_desde: usize, protegido: bool) -> MockServer {
    let servidor = MockServer::start().await;
    let mut tarjeta: Value = serde_json::from_str(TARJETA).unwrap();
    for interfaz in tarjeta["supportedInterfaces"].as_array_mut().unwrap() {
        interfaz["url"] = json!(format!("{}/a2a/v1", servidor.uri()));
    }
    // La tarjeta de ejemplo pide OpenID Connect, que no está soportado: o se quita la seguridad
    // (remoto abierto) o se sustituye por un `Bearer`, que exige la cabecera en cada llamada.
    let objeto = tarjeta.as_object_mut().unwrap();
    objeto.remove("securityRequirements");
    objeto.remove("securitySchemes");
    if protegido {
        objeto.insert(
            "securitySchemes".into(),
            json!({ "token": { "httpAuthSecurityScheme": { "scheme": "Bearer" } } }),
        );
        objeto.insert(
            "securityRequirements".into(),
            json!([{ "schemes": { "token": { "list": [] } } }]),
        );
    }
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tarjeta))
        .mount(&servidor)
        .await;
    let rpc = Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("A2A-Version", "1.0"));
    // Un remoto protegido no contesta a quien no lleve su credencial: ni siquiera con un 401.
    let rpc = if protegido {
        rpc.and(header(
            "authorization",
            format!("Bearer {SECRETO}").as_str(),
        ))
    } else {
        rpc
    };
    rpc.respond_with(RemotoSimulado {
        consultas: AtomicUsize::new(0),
        completa_en,
        cuelga_desde,
    })
    .mount(&servidor)
    .await;
    servidor
}

pub struct Montaje {
    pub lanzador: Arc<InProcessSubagentLauncher>,
    pub registro: Arc<AgentRegistry>,
    pub store: Arc<SqliteStore>,
    pub parent: SessionId,
    pub bus: EventBus,
    pub servidor: MockServer,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

pub async fn montar(completa_en: usize) -> Montaje {
    montar_con(completa_en, false, None).await
}

/// Remoto cuya consulta número `cuelga_desde` (y las siguientes) no contesta a tiempo.
pub async fn montar_colgado(cuelga_desde: usize) -> Montaje {
    montar_completo(0, cuelga_desde, false, None).await
}

/// `protegido`: el remoto exige `Bearer`; el perfil declara la variable `AGENTE_KEY`, cuyo valor
/// en el «entorno» de la prueba es `valor_env`.
pub async fn montar_con(
    completa_en: usize,
    protegido: bool,
    valor_env: Option<&'static str>,
) -> Montaje {
    montar_completo(completa_en, 0, protegido, valor_env).await
}

async fn montar_completo(
    completa_en: usize,
    cuelga_desde: usize,
    protegido: bool,
    valor_env: Option<&'static str>,
) -> Montaje {
    let servidor = servidor(completa_en, cuelga_desde, protegido).await;
    let dir = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let credenciales = if protegido {
        "  credentials:\n    token: AGENTE_KEY\n"
    } else {
        ""
    };
    std::fs::write(
        dir.path().join("lejano.md"),
        format!(
            "---\nname: lejano\ndescription: Alta manual\ntransport: a2a-remote\na2a:\n  endpoint: {}\n{credenciales}---\n",
            servidor.uri()
        ),
    )
    .unwrap();
    let entorno = move |variable: &str| {
        if variable == "AGENTE_KEY" {
            valor_env.map(str::to_string)
        } else {
            None
        }
    };
    let registro = Arc::new(
        AgentRegistry::new(vec![
            Box::new(BuiltInSource),
            Box::new(FileSource::new(
                dir.path().to_path_buf(),
                ProfileLevel::Global,
            )),
        ])
        .with_trust(
            TrustStore::open(datos.path().to_path_buf()).unwrap(),
            dir.path().to_path_buf(),
            false,
        ),
    );
    let config_a2a = A2aConfig {
        role: A2aRole::Consume,
        request_timeout_secs: 2,
        ..A2aConfig::default()
    };
    let bus = EventBus::new(256);
    let cliente = http_client(&config_a2a).unwrap().unwrap();
    refresh_remote_cards_with_env(&registro, &bus, &cliente, &config_a2a, &entorno).await;

    let store = Arc::new(
        SqliteStore::open(dir.path().join("e2e.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let provider: Arc<dyn Provider> = Arc::new(ProveedorInerte);
    let config = CodeLoomConfig {
        provider: ProviderConfig {
            kind: ProviderKind::OpenAiCompat,
            base_url: "http://localhost".into(),
            api_key_env: "SIN_USO".into(),
        },
        model: "m".into(),
        max_steps: 5,
        effort: None,
    };
    let fabrica = A2aClientFactory::from_config(&config_a2a)
        .unwrap()
        .unwrap()
        .with_env(entorno);
    let lanzador = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config)),
        dir.path().to_path_buf(),
        Arc::new(LocalJobRegistry::default()),
        bus.clone(),
    )
    .with_registry(registro.clone())
    .with_remote_clients(Arc::new(fabrica))
    .with_remote_timing(Duration::from_millis(10), Duration::from_secs(30));
    Montaje {
        lanzador: Arc::new(lanzador),
        registro,
        store,
        parent,
        bus,
        servidor,
        _dirs: (dir, datos),
    }
}

pub async fn espera(montaje: &Montaje, hijo: SessionId, estado: TaskState) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let actual = montaje
                .store
                .list_subagents(montaje.parent)
                .await
                .unwrap()
                .into_iter()
                .find(|d| d.child == hijo)
                .map(|d| d.status.state);
            if actual == Some(estado) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("el hijo debería llegar a {estado:?}"));
}

pub async fn metodos_recibidos(servidor: &MockServer) -> Vec<String> {
    servidor
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .map(|r| {
            let cuerpo: Value = serde_json::from_slice(&r.body).unwrap();
            cuerpo["method"].as_str().unwrap().to_string()
        })
        .collect()
}
