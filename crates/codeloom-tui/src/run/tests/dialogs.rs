use super::*;

#[test]
fn dialog_captures_keys_and_answers() {
    use codeloom_core::permission::Decision;
    let mut app = App::new();
    let mut rx = open_dialog(&mut app);
    assert!(app.fx_pending);
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "");
    assert!(app.permission.is_none());
    assert_eq!(rx.try_recv().unwrap(), Decision::Allow);
}

#[test]
fn double_esc_and_ctrl_c_quit_even_with_permission_open() {
    let mut app = App::new();
    let _rx = open_dialog(&mut app);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_quit);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.should_quit);

    let mut app = App::new();
    let _rx = open_dialog(&mut app);
    handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(app.should_quit);
}

#[test]
fn session_picker_selects_existing_or_new_session() {
    let mut app = App::new();
    app.set_sessions(vec![
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(1),
            preview: "primera".into(),
            message_count: 2,
        },
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(2),
            preview: "segunda".into(),
            message_count: 4,
        },
    ]);
    handle_key(&mut app, KeyCode::End, KeyModifiers::NONE);
    assert_eq!(app.session_selected, 2);
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.session_selected, 1);
    handle_key(&mut app, KeyCode::Home, KeyModifiers::NONE);
    assert_eq!(app.session_selected, 0);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.session_choice,
        Some(SessionChoice::Resume(codeloom_core::session::SessionId(2)))
    );
    assert!(app.session_loading);

    let mut app = App::new();
    app.set_sessions(vec![codeloom_core::session::SessionInfo {
        id: codeloom_core::session::SessionId(1),
        preview: "primera".into(),
        message_count: 2,
    }]);
    handle_key(&mut app, KeyCode::Char('n'), KeyModifiers::NONE);
    assert_eq!(app.session_choice, Some(SessionChoice::New));

    let mut app = App::new();
    app.set_sessions(vec![
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(1),
            preview: "primera".into(),
            message_count: 2,
        },
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(2),
            preview: "segunda".into(),
            message_count: 4,
        },
    ]);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.session_selected, 0);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.session_choice,
        Some(SessionChoice::Resume(codeloom_core::session::SessionId(1)))
    );
    assert!(app.session_loading);
}

#[test]
fn session_picker_filters_case_insensitively_and_resumes_match() {
    let mut app = App::new();
    app.set_sessions(vec![
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(1),
            preview: "Build frontend".into(),
            message_count: 2,
        },
        codeloom_core::session::SessionInfo {
            id: codeloom_core::session::SessionId(2),
            preview: "Fix provider config".into(),
            message_count: 4,
        },
    ]);
    handle_key(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    for c in "PROVIDER".chars() {
        handle_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(app.matching_sessions().count(), 1);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.session_choice,
        Some(SessionChoice::Resume(codeloom_core::session::SessionId(2)))
    );
}

#[test]
fn f3_requests_session_picker_only_when_idle() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::F(3), KeyModifiers::NONE);
    assert!(app.session_loading);
    assert!(app.session_list_refresh_requested);
    assert!(app.session_picker_return_to_chat);

    let mut app = App::new();
    app.turn_in_flight = true;
    handle_key(&mut app, KeyCode::F(3), KeyModifiers::NONE);
    assert!(!app.session_loading);
    assert!(!app.session_list_refresh_requested);
}

#[test]
fn escape_clears_session_search_before_quitting_picker() {
    let mut app = App::new();
    app.set_sessions(vec![codeloom_core::session::SessionInfo {
        id: codeloom_core::session::SessionId(1),
        preview: "Build frontend".into(),
        message_count: 2,
    }]);
    handle_key(&mut app, KeyCode::Char('f'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.session_picker_open);
    assert!(!app.session_searching);
    assert!(app.session_query.is_empty());
    assert!(!app.should_quit);
}

#[test]
fn queued_requests_are_answered_in_order() {
    use codeloom_core::permission::Decision;
    let mut app = App::new();
    let mut first = open_dialog(&mut app);
    let mut second = open_dialog(&mut app);
    handle_key(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
    assert!(app.permission.is_some());
    handle_key(&mut app, KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(first.try_recv().unwrap(), Decision::Deny);
    assert_eq!(second.try_recv().unwrap(), Decision::AllowSession);
}
