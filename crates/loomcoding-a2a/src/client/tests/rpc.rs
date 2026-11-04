//! Tests: rpc.

use super::support::*;
use super::*;

#[tokio::test]
async fn send_returns_the_task_of_the_spec_example_and_asks_not_to_block() {
    let servidor = MockServer::start().await;
    let respuesta: Value = serde_json::from_str(ENVIO).unwrap();
    montar(
        &servidor,
        "SendMessage",
        ResponseTemplate::new(200).set_body_json(con_id_1(respuesta)),
    )
    .await;

    let bus = EventBus::new(4);
    let mut observaciones = bus.subscribe();
    let respuesta = cliente(&servidor, None)
        .with_observer("meteo", Some(bus))
        .send(Message::user_text("What is the weather today?"))
        .await
        .unwrap();

    let LiveEvent::A2aObserved(observacion) = observaciones.recv().await.unwrap() else {
        panic!("se esperaba la observación local del intercambio A2A");
    };
    assert_eq!(observacion.agent, "meteo");
    assert_eq!(observacion.operation, A2aOperation::SendMessage);
    assert_eq!(observacion.rpc_id.as_deref(), Some("1"));
    assert_eq!(observacion.task_id.as_deref(), Some("task-uuid"));
    assert_eq!(observacion.context_id.as_deref(), Some("context-uuid"));
    assert_eq!(observacion.state.as_deref(), Some("TASK_STATE_COMPLETED"));
    assert!(observacion.succeeded);
    assert!(observacion.artifact.is_none());

    match respuesta {
        RemoteReply::Task(tarea) => {
            assert_eq!(tarea.id, "task-uuid");
            assert_eq!(tarea.context_id.as_deref(), Some("context-uuid"));
            assert_eq!(tarea.status.state, TaskState::Completed);
            assert_eq!(
                tarea.artifacts[0].text(),
                "Today will be sunny with a high of 75°F"
            );
        }
        otro => panic!("se esperaba una tarea, y salió {otro:?}"),
    }
    let recibidas = servidor.received_requests().await.unwrap();
    let cuerpo: Value = serde_json::from_slice(&recibidas[0].body).unwrap();
    assert_eq!(cuerpo["params"]["configuration"]["returnImmediately"], true);
    assert_eq!(cuerpo["params"]["message"]["role"], "ROLE_USER");
    assert_eq!(
        cuerpo["params"]["message"]["parts"][0]["text"],
        "What is the weather today?"
    );
    assert!(cuerpo["params"].get("tenant").is_none());
}

#[tokio::test]
async fn the_tenant_of_the_selected_interface_travels_in_every_request() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(200).set_body_json(con_id_1(
            json!({"jsonrpc":"2.0","result":{"id":"t","status":{"state":"TASK_STATE_WORKING"}}}),
        )),
    )
    .await;

    cliente(&servidor, Some("equipo-a")).get("t").await.unwrap();

    let recibidas = servidor.received_requests().await.unwrap();
    let cuerpo: Value = serde_json::from_slice(&recibidas[0].body).unwrap();
    assert_eq!(cuerpo["params"]["tenant"], "equipo-a");
    assert_eq!(cuerpo["params"]["id"], "t");
    assert_eq!(cuerpo["params"]["historyLength"], 0);
}

#[tokio::test]
async fn send_can_be_answered_with_a_message_instead_of_a_task() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "SendMessage",
        ResponseTemplate::new(200).set_body_json(con_id_1(json!({
            "jsonrpc": "2.0",
            "result": {"message": {"messageId": "m", "role": "ROLE_AGENT", "parts": [{"text": "hola"}]}}
        }))),
    )
    .await;

    let respuesta = cliente(&servidor, None)
        .send(Message::user_text("hola"))
        .await
        .unwrap();

    match respuesta {
        RemoteReply::Message(mensaje) => assert_eq!(mensaje.text(), "hola"),
        otro => panic!("se esperaba un mensaje, y salió {otro:?}"),
    }
}

#[tokio::test]
async fn cancel_reports_what_the_remote_says_and_not_what_was_asked() {
    let servidor = MockServer::start().await;
    // El remoto contesta a la cancelación con la tarea todavía trabajando.
    montar(
        &servidor,
        "CancelTask",
        ResponseTemplate::new(200).set_body_json(con_id_1(
            json!({"jsonrpc":"2.0","result":{"id":"t","status":{"state":"TASK_STATE_WORKING"}}}),
        )),
    )
    .await;

    let tarea = cliente(&servidor, None).cancel("t").await.unwrap();

    assert_eq!(tarea.status.state, TaskState::Working);
}

