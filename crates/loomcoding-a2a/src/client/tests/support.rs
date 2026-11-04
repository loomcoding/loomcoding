//! Piezas comunes de los tests.

use super::*;

pub(super) const ENVIO: &str =
    include_str!("../../../tests/fixtures/jsonrpc-send-message.response.json");

pub(super) const NO_ENCONTRADA: &str =
    include_str!("../../../tests/fixtures/jsonrpc-error-task-not-found.json");

pub(super) fn http(segundos: u64) -> Client {
    http_client(&A2aConfig {
        role: A2aRole::Consume,
        request_timeout_secs: segundos,
        ..A2aConfig::default()
    })
    .unwrap()
    .unwrap()
}

pub(super) fn cliente(servidor: &MockServer, tenant: Option<&str>) -> JsonRpcClient {
    JsonRpcClient::new(
        http(2),
        format!("{}/a2a/v1", servidor.uri()),
        tenant.map(str::to_string),
        64 * 1024,
    )
}

/// Respuesta JSON-RPC con el `id` de la petición 1 (el primero que emite cada cliente).
pub(super) fn con_id_1(mut valor: Value) -> Value {
    valor["id"] = json!(1);
    valor
}

pub(super) async fn montar(servidor: &MockServer, metodo: &str, respuesta: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/a2a/v1"))
        .and(header("A2A-Version", "1.0"))
        .and(body_partial_json(
            json!({ "jsonrpc": "2.0", "method": metodo }),
        ))
        .respond_with(respuesta)
        .mount(servidor)
        .await;
}

pub(super) fn sobre(evento: Value) -> String {
    format!(
        "data: {}\n\n",
        json!({ "jsonrpc": "2.0", "id": 1, "result": evento })
    )
}

pub(super) fn flujo_sse(cuerpo: impl Into<String>) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(cuerpo.into().into_bytes(), "text/event-stream")
}

pub(super) fn tarea_wire(estado: &str) -> Value {
    json!({ "task": { "id": "t-1", "contextId": "c-1", "status": { "state": estado } } })
}

pub(super) fn estado_wire(estado: &str) -> Value {
    json!({ "statusUpdate": { "taskId": "t-1", "contextId": "c-1",
                              "status": { "state": estado } } })
}

pub(super) fn artefacto_wire(id: &str, texto: &str, append: bool) -> Value {
    json!({ "artifactUpdate": { "taskId": "t-1", "contextId": "c-1", "append": append,
            "artifact": { "artifactId": id, "parts": [{ "text": texto }] } } })
}

pub(super) async fn recoger(
    cliente: &JsonRpcClient,
) -> Vec<Result<loomcoding_core::remote_task::RemoteTask, RemoteTaskError>> {
    use futures::StreamExt;
    cliente.watch("t-1").await.unwrap().collect().await
}

pub(super) const SECRETO: &str = "s3cr3t0-muy-largo";

pub(super) fn tarjeta_segura(nombre: &str, esquema: Value) -> Value {
    json!({
        "securitySchemes": { nombre: esquema },
        "securityRequirements": [{ "schemes": { nombre: { "list": [] } } }],
    })
}

pub(super) fn plan(nombre: &str, esquema: Value, valor: &'static str) -> AuthPlan {
    let seguridad = CardSecurity::from_card_json(&tarjeta_segura(nombre, esquema));
    let credenciales = BTreeMap::from([(nombre.to_string(), "V".to_string())]);
    AuthPlan::choose(&seguridad, &credenciales, &|v| {
        (v == "V").then(|| valor.to_string())
    })
    .unwrap()
}

pub(super) async fn enviar_con(
    servidor: &MockServer,
    auth: AuthPlan,
) -> Result<RemoteReply, RemoteTaskError> {
    cliente(servidor, None)
        .with_auth(auth)
        .send(Message::user_text("hola"))
        .await
}

pub(super) fn respuesta_de_envio() -> ResponseTemplate {
    let valor: Value = serde_json::from_str(ENVIO).unwrap();
    ResponseTemplate::new(200).set_body_json(con_id_1(valor))
}

/// Contesta al envío copiando el `id` de la petición (varias llamadas seguidas llevan `id` distinto).
pub(super) struct EnvioConId;

impl wiremock::Respond for EnvioConId {
    fn respond(&self, peticion: &wiremock::Request) -> ResponseTemplate {
        let cuerpo: Value = serde_json::from_slice(&peticion.body).unwrap();
        let mut valor: Value = serde_json::from_str(ENVIO).unwrap();
        valor["id"] = cuerpo["id"].clone();
        ResponseTemplate::new(200).set_body_json(valor)
    }
}

pub(super) fn esquema_oauth(servidor: &MockServer) -> Value {
    json!({ "oauth2SecurityScheme": { "flows": { "clientCredentials": {
        "tokenUrl": format!("{}/token", servidor.uri()),
        "scopes": { "leer": "leer" }
    } } } })
}

pub(super) fn token(valor: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(json!({ "access_token": valor, "token_type": "Bearer", "expires_in": 3600 }))
}

pub(super) fn remoto_con_seguridad(servidor: &MockServer) -> RemoteSpec {
    let seguridad = CardSecurity::from_card_json(&tarjeta_segura(
        "claveApi",
        json!({ "apiKeySecurityScheme": { "location": "header", "name": "X-Api-Key" } }),
    ));
    RemoteSpec {
        endpoint: servidor.uri(),
        fetched: Some(RemoteFetched {
            card_hash: "h".into(),
            jsonrpc_url: format!("{}/a2a/v1", servidor.uri()),
            tenant: None,
            requires_auth: true,
            security: seguridad.to_json(),
            auth_problem: None,
            streaming: false,
        }),
        credentials: BTreeMap::from([("claveApi".to_string(), "AGENTE_KEY".to_string())]),
    }
}

pub(super) fn fabrica(valor: Option<&'static str>) -> A2aClientFactory {
    A2aClientFactory::from_config(&A2aConfig {
        role: A2aRole::Consume,
        ..A2aConfig::default()
    })
    .unwrap()
    .unwrap()
    .with_env(move |variable| {
        if variable == "AGENTE_KEY" {
            valor.map(str::to_string)
        } else {
            None
        }
    })
}

pub(super) fn remoto_mtls(servidor: &MockServer) -> RemoteSpec {
    let seguridad =
        CardSecurity::from_card_json(&tarjeta_segura("tls", json!({ "mtlsSecurityScheme": {} })));
    RemoteSpec {
        endpoint: servidor.uri(),
        fetched: Some(RemoteFetched {
            card_hash: "h".into(),
            jsonrpc_url: format!("{}/a2a/v1", servidor.uri()),
            tenant: None,
            requires_auth: true,
            security: seguridad.to_json(),
            auth_problem: None,
            streaming: false,
        }),
        credentials: BTreeMap::from([("tls".to_string(), "AGENTE_TLS".to_string())]),
    }
}

pub(super) fn fabrica_con_ruta(ruta: &'static str) -> A2aClientFactory {
    A2aClientFactory::from_config(&A2aConfig {
        role: A2aRole::Consume,
        ..A2aConfig::default()
    })
    .unwrap()
    .unwrap()
    .with_env(move |variable| (variable == "AGENTE_TLS").then(|| ruta.to_string()))
}
