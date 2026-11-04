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
