//! Tests de Anthropic: programmatic.

use super::*;
use crate::message::Usage;
use crate::provider::ToolSpec;
use serde_json::json;

fn request() -> ProviderRequest {
    ProviderRequest {
        model: "claude-test".into(),
        system: "system".into(),
        messages: vec![],
        tools: vec![
            ToolSpec {
                name: "view".into(),
                description: "read a file".into(),
                input_schema: json!({"type":"object"}),
                programmatic_allowed: true,
            },
            ToolSpec {
                name: "write".into(),
                description: "write a file".into(),
                input_schema: json!({"type":"object"}),
                programmatic_allowed: false,
            },
        ],
        temperature: 0.0,
        max_tokens: 128,
        effort: None,
        programmatic_tool_calling: true,
    }
}

#[tokio::test]
async fn stream_error_preserves_anthropic_400_message() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error":{"message":"messages.0.content: invalid"}
        })))
        .mount(&server)
        .await;
    let provider = AnthropicProvider::new(server.uri(), "test-key");
    let mut request = request();
    request.programmatic_tool_calling = false;
    let error = match provider.stream(request, CancellationToken::new()).await {
        Err(error) => error.to_string(),
        Ok(_) => panic!("400 response must be an error"),
    };
    assert!(error.contains("HTTP 400: messages.0.content: invalid"));
}

#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn openrouter_haiku_live_direct_vs_ptc() {
    use crate::message::{ContentPart, Message, Role};
    use futures::StreamExt;
    use std::time::Instant;

    let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
    let provider = AnthropicProvider::new("https://openrouter.ai/api", key);
    let mut total_cost = 0.0_f64;
    for ptc in [false, true] {
        let started = Instant::now();
        let mut req = request();
        req.model = OPENROUTER_HAIKU_55.into();
        req.system = "Read the fixture with the view tool and return only its VALUE.".into();
        req.messages = vec![Message::user(
            "Use view exactly once to read the local fixture.",
        )];
        req.max_tokens = 128;
        req.programmatic_tool_calling = ptc;
        let mut turns = 0;
        let mut usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            reported_cost_usd: None,
        };
        let mut called_programmatically = false;
        let mut final_text = String::new();
        let mut provider_rejected = false;
        for _ in 0..2 {
            turns += 1;
            let mut stream = match provider.stream(req.clone(), CancellationToken::new()).await {
                Ok(stream) => stream,
                Err(_) => {
                    provider_rejected = true;
                    break;
                }
            };
            let mut parts = Vec::new();
            let mut state = None;
            let mut tool_call = None;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    StreamChunk::TextDelta(text) => {
                        final_text.push_str(&text);
                        parts.push(ContentPart::Text { text });
                    }
                    StreamChunk::ToolCallComplete { id, name, input } => {
                        tool_call = Some((id.clone(), name.clone(), input.clone()));
                        parts.push(ContentPart::ToolCall { id, name, input });
                    }
                    StreamChunk::Usage(u) => {
                        usage.input_tokens = usage.input_tokens.max(u.input_tokens);
                        usage.output_tokens = usage.output_tokens.max(u.output_tokens);
                    }
                    StreamChunk::UsageUpdate(u) => {
                        if let Some(input) = u.input_tokens {
                            usage.input_tokens = usage.input_tokens.max(input);
                        }
                        if let Some(output) = u.output_tokens {
                            usage.output_tokens = usage.output_tokens.max(output);
                        }
                    }
                    StreamChunk::ProviderState(s) => state = Some(s),
                    StreamChunk::ProviderNotice(n) if n.contains("programmatic call") => {
                        called_programmatically = true
                    }
                    _ => {}
                }
            }
            let Some((id, name, _)) = tool_call else {
                break;
            };
            assert_eq!(name, "view");
            let mut assistant = Message::assistant(parts);
            assistant.provider_state = state;
            req.messages.push(assistant);
            req.messages.push(Message::new(
                Role::Tool,
                vec![ContentPart::ToolResult {
                    call_id: id,
                    content: "VALUE=cedar-7".into(),
                    is_error: false,
                }],
            ));
        }
        let cost = usage.input_tokens as f64 * 0.10 / 1_000_000.0
            + usage.output_tokens as f64 * 0.50 / 1_000_000.0;
        total_cost += cost;
        println!("mode={} tokens_in={} tokens_out={} turns={} latency_ms={} tool_ptc={} cost_usd={:.6} exact={} provider_rejected={}",
            if ptc { "ptc" } else { "direct" }, usage.input_tokens, usage.output_tokens,
            turns, started.elapsed().as_millis(), called_programmatically, cost,
            final_text.trim() == "cedar-7", provider_rejected);
        assert!(total_cost < 1.0, "USD 1 benchmark cap reached");
    }
}

