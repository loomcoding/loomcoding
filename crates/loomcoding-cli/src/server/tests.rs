use super::*;

#[test]
fn validates_image_prompts_before_accepting_them_over_ipc() {
    use base64::Engine as _;
    let mut prompt = loomcoding_tui::app::PendingPrompt {
        text: "describe this".into(),
        images: vec![loomcoding_core::message::ContentPart::Image {
            mime_type: "image/png".into(),
            data: base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\nsmall"),
            filename: "sample.png".into(),
        }],
    };
    assert!(validate_user_prompt(&prompt).is_ok());
    let loomcoding_core::message::ContentPart::Image { data, .. } = &mut prompt.images[0] else {
        unreachable!();
    };
    *data = "not-base64".into();
    assert!(validate_user_prompt(&prompt).is_err());
}

#[test]
fn runtime_peer_card_url_uses_a_direct_standard_card_path() {
    assert_eq!(
        runtime_card_url("http://127.0.0.1:7077/a2a", "tenant"),
        "http://127.0.0.1:7077/agents/tenant/.well-known/agent-card.json"
    );
}

#[test]
fn runtime_peer_name_uses_project_folder_and_disambiguates_duplicates() {
    assert_eq!(
        runtime_peer_name(
            std::path::Path::new("E:/work/My Animation Lab"),
            false,
            "0123456789abcdef"
        ),
        "my-animation-lab"
    );
    assert_eq!(
        runtime_peer_name(
            std::path::Path::new("E:/work/Animation"),
            true,
            "0123456789abcdef"
        ),
        "animation-01234567"
    );
}

#[test]
fn runtime_peer_profile_is_a_remote_profile_without_secrets_or_paths() {
    let profile = runtime_peer_profile(
        "runtime-0123456789ab",
        "Project animation agent",
        "http://127.0.0.1:7077/agents/0123456789abcdef/.well-known/agent-card.json",
        "LOOMCODING_A2A_TOKEN",
    );
    let (parsed, _) =
        loomcoding_core::registry::parse_profile("runtime-0123456789ab", &profile).unwrap();
    assert_eq!(
        parsed.remote_endpoint.as_deref(),
        Some("http://127.0.0.1:7077/agents/0123456789abcdef/.well-known/agent-card.json")
    );
    assert!(!profile.contains("E:\\workspace"));
    assert!(!profile.contains("secret-value"));
}

#[test]
fn local_token_comparison_requires_a_full_matching_token() {
    let token = "ab".repeat(32);
    assert!(constant_time_eq(&token, &token));
    assert!(!constant_time_eq(&token, &"ac".repeat(32)));
    assert!(!constant_time_eq(&token, "short"));
}

#[test]
fn ipc_events_are_bounded_before_serializing() {
    let event = LiveEvent::Error {
        message: "x".repeat(1_000_000),
    };
    let bounded = bounded_wire(event);
    let encoded = serde_json::to_vec(&bounded).unwrap();
    assert!(encoded.len() < ipc::MAX_FRAME_BYTES);
}
