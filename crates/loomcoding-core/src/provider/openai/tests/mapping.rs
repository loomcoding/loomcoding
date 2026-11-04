//! Tests: mapping.

use super::*;

#[test]
fn maps_text_delta() {
    let mut acc = ToolCallAcc::default();
    let data = json!({"choices":[{"delta":{"content":"hi"}}]}).to_string();
    let chunks = map_openai_event(&data, &mut acc);
    assert!(matches!(chunks.as_slice(), [StreamChunk::TextDelta(t)] if t == "hi"));
}

#[test]
fn usage_reports_cached_tokens_and_cache_writes() {
    let mut acc = ToolCallAcc::default();
    let chunks = map_openai_event(
        r#"{"usage":{"prompt_tokens":1000,"completion_tokens":2,
            "prompt_tokens_details":{"cached_tokens":800,"cache_write_tokens":50}}}"#,
        &mut acc,
    );
    // El fragmento de uso conserva el índice 0 y el de caché va a continuación.
    assert!(matches!(&chunks[0], StreamChunk::Usage(usage) if usage.input_tokens == 1000));
    assert!(matches!(
        &chunks[1],
        StreamChunk::CacheUsage(cache) if cache.read_tokens == 800 && cache.write_tokens == 50
    ));
}

#[test]
fn usage_reports_deepseek_cache_hits() {
    let mut acc = ToolCallAcc::default();
    let chunks = map_openai_event(
        r#"{"usage":{"prompt_tokens":500,"completion_tokens":1,"prompt_cache_hit_tokens":300}}"#,
        &mut acc,
    );
    assert!(chunks.iter().any(|chunk| matches!(
        chunk,
        StreamChunk::CacheUsage(cache) if cache.read_tokens == 300 && cache.write_tokens == 0
    )));
}

#[test]
fn usage_without_cache_details_emits_no_cache_chunk() {
    let mut acc = ToolCallAcc::default();
    let chunks = map_openai_event(
        r#"{"usage":{"prompt_tokens":4,"completion_tokens":2}}"#,
        &mut acc,
    );
    assert_eq!(chunks.len(), 1);
    assert!(matches!(&chunks[0], StreamChunk::Usage(_)));
}

#[test]
fn cache_usage_ignores_negative_and_non_numeric_values() {
    assert_eq!(
        cache_usage(&serde_json::json!({
            "prompt_tokens_details":{"cached_tokens":-1,"cache_write_tokens":"x"}
        })),
        None
    );
}

#[test]
fn maps_reported_usage_cost_and_rejects_invalid_cost() {
    let mut acc = ToolCallAcc::default();
    let chunks = map_openai_event(
        r#"{"usage":{"prompt_tokens":4,"completion_tokens":2,"cost":0.0012}}"#,
        &mut acc,
    );
    assert!(
        matches!(&chunks[0], StreamChunk::Usage(usage) if usage.reported_cost_usd == Some(0.0012))
    );
    let chunks = map_openai_event(r#"{"usage":{"cost":-1}}"#, &mut acc);
    assert!(matches!(&chunks[0], StreamChunk::Usage(usage) if usage.reported_cost_usd.is_none()));
}

#[test]
fn assembles_tool_call_across_events() {
    let mut acc = ToolCallAcc::default();
    let args1 = r#"{"path":"#;
    let args2 = r#""a.rs"}"#;
    let e1 = json!({"choices":[{"delta":{"tool_calls":[
        {"index":0,"id":"c1","function":{"name":"view","arguments":args1}}]}}]})
    .to_string();
    let e2 = json!({"choices":[{"delta":{"tool_calls":[
        {"index":0,"function":{"arguments":args2}}]}}]})
    .to_string();
    let e3 = json!({"choices":[{"finish_reason":"tool_calls"}]}).to_string();
    assert!(map_openai_event(&e1, &mut acc).is_empty());
    assert!(map_openai_event(&e2, &mut acc).is_empty());
    let chunks = map_openai_event(&e3, &mut acc);
    assert!(
        matches!(chunks.first(), Some(StreamChunk::ToolCallComplete { name, input, .. })
        if name == "view" && input["path"] == "a.rs")
    );
    assert!(matches!(
        chunks.last(),
        Some(StreamChunk::Done(FinishReason::ToolCalls))
    ));
}
