use super::*;

#[test]
fn slash_controls_pause_cancel_resume_and_sidebar_without_dispatching_commands() {
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.turn_in_flight = true;
    let token = CancellationToken::new();
    let active = Arc::new(Mutex::new(Some(token.clone())));
    send(&mut app, &tx, Some("pending work".into()));
    for text in ["/pause", "/pause"] {
        app.composer.set_text(text);
        let prompt = handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        send(&mut app, &tx, prompt);
        assert!(app.queue_paused);
        assert!(
            !token.is_cancelled(),
            "pause does not cancel the active turn"
        );
        assert!(rx.try_recv().is_err());
    }
    app.composer.set_text("/cancel");
    let prompt = handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    send(&mut app, &tx, prompt);
    cancel_active_turn(&mut app, &active);
    assert!(token.is_cancelled());
    app.apply(loomcoding_core::event::LiveEvent::TurnEnded);
    dispatch_next(&mut app, &tx);
    assert!(rx.try_recv().is_err());
    assert_eq!(app.queued, 1);
    app.composer.set_text("/resume");
    let prompt = handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    send(&mut app, &tx, prompt);
    assert!(!app.queue_paused);
    assert_eq!(rx.try_recv().unwrap(), "pending work");
    assert!(rx.try_recv().is_err());
    for expanded in [true, false] {
        app.composer.set_text("/sidebar");
        assert_eq!(
            handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
            None
        );
        assert_eq!(app.sidebar_expanded, expanded);
    }
    app.apply(loomcoding_core::event::LiveEvent::TurnEnded);
    app.composer.set_text("/cancel");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("No active turn to cancel")
    );
    assert_eq!(app.composer.text(), "/cancel");
    assert!(!app.cancel_requested);
    assert!(!app.queue_paused);
}

#[test]
fn turn_completion_discards_cancellation_that_arrived_after_the_worker_finished() {
    for event in [
        loomcoding_core::event::LiveEvent::TurnEnded,
        loomcoding_core::event::LiveEvent::Error {
            message: "failed".into(),
        },
    ] {
        let mut app = App::new();
        app.turn_in_flight = true;
        app.composer.set_text("/cancel");
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(app.cancel_requested);
        app.apply(event);
        let next = CancellationToken::new();
        let active = Arc::new(Mutex::new(Some(next.clone())));
        app.turn_in_flight = true;
        cancel_active_turn(&mut app, &active);
        assert!(!next.is_cancelled());
        assert!(!app.cancel_requested);
        assert!(app.queue_paused);
    }
}

#[test]
fn slash_menu_filters_completes_and_executes_without_sending_a_prompt() {
    let mut app = App::new();
    handle_event(&mut app, Event::Paste("/s".into()));
    // `/sidebar` empieza por «s» pero es un alias oculto: el popup muestra solo sessions y settings.
    assert_eq!(app.slash_matches().len(), 2);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "/settings");
    assert_eq!(app.focus, Focus::Composer);
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "test".into(),
        base_url: "https://example.test".into(),
        api_key_env: "KEY".into(),
        effort: None,
    });
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.settings_open);
    assert!(app.composer.is_empty());
    assert!(app.pending_prompts.is_empty());
    assert_eq!(app.queued, 0);
    assert!(!app.history.is_browsing());
}

#[test]
fn queued_prompt_keeps_its_image_parts_until_dispatch() {
    let mut app = App::new();
    app.turn_in_flight = true;
    app.draft_images
        .push(loomcoding_core::message::ContentPart::Image {
            mime_type: "image/png".into(),
            data: "cG5n".into(),
            filename: "sample.png".into(),
        });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    send(&mut app, &tx, Some("describe this".into()));
    assert_eq!(app.pending_prompts[0].images.len(), 1);
    app.turn_in_flight = false;
    dispatch_next(&mut app, &tx);
    let prompt = rx.try_recv().unwrap();
    assert_eq!(prompt.text, "describe this");
    assert_eq!(prompt.images.len(), 1);
}

#[test]
fn slash_commands_share_existing_actions_and_session_guards() {
    for text in ["/help", "/new", "/sessions", "/queue", "/tools", "/quit"] {
        let mut app = App::new();
        app.composer.set_text(text);
        assert_eq!(
            handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
            None
        );
        match text {
            "/help" => assert!(app.help_open),
            "/new" => {
                assert_eq!(app.session_choice, Some(SessionChoice::New));
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                send_session_commands(&mut app, &tx);
                assert!(matches!(
                    rx.try_recv().unwrap(),
                    SessionCommand::Select(SessionChoice::New)
                ));
            }
            "/sessions" => assert!(app.session_list_refresh_requested),
            "/queue" => assert!(app.queue_open),
            "/tools" => assert!(app.tools_open),
            _ => assert!(app.should_quit),
        }
        assert!(app.composer.is_empty());
    }
    for text in ["/new", "/sessions"] {
        let mut app = App::new();
        app.turn_in_flight = true;
        app.composer.set_text(text);
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.composer.text(), text);
        assert!(!app.session_loading);
        assert!(app.session_choice.is_none());
        assert!(app
            .slash_feedback
            .as_deref()
            .unwrap()
            .contains("active turn"));
    }
}

