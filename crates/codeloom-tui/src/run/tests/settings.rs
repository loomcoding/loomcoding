use super::*;

#[test]
fn settings_save_waits_for_idle_and_freezes_values_until_reply() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::OpenAiCompat,
        model: "old-model".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    app.provider_settings.as_mut().unwrap().model = "new-model".into();
    app.turn_in_flight = true;
    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(!app.settings_save_requested);
    assert!(app
        .settings_feedback
        .as_ref()
        .unwrap()
        .contains("active turn"));

    app.turn_in_flight = false;
    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut pending = None;
    send_provider_settings(&mut app, &tx, &mut pending);
    let command = rx.try_recv().unwrap();
    assert_eq!(command.settings.model, "new-model");
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_event(&mut app, Event::Paste("other-model".into()));
    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(!app.settings_editing);
    assert!(!app.settings_save_requested);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().kind,
        command.settings.kind
    );
    assert_eq!(app.model, "old-model");

    command
        .reply
        .send(Err("Cannot write config".into()))
        .unwrap();
    app.settings_saved(pending.take().unwrap().try_recv().unwrap());
    assert_eq!(app.model, "old-model");
    assert!(!app.settings_saving);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Cannot write config")
    );

    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    send_provider_settings(&mut app, &tx, &mut pending);
    rx.try_recv().unwrap().reply.send(Ok(true)).unwrap();
    app.settings_saved(pending.take().unwrap().try_recv().unwrap());
    assert_eq!(app.model, "new-model");
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Settings saved and applied")
    );

    drop(rx);
    app.request_settings_save();
    send_provider_settings(&mut app, &tx, &mut pending);
    assert!(pending.is_none());
    assert!(!app.settings_saving);
    assert_eq!(
        app.settings_feedback.as_deref(),
        Some("Settings worker is unavailable")
    );
}

#[test]
fn permission_interrupts_overlays_and_ignores_paste_and_abandoned_requests() {
    let mut app = App::new();
    app.help_open = true;
    handle_event(&mut app, Event::Paste("hidden input".into()));
    assert!(app.composer.is_empty());
    app.settings_open = true;
    app.settings_editing = true;
    let mut first = open_dialog(&mut app);
    let abandoned = open_dialog(&mut app);
    let mut last = open_dialog(&mut app);
    drop(abandoned);
    assert!(!app.help_open);
    assert!(!app.settings_open);
    assert!(!app.settings_editing);
    handle_event(&mut app, Event::Paste("hidden input".into()));
    assert!(app.composer.is_empty());
    handle_key(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(
        first.try_recv().unwrap(),
        codeloom_core::permission::Decision::Deny
    );
    assert_eq!(
        last.try_recv().unwrap(),
        codeloom_core::permission::Decision::Allow
    );
    assert!(app.permission.is_none());
}

#[test]
fn provider_settings_edit_values_and_request_save_without_touching_prompt() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::OpenAiCompat,
        model: "old-model".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    });
    handle_key(&mut app, KeyCode::F(2), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().kind,
        codeloom_core::config::ProviderKind::Anthropic
    );
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::End, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('2'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "old-model2");
    assert!(app.settings_save_requested);
    assert!(app.composer.is_empty());
}

#[test]
fn esc_in_settings_warns_before_discarding_changes() {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::OpenAiCompat,
        model: "old-model".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    app.provider_settings.as_mut().unwrap().model = "new-model".into();
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.settings_open);
    assert!(app
        .settings_feedback
        .as_deref()
        .unwrap()
        .contains("Esc again"));
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.settings_open);
    assert_eq!(app.provider_settings.as_ref().unwrap().model, "old-model");
}

#[tokio::test]
async fn catalog_update_is_applied_by_the_loop() {
    use codeloom_core::catalog::Catalog;

    let sample = r#"{"openrouter": {"id": "openrouter", "name": "OpenRouter",
        "npm": "@openrouter/ai-sdk-provider", "api": "https://openrouter.ai/api/v1",
        "env": ["OPENROUTER_API_KEY"],
        "models": {"m": {"id": "m", "tool_call": true}}}}"#;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(Arc::new(Catalog::from_json(sample).unwrap()))
        .expect("el canal sigue abierto");

    let mut app = App::new();
    let update = rx.recv().await.expect("el catálogo llega por el canal");
    apply_catalog(&mut app, update);

    assert_eq!(app.catalog.providers.len(), 1);
    assert_eq!(app.catalog.providers[0].id, "openrouter");
}

