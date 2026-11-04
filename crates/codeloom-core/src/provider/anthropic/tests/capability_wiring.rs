//! Tests de Anthropic: capability wiring.

use super::*;

#[test]
fn a_ptc_rejection_is_remembered_per_model_and_not_retried_every_turn() {
    let provider = AnthropicProvider::new("https://openrouter.ai/api", "key");
    assert!(!provider.ptc_rejected(OPENROUTER_HAIKU_55));
    provider.note_ptc_rejection(OPENROUTER_HAIKU_55, 400);
    assert!(provider.ptc_rejected(OPENROUTER_HAIKU_55));
    assert!(!provider.ptc_rejected("anthropic/claude-other"));
}

#[test]
fn code_execution_metadata_is_cached_per_target() {
    let provider = AnthropicProvider::new("https://api.anthropic.com/", "key");
    let target = provider.target("claude-x");
    assert_eq!(target.base_url, "https://api.anthropic.com");
    provider.note_code_execution("claude-x", true);
    assert_eq!(provider.cached_code_execution("claude-x"), Some(true));
    assert_eq!(provider.cached_code_execution("claude-y"), None);
}

#[test]
fn ptc_incompatibility_is_recognised_through_the_shared_classifier() {
    use reqwest::StatusCode;
    assert!(is_openrouter_ptc_rejection(
        StatusCode::BAD_REQUEST,
        "Unknown field allowed_callers"
    ));
    assert!(!is_openrouter_ptc_rejection(
        StatusCode::BAD_REQUEST,
        "messages.2: unexpected role"
    ));
}
