use super::*;

#[test]
fn tool_inspector_navigates_copies_and_preserves_the_draft() {
    let mut app = App::new();
    app.composer.set_text("unfinished draft");
    let mut first = loomcoding_cloth::state::ToolItem::new("first", "view");
    first.set_output("complete output\nsecond line");
    app.tools.push_back(first);
    app.tools
        .push_back(loomcoding_cloth::state::ToolItem::new("second", "bash"));
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::CONTROL);
    assert!(app.tools_open);
    assert_eq!(app.tool_selected, 1);
    handle_key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
    assert!(app.clipboard.is_none());
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
    assert_eq!(
        app.clipboard.as_deref(),
        Some("complete output\nsecond line")
    );
    handle_key(&mut app, KeyCode::PageDown, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(app.tool_scroll, (10, 5));
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.tool_scroll, (0, 0));
    handle_event(&mut app, Event::Paste("hidden input".into()));
    assert_eq!(app.composer.text(), "unfinished draft");
    let _reply = open_dialog(&mut app);
    assert!(!app.tools_open);
    handle_key(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('t'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.tools_open);
    assert!(!app.cancel_requested);
    assert!(!app.should_quit);

    let mut empty = App::new();
    handle_key(&mut empty, KeyCode::Char('t'), KeyModifiers::CONTROL);
    handle_key(&mut empty, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut empty, KeyCode::Char('y'), KeyModifiers::NONE);
    assert_eq!(empty.tool_selected, 0);
    assert!(empty.clipboard.is_none());
}

#[test]
fn tool_inspector_tracks_selection_when_old_calls_are_evicted() {
    let mut app = App::new();
    for index in 0..8 {
        app.apply(loomcoding_core::event::LiveEvent::ToolStarted {
            call_id: index.to_string(),
            name: "view".into(),
        });
    }
    app.tools_open = true;
    app.tool_selected = 2;
    app.tool_scroll = (10, 5);
    for index in 8..10 {
        app.apply(loomcoding_core::event::LiveEvent::ToolStarted {
            call_id: index.to_string(),
            name: "view".into(),
        });
        assert_eq!(app.tools[app.tool_selected].call_id, "2");
    }
    assert_eq!(app.tool_scroll, (10, 5));
    app.apply(loomcoding_core::event::LiveEvent::ToolStarted {
        call_id: "10".into(),
        name: "view".into(),
    });
    assert_eq!(app.tools[app.tool_selected].call_id, "3");
    assert_eq!(app.tool_scroll, (0, 0));
    app.restore_messages(Vec::new());
    assert!(!app.tools_open);
    assert!(app.tools.is_empty());
}

#[test]
fn queue_editing_preserves_original_and_composer_until_accepting() {
    let mut app = App::new();
    app.pending_prompts.push_back("original".into());
    app.queued = 1;
    app.composer.set_text("separate draft");
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(
        app.queue_editing,
        "empty prompts cannot replace the original"
    );
    handle_event(&mut app, Event::Paste("changed\nsecond line".into()));
    assert_eq!(app.pending_prompts[0].text, "original");
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.queue_open);
    assert!(!app.queue_editing);
    assert_eq!(app.pending_prompts[0].text, "original");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::CONTROL);
    handle_event(&mut app, Event::Paste("changed".into()));
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.pending_prompts[0].text, "changed\nx");
    assert_eq!(app.composer.text(), "separate draft");
    assert_eq!(app.queued, 1);
    assert!(!app.queue_editing);
}

#[test]
fn queue_pause_cancel_and_resume_dispatch_one_prompt_at_a_time() {
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let token = CancellationToken::new();
    let active = Arc::new(Mutex::new(Some(token.clone())));
    app.turn_in_flight = true;
    send(&mut app, &tx, Some("first".into()));
    send(&mut app, &tx, Some("second".into()));
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::CONTROL);
    cancel_active_turn(&mut app, &active);
    assert!(token.is_cancelled());
    assert!(app.queue_paused);
    app.apply(loomcoding_core::event::LiveEvent::TurnEnded);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    dispatch_next(&mut app, &tx);
    assert!(rx.try_recv().is_err());
    assert_eq!(app.queued, 2);

    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
    dispatch_next(&mut app, &tx);
    assert!(
        rx.try_recv().is_err(),
        "inspection holds dispatch until close"
    );
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    dispatch_next(&mut app, &tx);
    assert_eq!(rx.try_recv().unwrap(), "first");
    dispatch_next(&mut app, &tx);
    assert!(rx.try_recv().is_err());
    app.apply(loomcoding_core::event::LiveEvent::Error {
        message: "provider failed".into(),
    });
    dispatch_next(&mut app, &tx);
    assert_eq!(rx.try_recv().unwrap(), "second");
    assert_eq!(app.queued, 0);
    assert_eq!(app.queue_selected, 0);

    app.apply(loomcoding_core::event::LiveEvent::TurnEnded);
    drop(rx);
    send(&mut app, &tx, Some("retained on failure".into()));
    assert_eq!(app.pending_prompts[0].text, "retained on failure");
    assert!(!app.turn_in_flight);
}
