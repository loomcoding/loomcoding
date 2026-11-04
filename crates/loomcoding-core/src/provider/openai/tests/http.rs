//! Tests: http.

use super::*;

#[tokio::test]
async fn streams_text_then_done() {
    let server = MockServer::start().await;
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n";
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let p = OpenAiCompatProvider::new(server.uri(), "sk-test");
    let req = ProviderRequest {
        model: "m".into(),
        system: String::new(),
        messages: vec![],
        tools: vec![],
        temperature: 0.0,
        max_tokens: 16,
        effort: None,
        programmatic_tool_calling: false,
    };
    let mut s = p.stream(req, CancellationToken::new()).await.unwrap();
    let mut texts = String::new();
    let mut saw_done = false;
    while let Some(c) = s.next().await {
        match c {
            StreamChunk::TextDelta(t) => texts.push_str(&t),
            StreamChunk::Done(_) => saw_done = true,
            _ => {}
        }
    }
    assert_eq!(texts, "hello");
    assert!(saw_done);
}

#[tokio::test]
async fn request_timeout_reports_failure_before_response_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_raw("data: [DONE]\n\n", "text/event-stream"),
        )
        .mount(&server)
        .await;
    let mut provider = OpenAiCompatProvider::new(server.uri(), "sk-test");
    provider.request_timeout = Duration::from_millis(10);
    let request = ProviderRequest {
        model: "m".into(),
        system: String::new(),
        messages: vec![],
        tools: vec![],
        temperature: 0.0,
        max_tokens: 16,
        effort: None,
        programmatic_tool_calling: false,
    };

    assert!(matches!(
        provider.stream(request, CancellationToken::new()).await,
        Err(CoreError::Provider(message)) if message.contains("timed out before response headers")
    ));
}

fn request_for_error_tests() -> ProviderRequest {
    ProviderRequest {
        model: "m".into(),
        system: String::new(),
        messages: vec![],
        tools: vec![],
        temperature: 0.0,
        max_tokens: 16,
        effort: None,
        programmatic_tool_calling: false,
    }
}

#[tokio::test]
async fn a_llama_cpp_400_reaches_the_caller_as_a_context_overflow() {
    let server = MockServer::start().await;
    let body = "{\"error\":{\"code\":400,\"message\":\"request (54608 tokens) exceeds the available context size (35584 tokens), try increasing it\",\"type\":\"exceed_context_size_error\",\"n_prompt_tokens\":54608,\"n_ctx\":35584}}";
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new(server.uri(), "sk-test");
    let Err(error) = provider
        .stream(request_for_error_tests(), CancellationToken::new())
        .await
    else {
        panic!("un 400 debe ser un error");
    };
    let CoreError::Provider(message) = &error else {
        panic!("se esperaba un error del proveedor: {error:?}");
    };
    assert!(message.starts_with("http 400 Bad Request: "), "{message}");
    assert!(message.contains("exceed_context_size_error"), "{message}");
    assert!(crate::agent::compact::is_context_overflow(&error));
}

#[tokio::test]
async fn an_error_body_is_bounded_in_the_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_raw("x".repeat(100_000), "text/plain"))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new(server.uri(), "sk-test");
    let Err(CoreError::Provider(message)) = provider
        .stream(request_for_error_tests(), CancellationToken::new())
        .await
    else {
        panic!("un 400 debe ser un error del proveedor");
    };
    assert!(message.len() < 700, "{} bytes", message.len());
}

#[tokio::test]
async fn an_error_without_a_body_keeps_the_plain_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new(server.uri(), "sk-test");
    let Err(CoreError::Provider(message)) = provider
        .stream(request_for_error_tests(), CancellationToken::new())
        .await
    else {
        panic!("un 503 debe ser un error del proveedor");
    };
    assert_eq!(message, "http 503 Service Unavailable");
}

fn slow_server_response(delay_ms: u64) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_delay(Duration::from_millis(delay_ms))
        .set_body_raw("data: [DONE]\n\n", "text/event-stream")
}

#[tokio::test]
async fn a_configured_timeout_cuts_a_slow_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(slow_server_response(1_000))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new(server.uri(), "sk-test")
        .with_request_timeout(Duration::from_millis(200));
    let result = provider
        .stream(request_for_error_tests(), CancellationToken::new())
        .await;
    assert!(
        result.is_err(),
        "la respuesta lenta debía cortarse por plazo"
    );
}

#[tokio::test]
async fn a_configured_timeout_lets_a_response_within_it_through() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(slow_server_response(200))
        .mount(&server)
        .await;
    let provider = OpenAiCompatProvider::new(server.uri(), "sk-test")
        .with_request_timeout(Duration::from_secs(2));
    let result = provider
        .stream(request_for_error_tests(), CancellationToken::new())
        .await;
    assert!(result.is_ok(), "la respuesta dentro del plazo debía pasar");
}

#[test]
fn without_configuration_the_timeout_stays_at_120_seconds() {
    let provider = OpenAiCompatProvider::new("http://x", "sk-test");
    assert_eq!(provider.request_timeout, Duration::from_secs(120));
}
