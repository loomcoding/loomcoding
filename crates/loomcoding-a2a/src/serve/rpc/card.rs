//! Tarjeta del agente: contenido público y respuesta cacheable.

use super::*;

pub(super) async fn tarjeta_agente(
    Path(tenant): Path<String>,
    State(estado): State<Arc<ServeState>>,
    cabeceras: HeaderMap,
) -> Response {
    let Ok(cards) = estado.agent_cards.read() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(card) = cards.get(&tenant) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    respuesta_tarjeta(card, &cabeceras)
}

pub(super) async fn tarjeta(
    State(estado): State<Arc<ServeState>>,
    cabeceras: HeaderMap,
) -> Response {
    respuesta_tarjeta(&estado.card, &cabeceras)
}

pub(super) fn respuesta_tarjeta(card: &ServedCard, cabeceras: &HeaderMap) -> Response {
    let etag = HeaderValue::from_str(&card.etag).expect("el ETag es ASCII");
    if cabeceras
        .get(header::IF_NONE_MATCH)
        .is_some_and(|valor| *valor == etag)
    {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
    }
    (
        StatusCode::OK,
        [
            (header::ETAG, etag),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("max-age=60"),
            ),
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            ),
        ],
        card.json.to_string(),
    )
        .into_response()
}