/// Medición real de caché (A1/B1): cuatro peticiones pequeñas a Haiku 4.5 por OpenRouter. Dos por
/// el adaptador tal cual (sin `cache_control`) y dos con `cache_control` puesto a mano, para ver
/// qué comunica el proveedor y si nuestro lector de caché lo entiende. Tope de gasto: 0,50 USD.
#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn openrouter_haiku_live_cache_usage_measurement() {
    use crate::message::Message;
    use futures::StreamExt;

    const CAP_USD: f64 = 0.50;
    let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
    let provider = AnthropicProvider::new("https://openrouter.ai/api", key.clone());
    // Prefijo estable de unos 7 000 tokens, por encima del mínimo cacheable de Haiku 4.5.
    let system: String = (0..420)
        .map(|i| format!("Rule {i}: keep answers short, cite the rule number, never invent facts about item {i}.
"))
        .collect();
    let mut req = request();
    req.model = "anthropic/claude-haiku-4.5".into();
    req.system = system;
    req.messages = vec![Message::user("Reply with the single word OK.")];
    req.tools = Vec::new();
    req.max_tokens = 16;
    req.programmatic_tool_calling = false;
    let mut spent = 0.0_f64;

    for round in 1..=2 {
        let mut stream = provider
            .stream(req.clone(), CancellationToken::new())
            .await
            .expect("stream");
        let (mut input, mut output, mut cost, mut cache) = (0, 0, None, None);
        while let Some(chunk) = stream.next().await {
            match chunk {
                StreamChunk::UsageUpdate(u) => {
                    input = input.max(u.input_tokens.unwrap_or(0));
                    output = output.max(u.output_tokens.unwrap_or(0));
                    cost = u.reported_cost_usd.or(cost);
                }
                StreamChunk::CacheUsage(c) => cache = Some(c),
                _ => {}
            }
        }
        spent += cost.unwrap_or(input as f64 * 1.0 / 1e6 + output as f64 * 5.0 / 1e6);
        println!("adapter-as-is round={round} input={input} output={output} cost={cost:?} cache_chunk={cache:?}");
        assert!(spent < CAP_USD, "cap reached");
    }

    // Con `cache_control` en el bloque de sistema, a mano: lo que añadiría B1.
    let mut body = provider.build_body(&req);
    body["stream"] = json!(false);
    body["system"] = json!([{
        "type":"text",
        "text": req.system,
        "cache_control": {"type":"ephemeral"}
    }]);
    let client = reqwest::Client::new();
    for round in 1..=2 {
        let response = client
            .post("https://openrouter.ai/api/v1/messages")
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .expect("request");
        let status = response.status();
        let value: serde_json::Value = response.json().await.expect("json");
        let usage = value.get("usage").cloned().unwrap_or(json!(null));
        let parsed = cache_usage(&usage);
        println!("with-cache_control round={round} status={status} usage={usage} parsed_by_cache_usage={parsed:?}");
        spent += usage["cost"].as_f64().unwrap_or(0.02);
        assert!(spent < CAP_USD, "cap reached");
    }
    println!("total_spent_usd~{spent:.5}");
}

fn cached_provider() -> AnthropicProvider {
    AnthropicProvider::new("https://api.anthropic.com", "k").with_prompt_cache(true)
}

fn marked(value: &serde_json::Value) -> usize {
    value.to_string().matches("\"cache_control\"").count()
}

#[test]
fn cache_points_are_off_by_default() {
    let mut req = request();
    req.system = "sistema".into();
    req.messages = vec![crate::message::Message::user("hola")];
    let body = AnthropicProvider::new("https://api.anthropic.com", "k").build_body(&req);
    assert_eq!(body["system"], "sistema");
    assert_eq!(marked(&body), 0);
}

#[test]
fn cache_points_mark_the_system_block_and_the_last_message_block() {
    let mut req = request();
    req.system = "sistema".into();
    req.programmatic_tool_calling = false;
    req.messages = vec![
        crate::message::Message::user("primero"),
        crate::message::Message::user("ultimo"),
    ];
    let body = cached_provider().build_body(&req);
    assert_eq!(body["system"][0]["text"], "sistema");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    let last = body["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["content"][0]["text"], "ultimo");
    assert_eq!(last["content"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(marked(&body), 2, "como mucho dos puntos");
    // Solo el último mensaje se marca; el anterior sigue siendo una cadena.
    assert_eq!(body["messages"][0]["content"], "primero");
}

#[test]
fn cache_points_fall_back_to_the_last_tool_without_a_system_prompt() {
    let mut req = request();
    req.system.clear();
    req.programmatic_tool_calling = false;
    let body = cached_provider().build_body(&req);
    let tools = body["tools"].as_array().unwrap();
    assert!(tools.last().unwrap().get("cache_control").is_some());
    assert_eq!(marked(&body), 1);
}

#[test]
fn cache_points_use_only_the_system_block_without_messages() {
    let mut req = request();
    req.system = "sistema".into();
    req.programmatic_tool_calling = false;
    let body = cached_provider().build_body(&req);
    assert_eq!(marked(&body), 1);
}

#[test]
fn cache_points_skip_a_trailing_thinking_block() {
    let mut body = json!({
        "system": "s",
        "messages": [{"role":"assistant","content":[
            {"type":"text","text":"t"},
            {"type":"thinking","thinking":"x","signature":"sig"}
        ]}]
    });
    apply_cache_points(&mut body);
    assert!(body["messages"][0]["content"][1]
        .get("cache_control")
        .is_none());
    assert_eq!(marked(&body), 1, "solo el punto del sistema");
}

/// Medición real de los puntos de caché del propio adaptador (B1): tres peticiones por streaming a
/// Haiku 4.5 por OpenRouter. Prefijo único por ejecución para no leer cachés de ejecuciones previas.
/// Esperado: la 1.ª escribe, la 2.ª lee el prefijo y la 3.ª, con un mensaje añadido, lo vuelve a leer.
#[tokio::test]
#[ignore = "live OpenRouter calls; incurs provider usage"]
async fn openrouter_haiku_live_adapter_cache_points() {
    use crate::message::{ContentPart, Message};
    use futures::StreamExt;

    const CAP_USD: f64 = 0.15;
    let key = std::env::var("OPENROUTER_API_KEY").expect("set OPENROUTER_API_KEY");
    let provider = AnthropicProvider::new("https://openrouter.ai/api", key).with_prompt_cache(true);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let system: String = std::iter::once(format!(
        "Session nonce {nonce}.
"
    ))
    .chain((0..420).map(|i| {
        format!(
            "Rule {i}: keep answers short, cite the rule number, never invent facts about item {i}.
"
        )
    }))
    .collect();
    let mut req = request();
    req.model = "anthropic/claude-haiku-4.5".into();
    req.system = system;
    req.tools = Vec::new();
    req.max_tokens = 16;
    req.programmatic_tool_calling = false;
    let mut spent = 0.0_f64;
    let mut reads = Vec::new();
    let mut writes = Vec::new();

    for round in 1..=3 {
        req.messages = vec![Message::user("Reply with the single word OK.")];
        if round == 3 {
            req.messages
                .push(Message::assistant(vec![ContentPart::Text {
                    text: "OK".into(),
                }]));
            req.messages.push(Message::user("Say OK once more."));
        }
        let mut stream = provider
            .stream(req.clone(), CancellationToken::new())
            .await
            .expect("stream");
        let (mut input, mut output, mut cost, mut cache) = (0, 0, None, None);
        while let Some(chunk) = stream.next().await {
            match chunk {
                StreamChunk::UsageUpdate(u) => {
                    input = input.max(u.input_tokens.unwrap_or(0));
                    output = output.max(u.output_tokens.unwrap_or(0));
                    cost = u.reported_cost_usd.or(cost);
                }
                StreamChunk::CacheUsage(c) => cache = Some(c),
                _ => {}
            }
        }
        let cache = cache.unwrap_or_default();
        reads.push(cache.read_tokens);
        writes.push(cache.write_tokens);
        spent += cost.unwrap_or(0.02);
        println!(
            "adapter-with-cache round={round} input={input} output={output} cost={cost:?} read={} write={}",
            cache.read_tokens, cache.write_tokens
        );
        assert!(spent < CAP_USD, "cap reached");
    }
    println!("total_spent_usd~{spent:.5}");
    assert!(
        writes[0] > 9000,
        "la 1.ª petición debe escribir el prefijo: {writes:?}"
    );
    assert!(reads[1] > 9000, "la 2.ª debe leer el prefijo: {reads:?}");
    assert!(
        reads[2] > 9000,
        "la 3.ª debe leer el prefijo anterior: {reads:?}"
    );
}

#[test]
fn body_enables_code_execution_only_when_a_tool_is_eligible() {
    let provider = AnthropicProvider::new("http://localhost", "test-key");
    let body = provider.build_body(&request());
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["type"] == "code_execution_20260521"));
    assert_eq!(
        body["tools"][0]["allowed_callers"],
        json!(["direct", "code_execution_20260120"])
    );

    let mut direct = request();
    direct
        .tools
        .iter_mut()
        .for_each(|tool| tool.programmatic_allowed = false);
    assert!(
        AnthropicProvider::new("http://localhost", "test-key").build_body(&direct)["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tool| tool["type"] != "code_execution_20260521")
    );
}

#[test]
fn openrouter_haiku_ptc_is_pinned_and_direct_anthropic_body_is_unchanged() {
    let mut req = request();
    req.model = OPENROUTER_HAIKU_55.into();
    let openrouter = AnthropicProvider::new("https://openrouter.ai/api", "key");
    let body = openrouter.build_body(&req);
    assert_eq!(body["provider"]["order"], json!(["anthropic"]));
    assert_eq!(body["provider"]["allow_fallbacks"], false);
    assert!(body.get("temperature").is_none());
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["type"] == "code_execution_20260521"));

    let direct = AnthropicProvider::new("https://api.anthropic.com", "key").build_body(&req);
    assert!(direct.get("provider").is_none());
}

#[test]
fn openrouter_falls_back_only_on_explicit_bad_request_about_ptc() {
    use reqwest::StatusCode;

    assert!(is_openrouter_ptc_rejection(
        StatusCode::BAD_REQUEST,
        "code_execution is unsupported"
    ));
    assert!(!is_openrouter_ptc_rejection(
        StatusCode::BAD_REQUEST,
        "invalid model"
    ));
    assert!(!is_openrouter_ptc_rejection(
        StatusCode::INTERNAL_SERVER_ERROR,
        "code_execution unavailable"
    ));
    assert!(!is_openrouter_ptc_rejection(
        StatusCode::UNAUTHORIZED,
        "code_execution unsupported"
    ));
}

#[test]
fn programmatic_continuation_replays_provider_blocks_and_container_in_memory() {
    use crate::message::{ContentPart, Message, ProviderState, Role};
    let provider = AnthropicProvider::new("http://localhost", "test-key");
    let mut assistant = Message::assistant(vec![ContentPart::ToolCall {
        id: "tool-1".into(),
        name: "view".into(),
        input: json!({"path":"README.md"}),
    }]);
    let content = json!([
        {"type":"server_tool_use","id":"exec-1","name":"code_execution","input":{"code":"x = 1"}},
        {"type":"tool_use","id":"tool-1","name":"view","input":{"path":"README.md"},
            "caller":{"type":"code_execution_20260120","tool_id":"exec-1"}}
    ]);
    assistant.provider_state = Some(ProviderState {
        provider: "anthropic".into(),
        data: json!({"model":"claude-test","base_url":"http://localhost",
            "container_id":"container-1","assistant_content":content.clone()}),
    });
    let result = Message::new(
        Role::Tool,
        vec![ContentPart::ToolResult {
            call_id: "tool-1".into(),
            content: "file contents".into(),
            is_error: false,
        }],
    );
    let mut req = request();
    req.messages = vec![assistant, result];
    let body = provider.build_body(&req);
    assert_eq!(body["container"]["id"], "container-1");
    assert_eq!(body["messages"][0]["content"], content);
    assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "tool-1");
}

