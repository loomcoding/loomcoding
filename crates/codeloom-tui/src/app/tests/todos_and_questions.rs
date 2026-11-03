use super::*;

/// Tarea de prueba con el contenido y el estado indicados.
fn todo(content: &str, status: codeloom_core::todo::TodoStatus) -> codeloom_core::todo::TodoItem {
    codeloom_core::todo::TodoItem {
        content: content.into(),
        status,
    }
}

#[test]
fn todos_event_creates_and_updates_one_element() {
    use codeloom_core::todo::TodoStatus::{Completed, Pending};
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("uno", Pending), todo("dos", Pending)],
    });
    assert_eq!(app.transcript.entry_count(), 1);
    assert!(texts(&app).iter().any(|t| t == "todos  0/2"));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("uno", Completed), todo("dos", Pending)],
    });
    assert_eq!(app.transcript.entry_count(), 1);
    let lines = texts(&app);
    assert!(lines.iter().any(|t| t == "todos  1/2"));
    assert!(!lines.iter().any(|t| t == "todos  0/2"));
}

#[test]
fn foreign_session_todos_are_ignored() {
    use codeloom_core::todo::TodoStatus::Pending;
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(2),
        items: vec![todo("ajena", Pending)],
    });
    assert!(app.todos.is_empty());
    assert_eq!(app.transcript.entry_count(), 0);
}

#[test]
fn classic_look_keeps_todos_out_of_the_transcript_but_stores_them() {
    use codeloom_core::todo::TodoStatus::InProgress;
    let mut app = App::new(); // tema Batten = Classic
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("uno", InProgress)],
    });
    assert_eq!(app.todos.len(), 1);
    assert!(!texts(&app).iter().any(|t| t.starts_with("todos")));
}

#[test]
fn switching_look_hides_and_restores_todos() {
    use codeloom_core::todo::TodoStatus::Pending;
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("uno", Pending)],
    });
    assert!(texts(&app).iter().any(|t| t == "todos  0/1"));
    app.theme = batten::widgets::Theme::preset(batten::widgets::ThemePreset::Batten);
    app.sync_all_tools();
    assert!(!texts(&app).iter().any(|t| t.starts_with("todos")));
    app.theme = batten::widgets::Theme::preset(batten::widgets::ThemePreset::Crush);
    app.sync_all_tools();
    assert!(texts(&app).iter().any(|t| t == "todos  0/1"));
}

#[test]
fn todo_write_tool_has_no_transcript_card() {
    let mut app = crush_app();
    app.apply(LiveEvent::ToolStarted {
        call_id: "t1".into(),
        name: "todo_write".into(),
    });
    assert_eq!(app.tools.len(), 1);
    assert!(!texts(&app).iter().any(|t| t.starts_with("todo_write")));
    // Un cambio de look no debe devolver la tarjeta de la herramienta.
    app.sync_all_tools();
    assert!(!texts(&app).iter().any(|t| t.starts_with("todo_write")));
}

#[test]
fn empty_list_clears_the_element() {
    use codeloom_core::todo::TodoStatus::Pending;
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("uno", Pending), todo("dos", Pending)],
    });
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: Vec::new(),
    });
    assert!(app.todos.is_empty());
    assert!(!texts(&app).iter().any(|t| t.starts_with("todos")));
    assert_eq!(app.transcript.entry_count(), 1);
}

#[test]
fn restore_rebuilds_todos_from_history() {
    let mut app = crush_app();
    app.restore_messages(vec![Message::assistant(vec![ContentPart::ToolCall {
        id: "t1".into(),
        name: "todo_write".into(),
        input: json!({"todos": [
            {"content": "a", "status": "completed"},
            {"content": "b", "status": "pending"}
        ]}),
    }])]);
    assert_eq!(app.todos.len(), 2);
    assert!(texts(&app).iter().any(|t| t == "todos  1/2"));
}

#[test]
fn restore_keeps_only_the_last_todo_list() {
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::assistant(vec![ContentPart::ToolCall {
            id: "t1".into(),
            name: "todo_write".into(),
            input: json!({"todos": [{"content": "a", "status": "pending"}]}),
        }]),
        Message::assistant(vec![ContentPart::ToolCall {
            id: "t2".into(),
            name: "todo_write".into(),
            input: json!({"todos": [
                {"content": "a", "status": "completed"},
                {"content": "b", "status": "in_progress"}
            ]}),
        }]),
    ]);
    assert_eq!(app.todos.len(), 2);
    let lines = texts(&app);
    assert_eq!(lines.iter().filter(|t| t.starts_with("todos")).count(), 1);
    assert!(lines.iter().any(|t| t == "todos  1/2"));
}

