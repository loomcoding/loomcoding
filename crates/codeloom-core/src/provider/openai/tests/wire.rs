//! Tests: wire.

use super::*;

#[test]
fn serializes_tool_calls_and_tool_results() {
    let p = OpenAiCompatProvider::new("http://x", "k");
    let req = ProviderRequest {
        model: "m".into(),
        system: "sys".into(),
        messages: vec![
            Message::assistant(vec![ContentPart::ToolCall {
                id: "c1".into(),
                name: "view".into(),
                input: serde_json::json!({"path":"a"}),
            }]),
            Message::new(
                Role::Tool,
                vec![ContentPart::ToolResult {
                    call_id: "c1".into(),
                    content: "ok".into(),
                    is_error: false,
                }],
            ),
        ],
        tools: vec![],
        temperature: 0.0,
        max_tokens: 16,
        effort: None,
        programmatic_tool_calling: false,
    };
    let body = p.build_body(&req);
    let msgs = body["messages"].as_array().unwrap();
    let assistant = msgs.iter().find(|m| m["role"] == "assistant").unwrap();
    assert_eq!(assistant["tool_calls"][0]["id"], "c1");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "view");
    assert!(assistant["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap()
        .contains("path"));
    let tool = msgs.iter().find(|m| m["role"] == "tool").unwrap();
    assert_eq!(tool["tool_call_id"], "c1");
    assert_eq!(tool["content"], "ok");
}

#[test]
fn body_requests_usage_in_the_stream() {
    let p = OpenAiCompatProvider::new("http://x", "k");
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
    let body = p.build_body(&req);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn oversized_tool_results_from_old_history_are_capped_on_the_wire() {
    use crate::message::{ContentPart, Message, Role};
    let big = "x".repeat(crate::tools::MAX_TOOL_OUTPUT_BYTES * 4);
    let msgs = vec![Message::new(
        Role::Tool,
        vec![ContentPart::ToolResult {
            call_id: "c1".into(),
            content: big.clone(),
            is_error: false,
        }],
    )];
    let wire = super::wire_messages(&msgs);
    let sent = wire[0]["content"].as_str().unwrap();
    assert!(sent.len() < big.len());
    // ADAPTADO: el resultado se recorta ahora con cabeza y cola y el marcador «bytes omitted».
    assert!(sent.contains("bytes omitted ...]"));
}

#[test]
fn encodes_local_image_parts_as_openai_data_urls() {
    use crate::message::{ContentPart, Message};
    let mut message = Message::human("What is this?");
    message.parts.push(ContentPart::Image {
        mime_type: "image/png".into(),
        data: "cG5n".into(),
        filename: "sample.png".into(),
    });
    let wire = super::wire_messages(&[message]);
    assert_eq!(wire[0]["content"][0]["text"], "What is this?");
    assert_eq!(
        wire[0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,cG5n"
    );
}
