//! Piezas comunes de los tests.

use super::*;

pub(super) const SECRETO: &str = "s3cr3to-de-prueba";

/// Anfitrión falso: guarda lo que se le pide y contesta según un guion.
#[derive(Default)]
pub(super) struct Falso {
    pub(super) iniciadas: Mutex<Vec<String>>,
    pub(super) esperas: Mutex<usize>,
    pub(super) ocupado: bool,
    pub(super) termina_al_instante: bool,
    pub(super) con_flujo: bool,
}

pub(super) fn flujo_falso() -> EventStream {
    futures::stream::iter(vec![
        json!({ "task": { "id": "t-1", "status": { "state": "TASK_STATE_WORKING" } } }),
        json!({ "statusUpdate": { "taskId": "t-1", "contextId": "c",
                "status": { "state": "TASK_STATE_COMPLETED" } } }),
    ])
    .boxed()
}

pub(super) fn tarea(estado: &str, terminal: bool) -> TaskView {
    TaskView {
        json: json!({ "id": "t-1", "status": { "state": estado } }),
        terminal,
    }
}

#[async_trait]
impl TaskHost for Falso {
    async fn start(&self, prompt: String) -> Result<TaskView, HostError> {
        if self.ocupado {
            return Err(HostError::Busy);
        }
        self.iniciadas.lock().unwrap().push(prompt);
        Ok(if self.termina_al_instante {
            tarea("TASK_STATE_COMPLETED", true)
        } else {
            tarea("TASK_STATE_WORKING", false)
        })
    }
    async fn get(&self, id: &str) -> Result<TaskView, HostError> {
        if id == "t-1" {
            Ok(tarea("TASK_STATE_WORKING", false))
        } else {
            Err(HostError::NotFound)
        }
    }
    async fn cancel(&self, id: &str) -> Result<TaskView, HostError> {
        match id {
            "t-1" => Ok(tarea("TASK_STATE_CANCELED", true)),
            "terminada" => Err(HostError::NotCancelable),
            _ => Err(HostError::NotFound),
        }
    }
    async fn wait_terminal(&self, _id: &str, _max: Duration) -> Result<TaskView, HostError> {
        *self.esperas.lock().unwrap() += 1;
        Ok(tarea("TASK_STATE_COMPLETED", true))
    }
    async fn subscribe(&self, id: &str) -> Result<EventStream, HostError> {
        match (self.con_flujo, id) {
            (true, "t-1") => Ok(flujo_falso()),
            (true, "terminada") => Err(HostError::Unsupported),
            (true, _) => Err(HostError::NotFound),
            (false, _) => Err(HostError::Unsupported),
        }
    }
    async fn start_streaming(&self, prompt: String) -> Result<EventStream, HostError> {
        if self.ocupado {
            return Err(HostError::Busy);
        }
        self.iniciadas.lock().unwrap().push(prompt);
        if self.con_flujo {
            Ok(flujo_falso())
        } else {
            Err(HostError::Unsupported)
        }
    }
}

pub(super) fn estado(host: Falso, esquema: ServeAuthKind) -> (Arc<ServeState>, Arc<Falso>) {
    let host = Arc::new(host);
    let config = ServeConfig {
        auth: Some(ServeAuthConfig {
            scheme: esquema,
            name: Some("X-Api-Key".into()),
            env: "S".into(),
        }),
        ..ServeConfig::default()
    };
    let resuelta = config.validate(&|_| Some(SECRETO.to_string())).unwrap();
    let tarjeta = crate::serve::card::served_card_for_tenant(
        &AgentCard::local("default", "Agente de Loomcoding", "1.0.0"),
        "http://127.0.0.1:7077/a2a",
        None,
        &resuelta.scheme,
    );
    let estado = Arc::new(ServeState {
        host: host.clone(),
        auth: Authenticator::new(resuelta),
        card: tarjeta,
        expected_tenant: None,
        agent_cards: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        max_wait: Duration::from_secs(5),
    });
    (estado, host)
}

pub(super) fn bearer(host: Falso) -> (Arc<ServeState>, Arc<Falso>) {
    estado(host, ServeAuthKind::Bearer)
}

pub(super) fn rpc_peticion(cuerpo: &str, auth: Option<&str>, version: Option<&str>) -> Request {
    let mut constructor = Request::builder().method("POST").uri("/a2a");
    if let Some(auth) = auth {
        constructor = constructor.header("authorization", auth);
    }
    if let Some(version) = version {
        constructor = constructor.header("a2a-version", version);
    }
    constructor.body(Body::from(cuerpo.to_string())).unwrap()
}

pub(super) fn buena(cuerpo: Value) -> Request {
    rpc_peticion(
        &cuerpo.to_string(),
        Some(&format!("Bearer {SECRETO}")),
        Some("1.0"),
    )
}

pub(super) async fn enviar_req(
    estado: &Arc<ServeState>,
    req: Request,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let respuesta = router(estado.clone()).oneshot(req).await.unwrap();
    let estado_http = respuesta.status();
    let cabeceras = respuesta.headers().clone();
    let bytes = to_bytes(respuesta.into_body(), usize::MAX).await.unwrap();
    (estado_http, cabeceras, bytes.to_vec())
}

pub(super) async fn llamar(estado: &Arc<ServeState>, cuerpo: Value) -> (StatusCode, Value) {
    let (http, _, bytes) = enviar_req(estado, buena(cuerpo)).await;
    (http, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

pub(super) fn mensaje(texto: &str) -> Value {
    json!({ "messageId": "m1", "role": "ROLE_USER", "parts": [{ "text": texto }] })
}

pub(super) fn envio(texto: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "SendMessage",
            "params": { "message": mensaje(texto) } })
}

pub(super) fn flujo_peticion(metodo: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 7, "method": metodo, "params": params })
}

/// Los objetos `result` de cada línea `data:` de un cuerpo SSE.
pub(super) fn eventos_sse(cuerpo: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(cuerpo)
        .lines()
        .filter_map(|linea| linea.strip_prefix("data:"))
        .map(|json| serde_json::from_str::<Value>(json.trim()).unwrap())
        .collect()
}