#[tokio::test]
async fn the_a2a_error_codes_are_translated() {
    for (codigo, esperado) in [
        (-32001, RemoteTaskError::NotFound),
        (-32002, RemoteTaskError::NotCancelable),
        (-32004, RemoteTaskError::Unsupported("ya terminada".into())),
        (
            -32009,
            RemoteTaskError::Other("code -32009: versión".into()),
        ),
    ] {
        let servidor = MockServer::start().await;
        let mensaje = match codigo {
            -32004 => "ya terminada",
            -32009 => "versión",
            _ => "x",
        };
        montar(
            &servidor,
            "GetTask",
            ResponseTemplate::new(200).set_body_json(
                json!({"jsonrpc":"2.0","id":1,"error":{"code":codigo,"message":mensaje}}),
            ),
        )
        .await;

        let resultado = cliente(&servidor, None).get("t").await;

        assert_eq!(resultado, Err(esperado), "código {codigo}");
    }
}

#[tokio::test]
async fn the_spec_task_not_found_example_is_translated() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(200).set_body_raw(NO_ENCONTRADA, "application/json"),
    )
    .await;

    let resultado = cliente(&servidor, None).get("nonexistent-task-id").await;

    assert_eq!(resultado, Err(RemoteTaskError::NotFound));
}

#[tokio::test]
async fn a_response_with_the_wrong_id_or_with_both_result_and_error_is_rejected() {
    for cuerpo in [
        json!({"jsonrpc":"2.0","id":99,"result":{"id":"t","status":{"state":"TASK_STATE_WORKING"}}}),
        json!({"jsonrpc":"2.0","id":1,"result":{"id":"t","status":{"state":"TASK_STATE_WORKING"}},"error":{"code":-1,"message":"x"}}),
        json!({"jsonrpc":"2.0","id":1}),
    ] {
        let servidor = MockServer::start().await;
        montar(
            &servidor,
            "GetTask",
            ResponseTemplate::new(200).set_body_json(cuerpo.clone()),
        )
        .await;

        let resultado = cliente(&servidor, None).get("t").await;

        assert!(
            matches!(resultado, Err(RemoteTaskError::Other(_))),
            "{cuerpo}: {resultado:?}"
        );
    }
}

#[tokio::test]
async fn an_unknown_task_state_is_an_error_and_not_a_guess() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(200).set_body_json(con_id_1(
            json!({"jsonrpc":"2.0","result":{"id":"t","status":{"state":"TASK_STATE_DORMIDA"}}}),
        )),
    )
    .await;

    let resultado = cliente(&servidor, None).get("t").await;

    assert!(
        matches!(resultado, Err(RemoteTaskError::Other(_))),
        "{resultado:?}"
    );
}

#[tokio::test]
async fn a_response_larger_than_the_cap_is_refused() {
    let servidor = MockServer::start().await;
    let enorme = "x".repeat(10_000);
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(200).set_body_json(con_id_1(json!({
            "jsonrpc":"2.0",
            "result":{"id":"t","status":{"state":"TASK_STATE_COMPLETED"},
                      "artifacts":[{"artifactId":"a","parts":[{"text": enorme}]}]}
        }))),
    )
    .await;
    let pequeno = JsonRpcClient::new(http(2), format!("{}/a2a/v1", servidor.uri()), None, 1_000);

    let resultado = pequeno.get("t").await;

    match resultado {
        Err(RemoteTaskError::Other(texto)) => assert!(texto.contains("exceeds"), "{texto}"),
        otro => panic!("se esperaba un error de tamaño, y salió {otro:?}"),
    }
}

#[tokio::test]
async fn a_remote_that_does_not_answer_in_time_is_a_timeout() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(200)
            .set_body_json(json!({}))
            .set_delay(Duration::from_secs(4)),
    )
    .await;
    let lento = JsonRpcClient::new(http(1), format!("{}/a2a/v1", servidor.uri()), None, 1_000);

    let resultado = lento.get("t").await;

    assert_eq!(resultado, Err(RemoteTaskError::Timeout));
}

#[tokio::test]
async fn a_plain_http_failure_without_json_reports_the_status() {
    let servidor = MockServer::start().await;
    montar(
        &servidor,
        "GetTask",
        ResponseTemplate::new(502).set_body_string("bad gateway"),
    )
    .await;

    let resultado = cliente(&servidor, None).get("t").await;

    assert_eq!(
        resultado,
        Err(RemoteTaskError::Other("HTTP status 502".into()))
    );
}
