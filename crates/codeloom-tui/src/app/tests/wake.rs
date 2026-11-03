use super::*;

/// Evento de cierre de un hijo (sesión 9) de la sesión 1 con el estado dado.
fn settled(status: TaskState) -> LiveEvent {
    LiveEvent::SubagentSettled {
        parent: SessionId(1),
        child: SessionId(9),
        status,
    }
}

#[test]
fn wake_requested_on_child_question() {
    let mut app = idle_parent_app();
    app.apply(LiveEvent::SubagentInputRequested {
        parent: SessionId(1),
        child: SessionId(9),
        label: "review".into(),
    });
    assert!(app.wake_pending);
    assert!(app.take_wake());
}

#[test]
fn wake_requested_on_completed_or_failed_but_not_canceled_or_rejected() {
    for status in [TaskState::Completed, TaskState::Failed] {
        let mut app = idle_parent_app();
        app.apply(settled(status));
        assert!(app.wake_pending, "{status:?} debe despertar al padre");
    }
    for status in [TaskState::Canceled, TaskState::Rejected] {
        let mut app = idle_parent_app();
        app.apply(settled(status));
        assert!(!app.wake_pending, "{status:?} no debe despertar al padre");
    }
}

#[test]
fn no_wake_while_turn_in_flight_or_dialog_open_or_prompt_queued() {
    type Blocker = (&'static str, fn(&mut App));
    let blockers: [Blocker; 11] = [
        ("wake_in_flight", |app| app.wake_in_flight = true),
        ("turn_in_flight", |app| app.turn_in_flight = true),
        ("pending_prompts", |app| {
            app.pending_prompts.push_back("en cola".into())
        }),
        ("permission", |app| {
            let (reply, _rx) = tokio::sync::oneshot::channel();
            app.enqueue_permission(
                PermissionRequest {
                    tool: "bash".into(),
                    action: "run".into(),
                    detail: "{}".into(),
                },
                reply,
            );
        }),
        ("question", |app| {
            let (reply, _rx) = tokio::sync::oneshot::channel();
            app.enqueue_questions(vec![ask_question("¿Cuál?", &["a", "b"], false)], reply);
        }),
        ("queue_open", |app| app.queue_open = true),
        ("queue_paused", |app| app.queue_paused = true),
        ("session_loading", |app| app.session_loading = true),
        ("session_picker_open", |app| app.session_picker_open = true),
        ("settings_open", |app| app.settings_open = true),
        ("settings_saving", |app| app.settings_saving = true),
    ];
    for (name, block) in blockers {
        let mut app = idle_parent_app();
        app.wake_pending = true;
        block(&mut app);
        assert!(!app.take_wake(), "{name} debe impedir el despertar");
        assert!(app.wake_pending, "{name}: el despertar queda pendiente");
    }
    let mut app = idle_parent_app();
    app.wake_pending = true;
    assert!(app.take_wake(), "sin bloqueos el despertar se pide");
}

#[test]
fn wake_spends_budget_only_when_a_turn_starts() {
    let mut app = idle_parent_app();
    app.wake_pending = true;
    assert!(app.take_wake());
    app.apply(LiveEvent::TurnStarted);
    assert_eq!(app.auto_turns_left, MAX_AUTO_TURNS - 1);
    assert_eq!(app.status, "auto-turn");
    assert!(!app.wake_in_flight);
    app.apply(LiveEvent::TurnEnded);

    // Un turno iniciado por el usuario no gasta presupuesto ni se marca como automático.
    app.apply(LiveEvent::TurnStarted);
    assert_eq!(app.auto_turns_left, MAX_AUTO_TURNS - 1);
    assert_eq!(app.status, "working");
}

#[test]
fn wake_idle_clears_in_flight_and_spends_nothing() {
    let mut app = idle_parent_app();
    app.wake_pending = true;
    assert!(app.take_wake());
    app.apply(LiveEvent::WakeIdle);
    assert!(!app.wake_in_flight);
    assert!(!app.turn_in_flight);
    assert_eq!(app.auto_turns_left, MAX_AUTO_TURNS);
}

#[test]
fn wake_error_clears_in_flight() {
    let mut app = idle_parent_app();
    app.wake_pending = true;
    assert!(app.take_wake());
    app.apply(LiveEvent::Error {
        message: "proveedor caído".into(),
    });
    assert!(!app.wake_in_flight);
    assert!(!app.turn_in_flight);
    assert_eq!(app.auto_turns_left, MAX_AUTO_TURNS);
    // Tras el error, un nuevo aviso del hijo vuelve a poder despertar al padre.
    app.apply(settled(TaskState::Completed));
    assert!(app.take_wake());
}

#[test]
fn wake_ignores_other_sessions() {
    let mut app = idle_parent_app();
    app.apply(LiveEvent::SubagentInputRequested {
        parent: SessionId(2),
        child: SessionId(9),
        label: "review".into(),
    });
    app.apply(LiveEvent::SubagentSettled {
        parent: SessionId(2),
        child: SessionId(9),
        status: TaskState::Completed,
    });
    assert!(!app.wake_pending);

    let mut sin_sesion = App::new();
    sin_sesion.apply(settled(TaskState::Completed));
    assert!(!sin_sesion.wake_pending);
}

#[test]
fn opening_another_session_drops_the_pending_wake() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Completed));
    assert!(app.wake_pending);
    app.load_session(SessionId(2), Vec::new());
    assert!(!app.wake_pending);
    assert!(app.wake_causes.is_empty());
}

