use super::*;

#[test]
fn esc_and_ctrl_q_quit() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_quit);
    assert!(!app.cancel_requested);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.should_quit);

    let mut app = App::new();
    handle_key(&mut app, KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.should_quit);
}

#[test]
fn f1_opens_help_and_escape_closes_it_without_canceling() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    assert!(app.help_open);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.help_open);
    assert!(!app.cancel_requested);
    assert!(!app.should_quit);
}

#[test]
fn ctrl_b_toggles_sidebar_without_editing_composer() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert!(app.sidebar_expanded);
    assert!(app.composer.is_empty());
    handle_key(&mut app, KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert!(!app.sidebar_expanded);
}

#[test]
fn ctrl_o_toggles_latest_tool_output() {
    let mut app = App::new();
    app.tools
        .push_back(loomcoding_cloth::state::ToolItem::new("call-1", "view"));
    handle_key(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(app.tools.back().unwrap().expanded);
    handle_key(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(!app.tools.back().unwrap().expanded);
}

#[test]
fn esc_cancellation_waits_for_worker_token() {
    let mut app = App::new();
    app.turn_in_flight = true;
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
    cancel_active_turn(&mut app, &active_turn);
    assert!(app.cancel_requested);

    let token = CancellationToken::new();
    *active_turn.lock().unwrap() = Some(token.clone());
    cancel_active_turn(&mut app, &active_turn);
    assert!(token.is_cancelled());
    assert!(!app.cancel_requested);
    assert_eq!(app.status, "cancelling");
}

#[test]
fn reverse_tab_restores_focus_and_dialogs_preserve_the_draft() {
    let mut app = App::new();
    app.composer.set_text("unfinished prompt");
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(app.focus, Focus::Transcript);
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(app.focus, Focus::Transcript);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Composer);
    assert_eq!(app.composer.text(), "unfinished prompt");
    assert!(!app.should_quit);
}

#[test]
fn repeat_edits_text_but_does_not_submit_or_answer_permissions() {
    use crossterm::event::KeyEvent;
    let mut app = App::new();
    let repeat = |code| {
        Event::Key(KeyEvent::new_with_kind(
            code,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        ))
    };
    handle_event(&mut app, repeat(KeyCode::Char('a')));
    handle_event(&mut app, repeat(KeyCode::Char('b')));
    handle_event(&mut app, repeat(KeyCode::Backspace));
    assert_eq!(app.composer.text(), "a");
    assert_eq!(handle_event(&mut app, repeat(KeyCode::Enter)), None);
    assert_eq!(app.composer.text(), "a");
    let mut reply = open_dialog(&mut app);
    handle_event(&mut app, repeat(KeyCode::Char('a')));
    handle_event(&mut app, repeat(KeyCode::Enter));
    assert!(matches!(
        reply.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(app.permission.is_some());
}

fn ask_question_for_keys() -> loomcoding_core::question::Question {
    use loomcoding_core::question::{Question, QuestionOption};
    let option = |label: &str| QuestionOption {
        label: label.into(),
        description: String::new(),
    };
    Question {
        header: "Stack".into(),
        question: "Which?".into(),
        options: vec![option("a"), option("b")],
        multi_select: false,
    }
}

#[test]
fn keys_reach_the_question_dialog_and_not_the_composer() {
    use loomcoding_core::question::Answer;
    let mut app = App::new();
    let (tx, mut rx) = oneshot::channel();
    app.enqueue_questions(vec![ask_question_for_keys()], tx);
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    handle_event(&mut app, Event::Paste("oculto".into()));
    assert!(app.composer.is_empty());
    assert!(!app.help_open);
    assert!(app.question.is_some());
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.question.is_none());
    assert_eq!(
        rx.try_recv().unwrap(),
        Some(vec![Answer {
            question: "Which?".into(),
            selected: vec!["b".into()],
        }])
    );

    // Esc responde a la pregunta (sin contar para la salida por doble Esc).
    let (tx, mut rx) = oneshot::channel();
    app.enqueue_questions(vec![ask_question_for_keys()], tx);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(rx.try_recv().unwrap(), None);
    assert!(!app.should_quit);
    assert!(app.last_escape.is_none());
    assert!(app.composer.is_empty());
}

#[test]
fn tab_moves_focus_and_transcript_keys_scroll() {
    let mut app = App::new();
    app.viewport.set_geometry(100, 10);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "");
    assert!(!app.viewport.is_following());
    handle_key(&mut app, KeyCode::Char('G'), KeyModifiers::NONE);
    assert!(app.viewport.is_following());
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    assert!(
        app.should_quit,
        "Esc sale también con foco en el transcript"
    );
}

#[test]
fn enter_sends_prompt() {
    let mut app = App::new();
    assert_eq!(
        handle_key(&mut app, KeyCode::Char('h'), KeyModifiers::NONE),
        None
    );
    assert_eq!(
        handle_key(&mut app, KeyCode::Char('i'), KeyModifiers::NONE),
        None
    );
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        Some("hi".to_string())
    );
    assert!(!app.should_quit);
}

#[test]
fn ctrl_j_adds_line_and_enter_sends() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Char('b'), KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "a\nb");
    let sent = handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent, Some("a\nb".to_string()));
}

#[test]
fn up_recalls_history_when_single_line() {
    let mut app = App::new();
    for c in "hola".chars() {
        handle_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "hola");
}

#[test]
fn escape_quits_with_transcript_selection() {
    let mut app = App::new();
    for s in ["uno", "dos", "tres"] {
        app.push_user(s);
    }
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('v'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
    assert_eq!(app.clipboard.as_deref(), Some("dos\ntres"));
    assert!(app.selection.is_none());
    handle_key(&mut app, KeyCode::Char('v'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.should_quit);
    assert_eq!(app.focus, Focus::Transcript);
}

#[test]
fn up_moves_the_cursor_when_multiline() {
    let mut app = App::new();
    app.composer.set_text("uno\ndos");
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.composer.cursor_row_col().0, 0);
}
