//! Tests: responses.

use super::support::*;
use super::*;

#[test]
fn responses_request_opts_in_only_read_tools_and_keeps_stateless_mode() {
    let provider = OpenAiResponsesProvider::new("http://localhost/v1", "test-key");
    let body = provider.build_body(&request());
    assert_eq!(body["store"], false);
    assert!(body.get("previous_response_id").is_none());
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["type"] == "programmatic_tool_calling"));
    assert_eq!(
        body["tools"][0]["allowed_callers"],
        json!(["direct", "programmatic"])
    );
    assert!(body["tools"][1].get("allowed_callers").is_none());
}

#[test]
fn responses_replay_preserves_program_items_and_caller_on_tool_results() {
    use crate::message::{ContentPart, Message, ProviderState, Role};
    let provider = OpenAiResponsesProvider::new("http://localhost/v1", "test-key");
    let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
        id: "call-1".into(),
        name: "view".into(),
        input: json!({"path":"README.md"}),
    }]);
    assistant.provider_state = Some(ProviderState {
        provider: "openai_responses".into(),
        data: json!({
            "response_id":"resp-1", "model":"model-with-ptc",
            "base_url":"http://localhost/v1",
            "replay_items":[{
                "type":"program","call_id":"prog-1","code":"const x = 1",
                "fingerprint":"opaque"
            },{
                "type":"function_call","call_id":"call-1","name":"view",
                "arguments":"{}","caller":{"type":"program","caller_id":"prog-1"}
            }],
            "caller_by_call_id":{"call-1":{"type":"program","caller_id":"prog-1"}}
        }),
    });
    let result = Message::new(
        Role::Tool,
        vec![ContentPart::ToolResult {
            call_id: "call-1".into(),
            content: "file contents".into(),
            is_error: false,
        }],
    );
    let mut req = request();
    req.messages = vec![assistant, result];
    let body = provider.build_body(&req);
    assert_eq!(body["store"], false);
    assert_eq!(body["input"][0]["type"], "program");
    assert_eq!(body["input"][1]["type"], "function_call");
    assert_eq!(body["input"][2]["type"], "function_call_output");
    assert_eq!(body["input"][2]["caller"]["caller_id"], "prog-1");
}

#[tokio::test]
async fn rejects_programmatic_calls_to_ineligible_tools() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"resp_1","status":"completed","output":[{
                "type":"function_call","call_id":"call_1","name":"write",
                "arguments":"{}","caller":{"type":"program","caller_id":"prog_1"}
            }]
        })))
        .mount(&server)
        .await;
    let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
    assert!(provider
        .stream(request(), CancellationToken::new())
        .await
        .is_err());
}

#[tokio::test]
async fn unsupported_programmatic_mode_retries_once_with_direct_tools() {
    use futures::StreamExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error":{"message":"programmatic_tool_calling is not supported"}
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"resp_direct","status":"completed","output":[]
        })))
        .mount(&server)
        .await;

    let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
    let mut stream = provider
        .stream(request(), CancellationToken::new())
        .await
        .unwrap();
    let mut notice = false;
    while let Some(chunk) = stream.next().await {
        if matches!(chunk, StreamChunk::ProviderNotice(ref text) if text.contains("using direct calls"))
        {
            notice = true;
        }
    }
    assert!(notice);
    let sent = server.received_requests().await.unwrap();
    assert_eq!(sent.len(), 2);
    let fallback: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
    assert!(fallback["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["type"] != "programmatic_tool_calling"));
}

/// Respuesta del servidor simulado: una vez rechaza el modo programático; luego contesta bien.
async fn servidor_que_rechaza_ptc_una_vez(status: u16, mensaje: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(status).set_body_json(json!({
            "error":{"message": mensaje}
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"resp_direct","status":"completed","output":[{
                "type":"message","role":"assistant",
                "content":[{"type":"output_text","text":"hecho"}]
            }]
        })))
        .mount(&server)
        .await;
    server
}

async fn vaciar(provider: &OpenAiResponsesProvider) -> Vec<String> {
    let mut stream = provider
        .stream(request(), CancellationToken::new())
        .await
        .unwrap();
    let mut avisos = Vec::new();
    while let Some(chunk) = stream.next().await {
        if let StreamChunk::ProviderNotice(texto) = chunk {
            avisos.push(texto);
        }
    }
    avisos
}

fn usa_ptc(cuerpo: &[u8]) -> bool {
    let value: serde_json::Value = serde_json::from_slice(cuerpo).unwrap();
    value["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["type"] == "programmatic_tool_calling")
}

#[tokio::test]
async fn a_programmatic_rejection_is_remembered_and_not_repeated_on_the_next_turn() {
    let server =
        servidor_que_rechaza_ptc_una_vez(400, "programmatic_tool_calling is not supported").await;
    let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");

    vaciar(&provider).await;
    let avisos = vaciar(&provider).await;

    let enviadas = server.received_requests().await.unwrap();
    // Turno 1: PTC rechazado + reintento directo. Turno 2: directo desde el principio.
    assert_eq!(enviadas.len(), 3);
    assert!(usa_ptc(&enviadas[0].body));
    assert!(!usa_ptc(&enviadas[1].body));
    assert!(
        !usa_ptc(&enviadas[2].body),
        "no se debe repetir la petición rechazada"
    );
    assert!(
        avisos
            .iter()
            .any(|texto| texto.contains("rejected earlier")),
        "{avisos:?}"
    );
}

#[tokio::test]
async fn a_rejection_for_another_model_does_not_disable_programmatic_calls() {
    let server =
        servidor_que_rechaza_ptc_una_vez(400, "programmatic_tool_calling is not supported").await;
    let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
    vaciar(&provider).await;

    let mut otra = request();
    otra.model = "otro-modelo".into();
    let mut stream = provider
        .stream(otra, CancellationToken::new())
        .await
        .unwrap();
    while stream.next().await.is_some() {}

    let enviadas = server.received_requests().await.unwrap();
    assert_eq!(enviadas.len(), 3);
    assert!(usa_ptc(&enviadas[2].body), "otro modelo es otro destino");
}

#[tokio::test]
async fn an_auth_or_quota_error_that_mentions_programmatic_calls_does_not_fall_back() {
    for status in [401u16, 402, 429, 500] {
        let server = servidor_que_rechaza_ptc_una_vez(
            status,
            "programmatic_tool_calling: check your key or quota",
        )
        .await;
        let provider = OpenAiResponsesProvider::new(server.uri(), "test-key");
        let resultado = provider.stream(request(), CancellationToken::new()).await;
        assert!(resultado.is_err(), "HTTP {status} no es incompatibilidad");
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