#[test]
fn settled_child_records_wake_cause() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Completed));
    assert_eq!(app.wake_causes, vec![(9, "completed")]);

    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Failed));
    assert_eq!(app.wake_causes, vec![(9, "failed")]);
}

#[test]
fn input_request_records_wake_cause() {
    let mut app = idle_parent_app();
    app.apply(LiveEvent::SubagentInputRequested {
        parent: SessionId(1),
        child: SessionId(9),
        label: "review".into(),
    });
    assert_eq!(app.wake_causes, vec![(9, "asks input")]);
}

#[test]
fn killed_child_records_no_cause() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Canceled));
    assert!(app.wake_causes.is_empty());
}

#[test]
fn woken_turn_emits_cause_line_and_clears_causes() {
    let mut app = idle_parent_app();
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(settled(TaskState::Completed));
    assert!(app.take_wake());
    app.apply(LiveEvent::TurnStarted);
    assert_eq!(
        system_lines(&app),
        vec!["⟳ auto-turn · woken by «review» (completed)".to_string()]
    );
    assert!(app.wake_causes.is_empty());
}

#[test]
fn user_turn_emits_no_cause_line() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Completed));
    // Turno del usuario: no hay `wake_in_flight`, así que no hay línea ni se toca la causa.
    app.apply(LiveEvent::TurnStarted);
    assert!(system_lines(&app).is_empty());
    assert_eq!(app.wake_causes, vec![(9, "completed")]);
}

#[test]
fn cause_line_with_several_causes_shows_latest_and_count() {
    let mut app = idle_parent_app();
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.wake_causes = vec![(9, "completed"), (9, "failed"), (9, "asks input")];
    assert_eq!(
        app.wake_cause_line().as_deref(),
        Some("⟳ auto-turn · woken by «review» (asks input) (+2 more)")
    );
}

#[test]
fn cause_line_uses_agent_id_for_unknown_child() {
    let mut app = idle_parent_app();
    app.wake_causes = vec![(42, "completed")];
    assert_eq!(
        app.wake_cause_line().as_deref(),
        Some("⟳ auto-turn · woken by «agent #42» (completed)")
    );
    assert_eq!(App::new().wake_cause_line(), None);
}

#[test]
fn exhausted_budget_drops_wake_causes() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Completed));
    app.auto_turns_left = 0;
    assert!(!app.take_wake());
    assert!(app.wake_causes.is_empty());
}

#[test]
fn escape_clears_wake_causes() {
    let mut app = idle_parent_app();
    app.apply(settled(TaskState::Completed));
    assert!(!app.wake_causes.is_empty());
    app.request_cancel();
    assert!(app.wake_causes.is_empty());
}