#[tokio::test]
async fn rejects_programmatic_calls_to_ineligible_tools() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/claude-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "capabilities":{"code_execution":{"supported":true}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason":"tool_use","container":{"id":"container-1"},
            "content":[{"type":"tool_use","id":"call-1","name":"write",
                "input":{},"caller":{"type":"code_execution_20260120","tool_id":"exec-1"}}]
        })))
        .mount(&server)
        .await;
    let provider = AnthropicProvider::new(server.uri(), "test-key");
    assert!(provider
        .stream(request(), CancellationToken::new())
        .await
        .is_err());
}

#[tokio::test]
async fn non_streaming_response_reports_cache_tokens() {
    use futures::StreamExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/claude-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "capabilities":{"code_execution":{"supported":true}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stop_reason":"end_turn",
            "content":[{"type":"text","text":"hola"}],
            "usage":{"input_tokens":12,"output_tokens":3,
                "cache_read_input_tokens":900,"cache_creation_input_tokens":40}
        })))
        .mount(&server)
        .await;
    let provider = AnthropicProvider::new(server.uri(), "test-key");
    let chunks: Vec<StreamChunk> = provider
        .stream(request(), CancellationToken::new())
        .await
        .unwrap()
        .collect()
        .await;
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::CacheUsage(cache) if cache.read_tokens == 900 && cache.write_tokens == 40
    )));
}

