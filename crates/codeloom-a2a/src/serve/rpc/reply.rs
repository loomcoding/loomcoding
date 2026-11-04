//! Respuestas HTTP y JSON-RPC: éxito, error, no autorizado y flujo SSE.

use super::*;

pub(super) fn respuesta_json(valor: Value) -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        valor.to_string(),
    )
        .into_response()
}

pub(super) fn error_rpc(id: Value, codigo: i64, mensaje: &str) -> Response {
    respuesta_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": codigo, "message": mensaje },
    }))
}

pub(super) fn exito(id: Value, resultado: Value) -> Response {
    respuesta_json(json!({ "jsonrpc": "2.0", "id": id, "result": resultado }))
}

/// 401 sin cuerpo informativo: no se dice si faltaba la credencial o era incorrecta.
pub(super) fn no_autorizado(esquema: &ServeAuthScheme) -> Response {
    let mut respuesta = StatusCode::UNAUTHORIZED.into_response();
    if matches!(esquema, ServeAuthScheme::Bearer) {
        respuesta
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    }
    respuesta
}

pub(super) fn desde_anfitrion(id: Value, error: HostError) -> Response {
    match error {
        HostError::NotFound => error_rpc(id, TASK_NOT_FOUND, "task not found"),
        HostError::NotCancelable => error_rpc(id, TASK_NOT_CANCELABLE, "task is not cancelable"),
        HostError::Unsupported => error_rpc(id, UNSUPPORTED_OPERATION, "operation not supported"),
        HostError::Busy => {
            let mut respuesta = error_rpc(id, SERVER_BUSY, "server busy: too many live tasks");
            *respuesta.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
            respuesta
        }
        // El detalle interno no se entrega al cliente.
        HostError::Other(_) => error_rpc(id, INTERNAL_ERROR, "internal error"),
    }
}

/// Emite el flujo como SSE: cada evento es una línea `data:` con una respuesta JSON-RPC completa
/// (§9.4.2). El flujo se cierra solo cuando se agota; los errores previos a su apertura ya viajaron
/// como JSON-RPC normal.
pub(super) fn respuesta_sse(id: Value, eventos: EventStream) -> Response {
    let eventos = eventos.map(move |evento| {
        let sobre = json!({ "jsonrpc": "2.0", "id": id, "result": evento });
        Ok::<_, std::convert::Infallible>(Event::default().data(sobre.to_string()))
    });
    Sse::new(eventos)
        .keep_alive(KeepAlive::new().interval(LATIDO).text("latido"))
        .into_response()
}