fn anthropic_app_with_catalog() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::Anthropic,
        model: "model-a".into(),
        base_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(crate::app::picker_sample_catalog());
    app.open_settings();
    app
}

#[test]
fn ctrl_l_opens_the_picker_for_the_focused_field() {
    let mut app = anthropic_app_with_catalog();
    app.settings_field = 1;
    handle_key(&mut app, KeyCode::Char('l'), KeyModifiers::CONTROL);
    assert_eq!(app.settings_picker, Some(SettingsPicker::Model));
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.settings_picker.is_none());
    assert!(app.settings_open, "Esc cierra solo el selector");

    app.settings_field = 0;
    handle_key(&mut app, KeyCode::Char('l'), KeyModifiers::CONTROL);
    assert_eq!(app.settings_picker, Some(SettingsPicker::Provider));
}

#[test]
fn enter_on_model_opens_the_model_picker_when_the_catalog_is_loaded() {
    let mut app = anthropic_app_with_catalog();
    app.settings_field = 1;
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.settings_picker, Some(SettingsPicker::Model));
}

#[test]
fn enter_on_model_edits_by_hand_without_a_catalog() {
    let mut app = anthropic_app_with_catalog();
    app.set_catalog(std::sync::Arc::new(
        codeloom_core::catalog::Catalog::default(),
    ));
    app.settings_field = 1;
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.settings_picker.is_none());
    assert!(app.settings_editing);
}

#[test]
fn enter_on_provider_opens_picker_and_arrows_still_toggle_kind() {
    let mut app = anthropic_app_with_catalog();
    app.settings_field = 0;
    app.settings_api_key = "old-provider-secret".into();
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.settings_picker, Some(SettingsPicker::Provider));
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().kind,
        codeloom_core::config::ProviderKind::Gemini
    );
    assert_eq!(
        app.provider_settings.as_ref().unwrap().base_url,
        "https://generativelanguage.googleapis.com/v1beta"
    );
    assert_eq!(
        app.provider_settings.as_ref().unwrap().api_key_env,
        "GEMINI_API_KEY"
    );
    assert!(app.settings_api_key.is_empty());
}

#[test]
fn provider_picker_exposes_responses_and_gemini_with_their_native_endpoints() {
    let mut app = anthropic_app_with_catalog();
    app.settings_field = 0;
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    for character in "gemini-interactions".chars() {
        handle_key(&mut app, KeyCode::Char(character), KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);

    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.kind, codeloom_core::config::ProviderKind::Gemini);
    assert_eq!(
        settings.base_url,
        "https://generativelanguage.googleapis.com/v1beta"
    );
    assert_eq!(settings.api_key_env, "GEMINI_API_KEY");
    assert_eq!(app.settings_field, 1);
    assert!(app
        .settings_feedback
        .as_deref()
        .unwrap()
        .contains("model ID"));
}

#[test]
fn picker_keys_filter_move_and_choose() {
    let mut app = anthropic_app_with_catalog();
    app.settings_field = 0;
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    for c in "open".chars() {
        handle_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.base_url, "https://openrouter.ai/api/v1");
    assert_eq!(settings.api_key_env, "OPENROUTER_API_KEY");
    // Elegido el proveedor, se abre la lista de sus modelos.
    assert_eq!(app.settings_picker, Some(SettingsPicker::Model));
    assert!(app.settings_open);
}

/// Diálogo abierto sobre `model` del catálogo de razonamiento, sin esfuerzo.
fn reasoning_settings_app(model: &str) -> App {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        kind: codeloom_core::config::ProviderKind::Anthropic,
        model: model.into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    app
}

#[test]
fn effort_field_is_reached_with_arrows_and_cycled_with_left_right() {
    let mut app = reasoning_settings_app("claude-think");
    app.settings_field = 3;
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.settings_field, SETTINGS_EFFORT_FIELD);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().effort.as_deref(),
        Some("low")
    );
    handle_key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(
        app.provider_settings.as_ref().unwrap().effort.as_deref(),
        Some("high"),
        "←/→ cicla también hacia atrás, desde «default»"
    );
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.settings_field, SETTINGS_EFFORT_FIELD + 1);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.settings_save_requested, "Enter sobre Save guarda");
}

#[test]
fn settings_without_effort_field_go_straight_to_save() {
    let mut app = reasoning_settings_app("claude-plain");
    app.settings_field = 3;
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.settings_field, SETTINGS_EFFORT_FIELD);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.settings_save_requested);
}
