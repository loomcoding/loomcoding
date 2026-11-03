use super::*;

#[test]
fn esc_returns_focus_from_side_panes_to_the_composer() {
    let mut app = App::new();
    app.focus = Focus::Sidebar;
    assert_eq!(handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE), None);
    assert_eq!(app.focus, Focus::Composer);
    // Esc que solo devuelve el foco no cuenta para la doble Esc de salida.
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_quit);
}

#[test]
fn printable_key_in_a_side_pane_refocuses_and_types() {
    let mut app = App::new();
    app.focus = Focus::Agent;
    handle_key(&mut app, KeyCode::Char('h'), KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Composer);
    assert_eq!(app.composer.text(), "h");
}

#[test]
fn printable_key_with_a_dialog_open_does_not_refocus() {
    let mut app = App::new();
    app.focus = Focus::Activity;
    let _rx = open_dialog(&mut app);
    handle_key(&mut app, KeyCode::Char('z'), KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Activity);
    assert!(app.composer.is_empty());
}

#[test]
fn tab_still_cycles_composer_and_transcript_only() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Composer);
    // Desde un panel lateral, Tab vuelve al composer en vez de entrar en él.
    app.focus = Focus::Rail;
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Composer);
}

/// Sesión 1 con los hijos 3 y 5 en el rail, con el foco en el rail.
fn rail_app() -> App {
    use codeloom_core::session::{SessionId, SubagentDescriptor};
    use codeloom_core::task::{TaskState, TaskStatus};
    let parent = SessionId(1);
    let mut app = App::new();
    app.session_id = Some(parent);
    app.subagents = [3, 5]
        .into_iter()
        .map(|id| {
            (
                id,
                SubagentDescriptor {
                    phase: SubagentPhase::Queued,
                    last_activity_at_ms: 0,
                    steps: 0,
                    tool_calls: 0,
                    tokens_in: 0,
                    tokens_out: 0,
                    parent,
                    child: SessionId(id),
                    job_id: format!("subagent-{id}"),
                    label: "review".into(),
                    status: TaskStatus::new(TaskState::Working),
                    artifacts: Vec::new(),
                    notice_delivered: false,
                    agent: None,
                    agent_origin: None,
                    reference_task_ids: Vec::new(),
                },
            )
        })
        .collect();
    app.focus = Focus::Rail;
    app
}

#[test]
fn rail_keys_select_and_open_child() {
    use codeloom_core::session::SessionId;
    let mut app = rail_app();
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.rail_selected, Some(3));
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.rail_selected, Some(5));
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.rail_selected, Some(3));
    assert!(app.composer.is_empty());

    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.session_choice,
        Some(SessionChoice::Resume(SessionId(3)))
    );
    assert_eq!(app.parent_stack, vec![SessionId(1)]);
    assert!(app.composer.is_empty());
}

#[test]
fn rail_x_requests_cancellation_of_only_the_selected_child() {
    let mut app = rail_app();
    app.rail_selected = Some(3);
    app.focus = Focus::Rail;
    handle_key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    assert_eq!(
        app.cancel_subagent_requested,
        Some(codeloom_core::session::SessionId(3))
    );
    assert!(app.status.contains("stopping subagent 3"));
    assert_eq!(app.focus, Focus::Rail);
    assert!(!app.cancel_requested);
}

#[test]
fn rail_enter_without_subagents_submits_composer() {
    let mut app = App::new();
    app.focus = Focus::Rail;
    app.push_char('h');
    app.push_char('i');
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        Some("hi".to_string())
    );
}

#[test]
fn alt_left_goes_to_parent_with_any_focus() {
    use codeloom_core::session::SessionId;
    for focus in [Focus::Composer, Focus::Transcript, Focus::Rail] {
        let mut app = rail_app();
        assert!(app.open_child(3));
        app.session_choice = None;
        app.focus = focus;
        handle_key(&mut app, KeyCode::Left, KeyModifiers::ALT);
        assert_eq!(
            app.session_choice,
            Some(SessionChoice::Resume(SessionId(1))),
            "con foco {focus:?}"
        );
    }
}

#[test]
fn parent_command_without_stack_sets_status() {
    let mut app = App::new();
    execute_slash_command(&mut app, crate::commands::Command::Parent);
    assert_eq!(app.status, "no parent session");
}

#[test]
fn parent_command_during_a_turn_asks_to_wait() {
    let mut app = App::new();
    app.parent_stack.push(codeloom_core::session::SessionId(1));
    app.turn_in_flight = true;
    execute_slash_command(&mut app, crate::commands::Command::Parent);
    assert_eq!(app.status, "wait for the turn to finish");
}