#[tokio::test]
async fn unverified_model_uses_direct_calls_and_reports_fallback() {
    use futures::StreamExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/claude-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "capabilities":{"code_execution":{"supported":false}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{}}}\n\n",
            "text/event-stream",
        ))
        .mount(&server)
        .await;
    let provider = AnthropicProvider::new(server.uri(), "test-key");
    let mut stream = provider
        .stream(request(), CancellationToken::new())
        .await
        .unwrap();
    let mut notice = false;
    while let Some(chunk) = stream.next().await {
        if matches!(chunk, StreamChunk::ProviderNotice(ref text) if text.contains("using direct tool calls"))
        {
            notice = true;
        }
    }
    assert!(notice);
    let sent = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
    assert!(body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["type"] != "code_execution_20260521"));
}

#[tokio::test]
async fn expired_programmatic_container_is_not_replayed_as_direct_calls() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/claude-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "capabilities":{"code_execution":{"supported":true}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error":{"message":"container expired"}
        })))
        .mount(&server)
        .await;

    let provider = AnthropicProvider::new(server.uri(), "test-key");
    assert!(provider
        .stream(request(), CancellationToken::new())
        .await
        .is_err());
    let sent = server.received_requests().await.unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent.iter()
            .filter(|request| request.method == "POST")
            .count(),
        1
    );
}
