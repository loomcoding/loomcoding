//! Tests: stream.

use super::support::*;
use super::*;

#[tokio::test]
async fn watch_yields_the_whole_task_after_each_event_and_applies_updates() {
    let servidor = MockServer::start().await;
    let cuerpo = [
        sobre(tarea_wire("TASK_STATE_WORKING")),
        ": latido\n\n".to_string(),
        sobre(artefacto_wire("a1", "hola ", false)),
        sobre(artefacto_wire("a1", "mundo", true)),
        sobre(estado_wire("TASK_STATE_COMPLETED")),
    ]
    .concat();
    montar(&servidor, "SubscribeToTask", flujo_sse(cuerpo)).await;

    let bus = EventBus::new(16);
    let mut observaciones = bus.subscribe();
    let cliente = cliente(&servidor, Some("t"))
        .with_streaming(true)
        .with_observer("meteo", Some(bus));
    let tareas = recoger(&cliente).await;

    let tareas: Vec<_> = tareas.into_iter().map(|t| t.unwrap()).collect();
    assert_eq!(tareas.len(), 4);
    assert_eq!(tareas[0].status.state, TaskState::Working);
    assert_eq!(tareas[1].artifacts[0].text(), "hola ");
    // `append` añade una parte al artefacto; `text()` une las partes con salto de línea.
    assert_eq!(tareas[2].artifacts[0].parts.len(), 2);
    assert_eq!(
        tareas[2].artifacts[0].text(),
        "hola 
mundo"
    );
    assert_eq!(tareas[3].status.state, TaskState::Completed);
    assert_eq!(tareas[3].artifacts.len(), 1);
    let recibidas = servidor.received_requests().await.unwrap();
    let cuerpo: Value = serde_json::from_slice(&recibidas[0].body).unwrap();
    assert_eq!(cuerpo["params"]["id"], "t-1");
    assert_eq!(cuerpo["params"]["tenant"], "t");
    assert_eq!(recibidas[0].headers["accept"], "text/event-stream");
    let mut operaciones = Vec::new();
    while let Ok(LiveEvent::A2aObserved(observacion)) = observaciones.try_recv() {
        operaciones.push((observacion.operation, observacion.state));
    }
    assert_eq!(
        operaciones.iter().map(|(op, _)| *op).collect::<Vec<_>>(),
        [
            A2aOperation::SubscribeToTask,
            A2aOperation::TaskArtifactUpdateEvent,
            A2aOperation::TaskArtifactUpdateEvent,
            A2aOperation::TaskStatusUpdateEvent,
        ]
    );
    assert_eq!(
        operaciones.last().unwrap().1.as_deref(),
        Some("TASK_STATE_COMPLETED")
    );
}

#[tokio::test]
async fn without_declared_streaming_watch_does_not_even_touch_the_network() {
    let servidor = MockServer::start().await;

    let error = cliente(&servidor, None).watch("t-1").await.err().unwrap();

    assert!(
        matches!(error, RemoteTaskError::Unsupported(_)),
        "{error:?}"
    );
    assert!(servidor.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn errors_before_the_stream_keep_their_meaning_and_a_401_is_an_auth_failure() {
    let servidor = MockServer::start().await;
    let no_encontrada: Value = serde_json::from_str(NO_ENCONTRADA).unwrap();
    montar(
        &servidor,
        "SubscribeToTask",
        ResponseTemplate::new(200).set_body_json(con_id_1(no_encontrada)),
    )
    .await;
    let error = cliente(&servidor, None)
        .with_streaming(true)
        .watch("t-1")
        .await
        .err()
        .unwrap();
    assert_eq!(error, RemoteTaskError::NotFound);

    let cerrado = MockServer::start().await;
    montar(
        &cerrado,
        "SubscribeToTask",
        ResponseTemplate::new(401).set_body_string("token-secreto-eco"),
    )
    .await;
    let error = cliente(&cerrado, None)
        .with_streaming(true)
        .watch("t-1")
        .await
        .err()
        .unwrap();
    assert!(matches!(error, RemoteTaskError::AuthFailed(_)), "{error:?}");
    assert!(!error.to_string().contains("token-secreto-eco"));
}

#[tokio::test]
async fn a_non_stream_answer_is_refused() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "SubscribeToTask",
        ResponseTemplate::new(200).set_body_string("hola"),
    )
    .await;

    let error = cliente(&servidor, None)
        .with_streaming(true)
        .watch("t-1")
        .await
        .err()
        .unwrap();

    assert!(matches!(error, RemoteTaskError::Other(_)), "{error:?}");
}

