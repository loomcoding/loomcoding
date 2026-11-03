use super::*;

#[test]
fn budget_exhausted_stops_waking_until_user_writes() {
    use codeloom_core::session::SessionId;
    use codeloom_core::task::TaskState;
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.session_id = Some(SessionId(1));
    app.auto_turns_left = 0;
    app.apply(codeloom_core::event::LiveEvent::SubagentSettled {
        parent: SessionId(1),
        child: SessionId(9),
        status: TaskState::Completed,
    });

    assert!(!app.take_wake());
    assert_eq!(app.status, "auto-turns exhausted — write to continue");
    assert!(!app.wake_pending);
    assert!(!app.wake_in_flight);
    assert_eq!(app.auto_turns_left, 0);

    // Solo un mensaje del usuario rearma el presupuesto.
    send(&mut app, &tx, Some("hola".into()));
    assert_eq!(app.auto_turns_left, crate::app::MAX_AUTO_TURNS);
    assert_eq!(rx.try_recv().unwrap(), "hola");
}

#[test]
fn queued_prompt_waits_while_a_wake_is_in_flight() {
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.wake_in_flight = true;
    send(&mut app, &tx, Some("después".into()));
    assert!(
        rx.try_recv().is_err(),
        "no se solapa un turno con el despertar en vuelo"
    );
    assert_eq!(app.pending_prompts.len(), 1);

    app.apply(codeloom_core::event::LiveEvent::WakeIdle);
    dispatch_next(&mut app, &tx);
    assert_eq!(rx.try_recv().unwrap(), "después");
}

#[test]
fn dispatch_wake_releases_the_flag_when_the_worker_is_gone() {
    let mut app = App::new();
    app.wake_pending = true;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    drop(rx);
    dispatch_wake(&mut app, &tx);
    assert!(!app.wake_in_flight);
    assert_eq!(app.status, "error: wake channel closed");
}

#[test]
fn cancel_clears_pending_wake() {
    use codeloom_core::session::SessionId;
    use codeloom_core::task::TaskState;
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.turn_in_flight = true;
    app.apply(codeloom_core::event::LiveEvent::SubagentSettled {
        parent: SessionId(1),
        child: SessionId(9),
        status: TaskState::Completed,
    });
    assert!(app.wake_pending);

    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.wake_pending);
    app.apply(codeloom_core::event::LiveEvent::TurnEnded);
    assert!(
        !app.take_wake(),
        "cancelar no rearma el despertar del mismo evento"
    );
}
