//! Tests: send.

use super::support::*;
use super::*;

#[tokio::test]
async fn a_blocking_send_waits_and_return_immediately_does_not() {
    let (estado, host) = bearer(Falso::default());

    let (_, valor) = llamar(&estado, envio("hola")).await;
    assert_eq!(
        valor["result"]["task"]["status"]["state"],
        "TASK_STATE_COMPLETED"
    );
    assert_eq!(*host.esperas.lock().unwrap(), 1);
    assert_eq!(host.iniciadas.lock().unwrap().as_slice(), ["hola"]);

    let mut inmediato = envio("otra");
    inmediato["params"]["configuration"] = json!({ "returnImmediately": true });
    let (_, valor) = llamar(&estado, inmediato).await;
    assert_eq!(
        valor["result"]["task"]["status"]["state"],
        "TASK_STATE_WORKING"
    );
    assert_eq!(*host.esperas.lock().unwrap(), 1, "no debe esperar");
}

#[tokio::test]
async fn a_task_already_terminal_is_not_waited_for() {
    let (estado, host) = bearer(Falso {
        termina_al_instante: true,
        ..Falso::default()
    });

    let (_, valor) = llamar(&estado, envio("hola")).await;

    assert_eq!(
        valor["result"]["task"]["status"]["state"],
        "TASK_STATE_COMPLETED"
    );
    assert_eq!(*host.esperas.lock().unwrap(), 0);
}

#[tokio::test]
async fn non_text_parts_are_refused_and_no_task_is_created() {
    let (estado, host) = bearer(Falso::default());
    for parte in [
        json!({ "raw": "aGVsbG8=" }),
        json!({ "url": "https://x.example/f" }),
        json!({ "data": { "a": 1 } }),
    ] {
        let cuerpo = json!({ "jsonrpc": "2.0", "id": 1, "method": "SendMessage",
            "params": { "message": { "messageId": "m", "role": "ROLE_USER", "parts": [parte] } } });

        let (_, valor) = llamar(&estado, cuerpo).await;

        assert_eq!(valor["error"]["code"], CONTENT_TYPE_NOT_SUPPORTED);
    }
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_messages_are_invalid_params() {
    let (estado, host) = bearer(Falso::default());
    let enorme = "x".repeat(MAX_PROMPT_BYTES + 1);
    let mut agente = envio("hola");
    agente["params"]["message"]["role"] = json!("ROLE_AGENT");
    let casos = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "SendMessage", "params": { "message": 3 } }),
        agente,
        envio(""),
        envio("   "),
        envio(&enorme),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "SendMessage",
            "params": { "message": { "messageId": "m", "role": "ROLE_USER", "parts": [] } } }),
    ];
    for caso in casos {
        let (_, valor) = llamar(&estado, caso.clone()).await;

        assert_eq!(valor["error"]["code"], INVALID_PARAMS, "{caso}");
    }
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn continuing_a_task_is_unsupported() {
    let (estado, host) = bearer(Falso::default());
    let mut continuacion = envio("hola");
    continuacion["params"]["message"]["taskId"] = json!("t-1");

    let (_, valor) = llamar(&estado, continuacion).await;

    assert_eq!(valor["error"]["code"], UNSUPPORTED_OPERATION);
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_busy_server_answers_503_with_a_json_rpc_error() {
    let (estado, _) = bearer(Falso {
        ocupado: true,
        ..Falso::default()
    });

    let (http, valor) = llamar(&estado, envio("hola")).await;

    assert_eq!(http, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(valor["error"]["code"], SERVER_BUSY);
}
