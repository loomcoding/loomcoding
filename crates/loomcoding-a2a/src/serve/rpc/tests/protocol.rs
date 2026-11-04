//! Tests: protocol.

use super::support::*;
use super::*;

#[tokio::test]
async fn an_oversized_body_is_refused_with_413() {
    let (estado, host) = bearer(Falso::default());
    let enorme = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"SendMessage","params":{{"x":"{}"}}}}"#,
        "a".repeat(MAX_BODY_BYTES)
    );

    let (http, _, _) = enviar_req(
        &estado,
        rpc_peticion(&enorme, Some(&format!("Bearer {SECRETO}")), Some("1.0")),
    )
    .await;

    assert_eq!(http, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn malformed_json_rpc_gets_a_json_rpc_error_not_a_500() {
    let (estado, _) = bearer(Falso::default());
    for (cuerpo, codigo) in [
        ("{no es json".to_string(), PARSE_ERROR),
        (json!([1, 2]).to_string(), INVALID_REQUEST),
        (
            json!({"jsonrpc": "1.0", "id": 1, "method": "GetTask"}).to_string(),
            INVALID_REQUEST,
        ),
        (
            json!({"jsonrpc": "2.0", "method": "GetTask"}).to_string(),
            INVALID_REQUEST,
        ),
        (
            json!({"jsonrpc": "2.0", "id": [1], "method": "GetTask"}).to_string(),
            INVALID_REQUEST,
        ),
        (
            json!({"jsonrpc": "2.0", "id": 1, "method": 7}).to_string(),
            INVALID_REQUEST,
        ),
    ] {
        let (http, _, bytes) = enviar_req(
            &estado,
            rpc_peticion(&cuerpo, Some(&format!("Bearer {SECRETO}")), Some("1.0")),
        )
        .await;
        let valor: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(http, StatusCode::OK, "{cuerpo}");
        assert_eq!(valor["error"]["code"], codigo, "{cuerpo}");
        assert_eq!(valor["id"], Value::Null, "{cuerpo}");
    }
}

#[tokio::test]
async fn only_a2a_version_1_0_is_served() {
    let (estado, host) = bearer(Falso::default());
    let cuerpo = envio("hola").to_string();
    for version in [None, Some(""), Some("0.3"), Some("1.1")] {
        let (_, _, bytes) = enviar_req(
            &estado,
            rpc_peticion(&cuerpo, Some(&format!("Bearer {SECRETO}")), version),
        )
        .await;
        let valor: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(valor["error"]["code"], VERSION_NOT_SUPPORTED, "{version:?}");
        assert_eq!(valor["id"], 1);
    }
    assert!(host.iniciadas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_and_unknown_methods_have_their_own_codes() {
    let (estado, _) = bearer(Falso::default());
    for (metodo, codigo) in [
        ("ListTasks", UNSUPPORTED_OPERATION),
        ("GetExtendedAgentCard", UNSUPPORTED_OPERATION),
        ("CreateTaskPushNotificationConfig", PUSH_NOT_SUPPORTED),
        ("GetTaskPushNotificationConfig", PUSH_NOT_SUPPORTED),
        ("ListTaskPushNotificationConfigs", PUSH_NOT_SUPPORTED),
        ("DeleteTaskPushNotificationConfig", PUSH_NOT_SUPPORTED),
        ("message/send", METHOD_NOT_FOUND),
        ("Inventado", METHOD_NOT_FOUND),
    ] {
        let cuerpo = json!({ "jsonrpc": "2.0", "id": 1, "method": metodo, "params": {} });

        let (_, valor) = llamar(&estado, cuerpo).await;

        assert_eq!(valor["error"]["code"], codigo, "{metodo}");
    }
}

#[tokio::test]
async fn an_internal_host_failure_never_leaks_its_detail() {
    struct Roto;
    #[async_trait]
    impl TaskHost for Roto {
        async fn start(&self, _: String) -> Result<TaskView, HostError> {
            Err(HostError::Other("ruta /home/secreta falló".into()))
        }
        async fn get(&self, _: &str) -> Result<TaskView, HostError> {
            Err(HostError::Other("detalle".into()))
        }
        async fn cancel(&self, _: &str) -> Result<TaskView, HostError> {
            Err(HostError::Other("detalle".into()))
        }
        async fn wait_terminal(&self, _: &str, _: Duration) -> Result<TaskView, HostError> {
            Err(HostError::Other("detalle".into()))
        }
    }
    let (mut estado, _) = bearer(Falso::default());
    Arc::get_mut(&mut estado).unwrap().host = Arc::new(Roto);

    let (_, _, bytes) = enviar_req(&estado, buena(envio("hola"))).await;
    let texto = String::from_utf8(bytes).unwrap();

    assert!(texto.contains("internal error"));
    assert!(!texto.contains("secreta"), "{texto}");
}
