//! Métodos JSON-RPC: despacho, envío, consulta y suscripción.

use super::*;

pub(super) async fn rpc(State(estado): State<Arc<ServeState>>, peticion: Request) -> Response {
    let (partes, cuerpo) = peticion.into_parts();

    // 1. Autenticación, antes de leer nada.
    let autenticada = estado.auth.check(&|nombre| {
        partes
            .headers
            .get(nombre)
            .and_then(|valor| valor.to_str().ok())
            .map(str::to_string)
    });
    if !autenticada {
        return no_autorizado(estado.auth.scheme());
    }

    // 2. Tope del cuerpo, sin leerlo entero si ya se sabe que pasa.
    let declarado = partes
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.parse::<usize>().ok());
    if declarado.is_some_and(|largo| largo > MAX_BODY_BYTES) {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let bytes = match to_bytes(Body::new(cuerpo), MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };

    // 3. JSON-RPC.
    let Ok(sobre) = serde_json::from_slice::<Value>(&bytes) else {
        return error_rpc(Value::Null, PARSE_ERROR, "parse error");
    };
    let id = match sobre.get("id") {
        Some(id @ (Value::String(_) | Value::Number(_))) => id.clone(),
        _ => Value::Null,
    };
    let metodo = sobre.get("method").and_then(Value::as_str);
    if sobre.get("jsonrpc") != Some(&json!("2.0")) || id.is_null() || metodo.is_none() {
        return error_rpc(Value::Null, INVALID_REQUEST, "invalid request");
    }
    let metodo = metodo.unwrap_or_default();

    // 4. Versión de A2A: solo 1.0, sin degradar (§3.6).
    let version = partes
        .headers
        .get("a2a-version")
        .and_then(|valor| valor.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if version != VERSION_SOPORTADA {
        return error_rpc(
            id,
            VERSION_NOT_SUPPORTED,
            "A2A version not supported: this server speaks 1.0",
        );
    }

    let params = sobre.get("params").cloned().unwrap_or(Value::Null);
    let tenant = match params.get("tenant") {
        Some(Value::String(tenant)) => Some(tenant.as_str()),
        Some(_) => return error_rpc(id, INVALID_PARAMS, "params.tenant must be a string"),
        None => None,
    };
    if estado
        .expected_tenant
        .as_deref()
        .is_some_and(|esperado| tenant != Some(esperado))
    {
        return error_rpc(
            id,
            INVALID_PARAMS,
            "params.tenant does not match the selected AgentInterface",
        );
    }
    match metodo {
        "SendMessage" => enviar(&estado, id, &params, tenant, false).await,
        "SendStreamingMessage" => enviar(&estado, id, &params, tenant, true).await,
        "SubscribeToTask" => suscribir(&estado, id, &params, tenant).await,
        "GetTask" => consultar(&estado, id, &params, tenant, false).await,
        "CancelTask" => consultar(&estado, id, &params, tenant, true).await,
        "ListTasks" | "GetExtendedAgentCard" => {
            error_rpc(id, UNSUPPORTED_OPERATION, "operation not supported")
        }
        "CreateTaskPushNotificationConfig"
        | "GetTaskPushNotificationConfig"
        | "ListTaskPushNotificationConfigs"
        | "DeleteTaskPushNotificationConfig" => error_rpc(
            id,
            PUSH_NOT_SUPPORTED,
            "push notifications are not supported",
        ),
        _ => error_rpc(id, METHOD_NOT_FOUND, "method not found"),
    }
}

pub(super) async fn suscribir(
    estado: &ServeState,
    id: Value,
    params: &Value,
    tenant: Option<&str>,
) -> Response {
    let tarea = params
        .get("id")
        .and_then(Value::as_str)
        .filter(|tarea| !tarea.is_empty() && tarea.len() <= MAX_ID_BYTES);
    let Some(tarea) = tarea else {
        return error_rpc(id, INVALID_PARAMS, "params.id must be a task id");
    };
    match estado.host.subscribe_for(tenant, tarea).await {
        Ok(eventos) => respuesta_sse(id, eventos),
        Err(error) => desde_anfitrion(id, error),
    }
}

pub(super) async fn consultar(
    estado: &ServeState,
    id: Value,
    params: &Value,
    tenant: Option<&str>,
    cancelar: bool,
) -> Response {
    let tarea = params
        .get("id")
        .and_then(Value::as_str)
        .filter(|tarea| !tarea.is_empty() && tarea.len() <= MAX_ID_BYTES);
    let Some(tarea) = tarea else {
        return error_rpc(id, INVALID_PARAMS, "params.id must be a task id");
    };
    let resultado = if cancelar {
        estado.host.cancel_for(tenant, tarea).await
    } else {
        estado.host.get_for(tenant, tarea).await
    };
    match resultado {
        Ok(vista) => exito(id, vista.json),
        Err(error) => desde_anfitrion(id, error),
    }
}

pub(super) async fn enviar(
    estado: &ServeState,
    id: Value,
    params: &Value,
    tenant: Option<&str>,
    flujo: bool,
) -> Response {
    let invalido = |motivo: &str| error_rpc(id.clone(), INVALID_PARAMS, motivo);
    let Some(mensaje) = params.get("message") else {
        return invalido("params.message is required");
    };
    let Ok(mensaje) = serde_json::from_value::<WireMessage>(mensaje.clone()) else {
        return invalido("params.message is not a valid Message");
    };
    if mensaje.role != WireRole::User {
        return invalido("params.message.role must be ROLE_USER");
    }
    // El agente servido nunca pide entrada (solo lee), así que no hay tarea que reanudar.
    if mensaje.task_id.is_some() {
        return error_rpc(
            id,
            UNSUPPORTED_OPERATION,
            "continuing an existing task is not supported",
        );
    }
    if mensaje.parts.is_empty() {
        return invalido("params.message.parts must not be empty");
    }
    let mut textos = Vec::with_capacity(mensaje.parts.len());
    for parte in &mensaje.parts {
        match (&parte.text, &parte.raw, &parte.url, &parte.data) {
            (Some(texto), None, None, None) => textos.push(texto.as_str()),
            _ => {
                return error_rpc(
                    id,
                    CONTENT_TYPE_NOT_SUPPORTED,
                    "only text parts are supported",
                )
            }
        }
    }
    let prompt = textos.join("\n");
    if prompt.trim().is_empty() {
        return invalido("params.message has no text");
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return invalido("params.message is too large");
    }
    if flujo {
        return match estado.host.start_streaming_for(tenant, prompt).await {
            Ok(eventos) => respuesta_sse(id, eventos),
            Err(error) => desde_anfitrion(id, error),
        };
    }
    let sin_esperar = params
        .get("configuration")
        .and_then(|configuracion| configuracion.get("returnImmediately"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut vista = match estado.host.start_for(tenant, prompt).await {
        Ok(vista) => vista,
        Err(error) => return desde_anfitrion(id, error),
    };
    if !sin_esperar && !vista.terminal {
        let tarea = vista.json.get("id").and_then(Value::as_str).unwrap_or("");
        match estado
            .host
            .wait_terminal_for(tenant, tarea, estado.max_wait)
            .await
        {
            Ok(esperada) => vista = esperada,
            Err(error) => return desde_anfitrion(id, error),
        }
    }
    exito(id, json!({ "task": vista.json }))
}