#[test]
fn restore_ignores_invalid_todo_input_and_clears_previous_todos() {
    use codeloom_core::todo::TodoStatus::Pending;
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![todo("viejo", Pending)],
    });
    app.restore_messages(vec![Message::assistant(vec![ContentPart::ToolCall {
        id: "t1".into(),
        name: "todo_write".into(),
        input: json!({"todos": "nope"}),
    }])]);
    assert!(app.todos.is_empty());
    assert!(!texts(&app).iter().any(|t| t.starts_with("todos")));
}

#[test]
fn enqueue_questions_opens_the_first_and_queues_the_rest() {
    use tokio::sync::oneshot::error::TryRecvError;
    let mut app = App::new();
    let (tx1, mut rx1) = tokio::sync::oneshot::channel();
    let (tx2, mut rx2) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question("Uno?", &["a", "b"], false)], tx1);
    app.enqueue_questions(vec![ask_question("Dos?", &["a", "b"], false)], tx2);
    let open = app.question.as_ref().expect("abre la primera pregunta");
    assert_eq!(open.questions[0].question, "Uno?");
    assert_eq!(open.dialog.len(), 1);
    assert_eq!(app.question_queue.len(), 1);
    assert!(matches!(rx1.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(rx2.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn answering_sends_answers_and_opens_the_next() {
    use codeloom_core::question::Answer;
    let mut app = App::new();
    let (tx1, mut rx1) = tokio::sync::oneshot::channel();
    let (tx2, _rx2) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question("Uno?", &["a", "b"], false)], tx1);
    app.enqueue_questions(vec![ask_question("Dos?", &["a", "b"], false)], tx2);
    app.question_key(batten::state::QuestionKey::Enter);
    assert_eq!(
        rx1.try_recv().unwrap(),
        Some(vec![Answer {
            question: "Uno?".into(),
            selected: vec!["a".into()],
        }])
    );
    let next = app.question.as_ref().expect("abre la siguiente");
    assert_eq!(next.questions[0].question, "Dos?");
}

#[test]
fn esc_sends_none() {
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question("Uno?", &["a", "b"], false)], tx);
    app.question_key(batten::state::QuestionKey::Esc);
    assert_eq!(rx.try_recv().unwrap(), None);
    assert!(app.question.is_none());
}

#[test]
fn cancel_question_drains_everything_with_none() {
    let mut app = App::new();
    let (tx1, mut rx1) = tokio::sync::oneshot::channel();
    let (tx2, mut rx2) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question("Uno?", &["a", "b"], false)], tx1);
    app.enqueue_questions(vec![ask_question("Dos?", &["a", "b"], false)], tx2);
    app.cancel_question();
    assert_eq!(rx1.try_recv().unwrap(), None);
    assert_eq!(rx2.try_recv().unwrap(), None);
    assert!(app.question.is_none());
    assert!(app.question_queue.is_empty());
}

#[test]
fn questions_close_other_overlays() {
    let mut app = App::new();
    app.help_open = true;
    app.tools_open = true;
    app.queue_open = true;
    app.settings_open = true;
    let (tx, _rx) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question("Uno?", &["a", "b"], false)], tx);
    assert!(!app.help_open);
    assert!(!app.tools_open);
    assert!(!app.queue_open);
    assert!(!app.settings_open);
    assert!(app.question.is_some());
}

#[test]
fn empty_question_list_is_answered_with_none() {
    let mut app = App::new();
    let (tx, mut rx) = tokio::sync::oneshot::channel();
    app.enqueue_questions(Vec::new(), tx);
    assert_eq!(rx.try_recv().unwrap(), None);
    assert!(app.question.is_none());
    assert!(app.question_queue.is_empty());
}

#[test]
fn subagent_input_events_drive_the_waiting_state() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));

    // Un cambio de un hijo de otro padre no se refleja en esta sesión.
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::InputRequired,
        SessionId(2),
    )));
    assert_eq!(app.subagents[&9].status.state, TaskState::Working);

    // La espera es el estado de la tarea; la pregunta solo despierta al padre.
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::InputRequired,
        SessionId(1),
    )));
    assert_eq!(app.subagents[&9].status.state, TaskState::InputRequired);
    app.apply(LiveEvent::SubagentInputRequested {
        parent: SessionId(1),
        child: SessionId(9),
        label: "review".into(),
    });
    assert!(app.wake_pending);

    // Responder devuelve la tarea a `working`, y el estado lo dice el propio cambio.
    app.apply(LiveEvent::SubagentInputResolved {
        parent: SessionId(1),
        child: SessionId(9),
    });
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    assert_eq!(app.subagents[&9].status.state, TaskState::Working);

    // Un estado terminal cierra la espera.
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Completed,
        SessionId(1),
    )));
    assert_eq!(app.subagents[&9].status.state, TaskState::Completed);
}