/// App con sesión abierta y `/compact` escrito en el composer.
fn app_con_compact() -> App {
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(1));
    app.composer.set_text("/compact");
    app
}

#[test]
fn slash_compact_requests_a_compaction_and_dispatches_it_once() {
    let mut app = app_con_compact();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None,
        "el comando no se envía como prompt"
    );
    assert!(app.compact_requested);
    assert!(app.composer.is_empty());

    dispatch_compact(&mut app, &tx);
    assert!(rx.try_recv().is_ok(), "el trabajador recibe la petición");
    assert!(app.compact_in_flight && !app.compact_requested);
    // Una segunda pasada no vuelve a pedirla.
    dispatch_compact(&mut app, &tx);
    assert!(rx.try_recv().is_err());
}

#[test]
fn slash_compact_is_refused_without_session_or_during_a_turn_and_keeps_the_text() {
    type Preparacion = fn(&mut App);
    let casos: [(&str, Preparacion); 3] = [
        ("No session", |app| app.session_id = None),
        ("active turn", |app| app.turn_in_flight = true),
        ("Already compacting", |app| app.compact_in_flight = true),
    ];
    for (esperado, preparar) in casos {
        let mut app = app_con_compact();
        preparar(&mut app);
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!app.compact_requested, "{esperado}");
        assert_eq!(app.composer.text(), "/compact", "{esperado}");
        assert!(
            app.slash_feedback.as_deref().unwrap().contains(esperado),
            "{esperado}: {:?}",
            app.slash_feedback
        );
    }
}

#[test]
fn a_compaction_in_flight_holds_the_prompt_queue_until_it_ends() {
    use loomcoding_core::event::{LiveEvent, ManualCompaction};
    let mut app = app_con_compact();
    let (compact_tx, _compact_rx) = tokio::sync::mpsc::unbounded_channel();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    dispatch_compact(&mut app, &compact_tx);
    app.pending_prompts.push_back("hola".into());

    dispatch_next(&mut app, &tx);
    assert!(rx.try_recv().is_err(), "el prompt espera a la compactación");

    app.apply(LiveEvent::ManualCompactionEnded {
        outcome: ManualCompaction::Compacted,
    });
    assert!(!app.compact_in_flight);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Older history summarized")
    );
    dispatch_next(&mut app, &tx);
    assert_eq!(rx.try_recv().unwrap(), "hola");
}

#[test]
fn the_outcome_of_a_manual_compaction_is_told_in_the_feedback() {
    use loomcoding_core::event::{LiveEvent, ManualCompaction};
    for (outcome, texto) in [
        (ManualCompaction::NothingToSummarize, "Nothing to summarize"),
        (ManualCompaction::Failed, "history unchanged"),
    ] {
        let mut app = App::new();
        app.compact_in_flight = true;
        app.apply(LiveEvent::ManualCompactionEnded { outcome });
        assert!(!app.compact_in_flight);
        assert!(app.slash_feedback.as_deref().unwrap().contains(texto));
    }
}

#[test]
fn slash_escape_preserves_literal_text_and_unknown_commands_do_not_dispatch() {
    let mut app = App::new();
    app.turn_in_flight = true;
    app.composer.set_text("/unknown");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.composer.text(), "/unknown");
    assert!(app
        .slash_feedback
        .as_deref()
        .unwrap()
        .contains("Unknown command"));
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.cancel_requested);
    assert!(!app.should_quit);
    assert!(app.slash_query().is_none());
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        Some("/unknown".into())
    );
    handle_key(&mut app, KeyCode::Char('/'), KeyModifiers::NONE);
    // El popup lista todo salvo los alias ocultos, que siguen en `COMMANDS`.
    let visibles = crate::commands::COMMANDS
        .iter()
        .filter(|(command, _, _)| !crate::commands_alias::is_hidden_alias(*command))
        .count();
    assert_eq!(app.slash_matches().len(), visibles);

    for text in [
        "explain /help",
        "/tmp/file.rs",
        "//literal",
        "/help me",
        "/help\nsecond line",
    ] {
        let mut app = App::new();
        app.composer.set_text(text);
        assert!(app.slash_query().is_none());
        assert_eq!(
            handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
            Some(text.into())
        );
    }
    app.composer.set_text("/tools");
    let _reply = open_dialog(&mut app);
    assert!(app.slash_query().is_none());
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!app.tools_open);
    assert_eq!(app.composer.text(), "/tools");
}
