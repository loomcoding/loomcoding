//! Tests de Anthropic: wire.

use super::*;
use crate::message::{ContentPart, Message, Role};
use serde_json::json;

#[test]
fn serializes_tool_use_and_tool_result_blocks() {
    let p = AnthropicProvider::new("http://x", "k");
    let req = ProviderRequest {
        model: "m".into(),
        system: "sys".into(),
        messages: vec![
            Message::assistant(vec![ContentPart::ToolCall {
                id: "t1".into(),
                name: "view".into(),
                input: json!({"path":"a"}),
            }]),
            Message::new(
                Role::Tool,
                vec![ContentPart::ToolResult {
                    call_id: "t1".into(),
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
    let blocks = assistant["content"].as_array().unwrap();
    assert!(blocks
        .iter()
        .any(|b| b["type"] == "tool_use" && b["id"] == "t1" && b["name"] == "view"));
    let user = msgs
        .iter()
        .find(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_array()
                    .map(|a| a.iter().any(|b| b["type"] == "tool_result"))
                    .unwrap_or(false)
        })
        .unwrap();
    let result = user["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_result")
        .unwrap();
    assert_eq!(result["tool_use_id"], "t1");
    assert_eq!(result["content"], "ok");
}

#[test]
fn moves_system_history_into_the_top_level_system_field() {
    let p = AnthropicProvider::new("http://x", "k");
    let mut req = ProviderRequest {
        model: "m".into(),
        system: "base instructions".into(),
        messages: vec![
            Message::new(
                Role::System,
                vec![ContentPart::Text {
                    text: "later instruction".into(),
                }],
            ),
            Message::user("hello"),
        ],
        tools: vec![],
        temperature: 0.0,
        max_tokens: 16,
        effort: None,
        programmatic_tool_calling: false,
    };
    let body = p.build_body(&req);
    assert_eq!(body["system"], "base instructions\n\nlater instruction");
    assert_eq!(
        body["messages"],
        json!([{ "role": "user", "content": "hello" }])
    );
    req.system.clear();
    assert_eq!(p.build_body(&req)["system"], "later instruction");
}

#[test]
fn anthropic_http_error_includes_a_bounded_provider_message() {
    let message = http_error(
        reqwest::StatusCode::BAD_REQUEST,
        &json!({"error":{"message":"invalid messages[0].role"}}).to_string(),
    );
    assert!(message.contains("HTTP 400: invalid messages[0].role"));
    let long = "x".repeat(600);
    assert!(
        http_error(
            reqwest::StatusCode::BAD_REQUEST,
            &json!({"error":{"message":long}}).to_string()
        )
        .chars()
        .count()
            < 560
    );
}
