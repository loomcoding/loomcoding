//! Tests: streaming.

use super::support::*;
use super::*;

#[tokio::test]
async fn send_streaming_message_answers_with_sse_starting_at_the_task() {
    let (estado, host) = bearer(Falso {
        con_flujo: true,
        ..Falso::default()
    });

    let (http, cabeceras, bytes) = enviar_req(
        &estado,
        buena(flujo_peticion(
            "SendStreamingMessage",
            json!({ "message": mensaje("hola") }),
        )),
    )
    .await;

    assert_eq!(http, StatusCode::OK);
    assert_eq!(cabeceras[header::CONTENT_TYPE], "text/event-stream");
    let eventos = eventos_sse(&bytes);
    assert_eq!(eventos.len(), 2, "{}", String::from_utf8_lossy(&bytes));
    for evento in &eventos {
        assert_eq!(evento["jsonrpc"], "2.0");
        assert_eq!(evento["id"], 7);
    }
    assert_eq!(eventos[0]["result"]["task"]["id"], "t-1");
    assert_eq!(
        eventos[1]["result"]["statusUpdate"]["status"]["state"],
        "TASK_STATE_COMPLETED"
    );
    assert_eq!(*host.iniciadas.lock().unwrap(), ["hola"]);
}

#[tokio::test]
async fn subscribe_to_task_streams_and_maps_its_errors_to_json_rpc() {
    let (estado, _) = bearer(Falso {
        con_flujo: true,
        ..Falso::default()
    });

    let (_, cabeceras, bytes) = enviar_req(
        &estado,
        buena(flujo_peticion("SubscribeToTask", json!({ "id": "t-1" }))),
    )
    .await;
    assert_eq!(cabeceras[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(eventos_sse(&bytes).len(), 2);

    let (_, terminada) = llamar(
        &estado,
        flujo_peticion("SubscribeToTask", json!({ "id": "terminada" })),
    )
    .await;
    assert_eq!(terminada["error"]["code"], UNSUPPORTED_OPERATION);

    let (_, ajena) = llamar(
        &estado,
        flujo_peticion("SubscribeToTask", json!({ "id": "ajena" })),
    )
    .await;
    assert_eq!(ajena["error"]["code"], TASK_NOT_FOUND);

    let (_, sin_id) = llamar(&estado, flujo_peticion("SubscribeToTask", json!({}))).await;
    assert_eq!(sin_id["error"]["code"], INVALID_PARAMS);
}

#[tokio::test]
async fn streaming_without_host_support_is_an_unsupported_operation_and_busy_is_503() {
    let (estado, _) = bearer(Falso::default());
    let (_, respuesta) = llamar(
        &estado,
        flujo_peticion(
            "SendStreamingMessage",
            json!({ "message": mensaje("hola") }),
        ),
    )
    .await;
    assert_eq!(respuesta["error"]["code"], UNSUPPORTED_OPERATION);

    let (estado, _) = bearer(Falso {
        con_flujo: true,
        ocupado: true,
        ..Falso::default()
    });
    let (http, respuesta) = llamar(
        &estado,
        flujo_peticion(
            "SendStreamingMessage",
            json!({ "message": mensaje("hola") }),
        ),
    )
    .await;
    assert_eq!(http, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(respuesta["error"]["code"], SERVER_BUSY);
}

#[tokio::test]
async fn the_streaming_methods_need_the_credential_and_validate_the_message_first() {
    let (estado, host) = bearer(Falso {
        con_flujo: true,
        ..Falso::default()
    });
    let cuerpo = flujo_peticion(
        "SendStreamingMessage",
        json!({ "message": mensaje("hola") }),
    );

    let (http, _, bytes) = enviar_req(
        &estado,
        rpc_peticion(&cuerpo.to_string(), None, Some("1.0")),
    )
    .await;
    assert_eq!(http, StatusCode::UNAUTHORIZED);
    assert!(bytes.is_empty());

    // Un mensaje sin texto se rechaza antes de crear nada.
    let (_, invalido) = llamar(
        &estado,
        flujo_peticion("SendStreamingMessage", json!({ "message": mensaje("  ") })),
    )
    .await;
    assert_eq!(invalido["error"]["code"], INVALID_PARAMS);
    assert!(host.iniciadas.lock().unwrap().is_empty());
}
