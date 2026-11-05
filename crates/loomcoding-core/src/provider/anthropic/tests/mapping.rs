//! Tests de Anthropic: mapping.

use super::*;
use serde_json::json;

fn ev(name: &str, v: serde_json::Value) -> Vec<StreamChunk> {
    let mut acc = AnthropicAcc::default();
    map_anthropic_event(name, &v.to_string(), &mut acc)
}

#[test]
fn maps_text_delta() {
    let chunks = ev(
        "content_block_delta",
        json!({"index":0,"delta":{"type":"text_delta","text":"hi"}}),
    );
    assert!(matches!(chunks.as_slice(), [StreamChunk::TextDelta(t)] if t == "hi"));
}

#[test]
fn encodes_local_image_parts_as_anthropic_base64_blocks() {
    use crate::message::{ContentPart, Message};
    let mut message = Message::human("What is this?");
    message.parts.push(ContentPart::Image {
        mime_type: "image/png".into(),
        data: "cG5n".into(),
        filename: "sample.png".into(),
    });
    let wire = super::wire_messages(&[message], "model", "https://api.example");
    assert_eq!(wire[0]["content"][0]["text"], "What is this?");
    assert_eq!(wire[0]["content"][1]["source"]["type"], "base64");
    assert_eq!(wire[0]["content"][1]["source"]["media_type"], "image/png");
    assert_eq!(wire[0]["content"][1]["source"]["data"], "cG5n");
}

#[test]
fn maps_provider_reported_cost_from_usage() {
    let chunks = ev(
        "message_delta",
        json!({"usage":{"output_tokens":2,"cost":0.002}}),
    );
    assert!(
        matches!(&chunks[0], StreamChunk::UsageUpdate(usage) if usage.reported_cost_usd == Some(0.002) && usage.final_update)
    );
}

#[test]
fn assembles_tool_use() {
    let mut acc = AnthropicAcc::default();
    let start = json!({"index":0,"content_block":{"type":"tool_use","id":"t1","name":"view"}});
    let d1 = json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}});
    let d2 = json!({"index":0,"delta":{"type":"input_json_delta","partial_json":"\"a.rs\"}"}});
    map_anthropic_event("content_block_start", &start.to_string(), &mut acc);
    map_anthropic_event("content_block_delta", &d1.to_string(), &mut acc);
    map_anthropic_event("content_block_delta", &d2.to_string(), &mut acc);
    let stop = json!({"delta":{"stop_reason":"tool_use"}});
    let chunks = map_anthropic_event("message_delta", &stop.to_string(), &mut acc);
    assert!(
        matches!(chunks.first(), Some(StreamChunk::ToolCallComplete { name, input, .. })
        if name == "view" && input["path"] == "a.rs")
    );
    assert!(matches!(
        chunks.last(),
        Some(StreamChunk::Done(FinishReason::ToolCalls))
    ));
}

#[test]
fn message_start_reports_cache_tokens_when_the_provider_informs_them() {
    let chunks = ev(
        "message_start",
        json!({"message":{"usage":{"input_tokens":12,"output_tokens":0,
            "cache_read_input_tokens":900,"cache_creation_input_tokens":40}}}),
    );
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::CacheUsage(cache) if cache.read_tokens == 900 && cache.write_tokens == 40
    )));
    // El conteo de entrada no se altera: Anthropic lo informa sin la caché.
    assert!(chunks
        .iter()
        .any(|chunk| matches!(chunk, StreamChunk::UsageUpdate(u) if u.input_tokens == Some(12))));
}

#[test]
fn message_start_without_cache_fields_emits_no_cache_chunk() {
    let chunks = ev(
        "message_start",
        json!({"message":{"usage":{"input_tokens":12,"output_tokens":0}}}),
    );
    assert!(!chunks
        .iter()
        .any(|chunk| matches!(chunk, StreamChunk::CacheUsage(_))));
}

#[test]
fn message_delta_reports_cache_tokens_too() {
    let chunks = ev(
        "message_delta",
        json!({"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7,
            "cache_read_input_tokens":5}}),
    );
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::CacheUsage(cache) if cache.read_tokens == 5 && cache.write_tokens == 0
    )));
}

#[test]
fn cache_usage_ignores_values_that_are_not_non_negative_integers() {
    assert_eq!(
        cache_usage(&json!({"cache_read_input_tokens":"9","cache_creation_input_tokens":-3})),
        None
    );
    assert_eq!(cache_usage(&json!({"input_tokens":4})), None);
}

#[test]
fn cache_usage_keeps_a_single_reported_field() {
    let cache = cache_usage(&json!({"cache_creation_input_tokens":21})).unwrap();
    assert_eq!((cache.read_tokens, cache.write_tokens), (0, 21));
}

#[test]
fn maps_message_start_usage() {
    let chunks = ev(
        "message_start",
        json!({"message":{"usage":{"input_tokens":12,"output_tokens":0}}}),
    );
    assert!(
        matches!(chunks.as_slice(), [StreamChunk::UsageUpdate(u)] if u.input_tokens == Some(12) && !u.final_update)
    );
}

#[test]
fn an_error_event_becomes_an_error_chunk() {
    let chunks = ev(
        "error",
        json!({"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}),
    );
    assert!(
        matches!(chunks.as_slice(), [StreamChunk::Error(text)]
            if text.contains("overloaded_error") && text.contains("Overloaded")),
        "{chunks:?}"
    );
}

#[test]
fn an_error_payload_without_an_event_name_is_still_an_error() {
    let chunks = ev(
        "",
        json!({"type":"error","error":{"type":"api_error","message":"the engine stopped"}}),
    );
    assert!(
        matches!(chunks.as_slice(), [StreamChunk::Error(text)] if text.contains("the engine stopped")),
        "{chunks:?}"
    );
}

#[test]
fn a_huge_anthropic_stream_error_is_bounded() {
    let chunks = ev(
        "error",
        json!({"type":"error","error":{"type":"api_error","message":"x".repeat(5000)}}),
    );
    let [StreamChunk::Error(text)] = chunks.as_slice() else {
        panic!("se esperaba un único error: {chunks:?}");
    };
    assert!(text.chars().count() <= 600, "{}", text.chars().count());
}