#[tokio::test]
async fn events_of_another_task_out_of_order_or_with_a_wrong_id_cut_the_stream() {
    let ajena = json!({ "task": { "id": "otra", "status": { "state": "TASK_STATE_COMPLETED" } } });
    let casos = [
        // La tarea de otro: nunca se da por buena.
        sobre(ajena),
        // Un estado antes de que llegue la tarea.
        sobre(estado_wire("TASK_STATE_COMPLETED")),
        // Un mensaje en un flujo de tarea.
        sobre(json!({ "message": { "messageId": "m", "role": "ROLE_AGENT",
                                   "parts": [{ "text": "x" }] } })),
        // Una respuesta a otra petición.
        format!(
            "data: {}\n\n",
            json!({ "jsonrpc": "2.0", "id": 99, "result": tarea_wire("TASK_STATE_WORKING") })
        ),
        // Una carga vacía y un estado desconocido.
        sobre(json!({})),
        sobre(tarea_wire("TASK_STATE_INVENTADO")),
        // JSON roto.
        "data: {no es json\n\n".to_string(),
    ];
    for cuerpo in casos {
        let servidor = MockServer::start().await;
        montar(&servidor, "SubscribeToTask", flujo_sse(cuerpo.clone())).await;

        let tareas = recoger(&cliente(&servidor, None).with_streaming(true)).await;

        assert_eq!(tareas.len(), 1, "{cuerpo}");
        assert!(tareas[0].is_err(), "{cuerpo}: {:?}", tareas[0]);
    }
}

#[tokio::test]
async fn an_error_event_in_the_stream_is_translated_and_ends_it() {
    let servidor = MockServer::start().await;
    let cuerpo = [
        sobre(tarea_wire("TASK_STATE_WORKING")),
        format!(
            "data: {}\n\n",
            json!({ "jsonrpc": "2.0", "id": 1,
                    "error": { "code": -32001, "message": "task not found" } })
        ),
        sobre(estado_wire("TASK_STATE_COMPLETED")),
    ]
    .concat();
    montar(&servidor, "SubscribeToTask", flujo_sse(cuerpo)).await;

    let tareas = recoger(&cliente(&servidor, None).with_streaming(true)).await;

    assert_eq!(tareas.len(), 2);
    assert!(tareas[0].is_ok());
    assert_eq!(tareas[1], Err(RemoteTaskError::NotFound));
}

#[tokio::test]
async fn an_oversized_line_or_result_cuts_the_stream() {
    let servidor = MockServer::start().await;
    let gigante = format!("data: {}\n\n", "x".repeat(crate::sse::MAX_LINEA + 10));
    montar(&servidor, "SubscribeToTask", flujo_sse(gigante)).await;
    let tareas = recoger(&cliente(&servidor, None).with_streaming(true)).await;
    assert_eq!(tareas.len(), 1);
    assert!(tareas[0].is_err());

    // El resultado acumulado no pasa del tope de resultado del cliente (64 KiB en las pruebas).
    let servidor = MockServer::start().await;
    let cuerpo = [
        sobre(tarea_wire("TASK_STATE_WORKING")),
        sobre(artefacto_wire("a1", &"y".repeat(40_000), false)),
        sobre(artefacto_wire("a2", &"y".repeat(40_000), false)),
    ]
    .concat();
    montar(&servidor, "SubscribeToTask", flujo_sse(cuerpo)).await;
    let tareas = recoger(&cliente(&servidor, None).with_streaming(true)).await;
    assert_eq!(tareas.len(), 3);
    assert!(tareas[1].is_ok());
    assert!(tareas[2].is_err());
}
