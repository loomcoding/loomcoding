//! Tests de Anthropic: thinking replay.

use super::*;
use serde_json::json;

fn feed(acc: &mut AnthropicAcc, events: &[(&str, serde_json::Value)]) -> Vec<StreamChunk> {
    events
        .iter()
        .flat_map(|(name, data)| map_anthropic_event(name, &data.to_string(), acc))
        .collect()
}

/// Respuesta en streaming con razonamiento y una llamada a herramienta.
fn thinking_then_tool_use() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "content_block_start",
            json!({"index":0,"content_block":{"type":"thinking","thinking":""}}),
        ),
        (
            "content_block_delta",
            json!({"index":0,"delta":{"type":"thinking_delta","thinking":"Voy a "}}),
        ),
        (
            "content_block_delta",
            json!({"index":0,"delta":{"type":"thinking_delta","thinking":"mirar."}}),
        ),
        (
            "content_block_delta",
            json!({"index":0,"delta":{"type":"signature_delta","signature":"firma-abc"}}),
        ),
        (
            "content_block_start",
            json!({"index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"view"}}),
        ),
        (
            "content_block_delta",
            json!({"index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.rs\"}"}}),
        ),
        (
            "message_delta",
            json!({"delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}),
        ),
    ]
}

#[test]
fn streamed_thinking_is_kept_so_the_next_request_can_replay_it() {
    // Con `thinking` activo, Messages exige que el turno del asistente con `tool_use`
    // reenvíe su bloque de razonamiento con la firma; de lo contrario responde 400.
    let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
    let chunks = feed(&mut acc, &thinking_then_tool_use());
    let state = chunks
        .into_iter()
        .find_map(|chunk| match chunk {
            StreamChunk::ProviderState(state) => Some(state),
            _ => None,
        })
        .expect("falta el estado con el razonamiento firmado");
    assert_eq!(state.provider, "anthropic");
    assert_eq!(state.data["model"], "claude-test");
    assert_eq!(state.data["base_url"], "https://api.anthropic.com");
    assert_eq!(
        state.data["assistant_content"],
        json!([
            {"type":"thinking","thinking":"Voy a mirar.","signature":"firma-abc"},
            {"type":"tool_use","id":"toolu_1","name":"view","input":{"path":"a.rs"}}
        ])
    );
}

#[test]
fn replayed_assistant_turn_starts_with_the_thinking_block() {
    let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
    let state = feed(&mut acc, &thinking_then_tool_use())
        .into_iter()
        .find_map(|chunk| match chunk {
            StreamChunk::ProviderState(state) => Some(state),
            _ => None,
        })
        .unwrap();
    let mut assistant = crate::message::Message::assistant(vec![]);
    assistant.provider_state = Some(state);
    let wire = wire_messages(&[assistant], "claude-test", "https://api.anthropic.com");
    assert_eq!(wire[0]["content"][0]["type"], "thinking");
    assert_eq!(wire[0]["content"][0]["signature"], "firma-abc");
}

#[test]
fn without_thinking_no_state_is_emitted() {
    let mut acc = AnthropicAcc::for_provider("claude-test", "https://api.anthropic.com");
    let chunks = feed(
        &mut acc,
        &[
            (
                "content_block_start",
                json!({"index":0,"content_block":{"type":"tool_use","id":"t","name":"view"}}),
            ),
            (
                "content_block_delta",
                json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}),
            ),
            ("message_delta", json!({"delta":{"stop_reason":"tool_use"}})),
        ],
    );
    assert!(!chunks
        .iter()
        .any(|chunk| matches!(chunk, StreamChunk::ProviderState(_))));
}
