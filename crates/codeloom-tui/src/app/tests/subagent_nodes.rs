use super::*;

#[test]
fn active_subagent_node_expands_by_default() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "voy a mirar".into(),
        }])),
    ));
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working)".to_string(),
            "└─ › voy a mirar".to_string(),
        ]
    );
    assert!(app.expanded_nodes.contains(&9));
}

#[test]
fn failed_subagent_node_shows_its_bounded_error_message() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mut child = sub("review", TaskState::Failed, SessionId(1));
    child.status.message = Some(codeloom_core::a2a::Message::agent_text(
        "provider unavailable",
    ));
    app.apply(LiveEvent::SubagentChanged(child));
    app.expanded_nodes.insert(9);
    app.refresh_child_node(9);
    assert!(texts(&app)
        .iter()
        .any(|line| line == "└─ ✗ error: provider unavailable"));
}

#[test]
fn each_a2a_state_has_its_own_glyph_and_exact_name() {
    let mut glifos = std::collections::HashSet::new();
    for state in TaskState::ALL {
        let mut app = App::new();
        app.session_id = Some(SessionId(1));
        app.apply(LiveEvent::SubagentChanged(sub(
            "review",
            state,
            SessionId(1),
        )));
        let glifo = agent_state(state).glyph();
        let nombre = agent_state(state).label();
        let cabecera = texts(&app).into_iter().next().expect("cabecera");
        let expansion = if state.is_terminal() { "▸" } else { "▾" };
        assert!(
            cabecera.starts_with(&format!("{expansion} {glifo} subagent")),
            "{state:?}: {cabecera}"
        );
        assert!(
            cabecera.contains(&format!("({nombre})")),
            "{state:?}: {cabecera}"
        );
        glifos.insert(glifo);
    }
    assert_eq!(glifos.len(), TaskState::ALL.len(), "glifos repetidos");
}

#[test]
fn a_working_node_shows_up_to_three_lines_of_its_latest_progress_note() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mut hijo = sub("review", TaskState::Working, SessionId(1));
    hijo.artifacts = codeloom_core::a2a::publicar_progreso(
        &[],
        "plan",
        None,
        "1. leer\n\n2. editar\n3. probar\n4. no cabe",
    )
    .unwrap();
    app.apply(LiveEvent::SubagentChanged(hijo.clone()));
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working) · 1 note".to_string(),
            "  │ plan: 1. leer".to_string(),
            "  │ 2. editar".to_string(),
            "  │ 3. probar".to_string(),
        ]
    );
    // El avance más reciente sustituye a la vista previa anterior.
    hijo.artifacts =
        codeloom_core::a2a::publicar_progreso(&hijo.artifacts, "hallazgos", None, "dos fallos")
            .unwrap();
    app.apply(LiveEvent::SubagentChanged(hijo));
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working) · 2 notes".to_string(),
            "  │ hallazgos: dos fallos".to_string(),
        ]
    );
}

#[test]
fn a_node_without_progress_notes_or_already_settled_has_no_preview() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    assert_eq!(
        texts(&app),
        vec!["▾ ● subagent  review (working)".to_string()]
    );
    let mut terminado = sub("review", TaskState::Completed, SessionId(1));
    terminado.artifacts =
        codeloom_core::a2a::publicar_progreso(&[], "plan", None, "1. leer").unwrap();
    app.apply(LiveEvent::SubagentChanged(terminado));
    let lineas = texts(&app);
    assert_eq!(lineas.len(), 1, "{lineas:?}");
    assert!(lineas[0].contains("(completed)"), "{lineas:?}");
}

#[test]
fn preview_lines_are_truncated_by_width() {
    let mut hijo = sub("review", TaskState::Working, SessionId(1));
    hijo.artifacts =
        codeloom_core::a2a::publicar_progreso(&[], "n", None, &"x".repeat(300)).unwrap();
    let previa = progress_preview(&hijo);
    assert_eq!(previa.len(), 1);
    assert_eq!(previa[0].chars().count(), PREVIEW_COLUMNS);
    assert!(previa[0].ends_with('…'));
}

#[test]
fn node_summary_shows_step_and_current_tool() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::StepStarted {
            step: 2,
            max_steps: 30,
        },
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolStarted {
            call_id: "c1".into(),
            name: "read_file".into(),
        },
    ));
    assert_eq!(
        texts(&app),
        vec!["▾ ● subagent  review (working) · step 2/30 · read_file".to_string()]
    );
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::ToolFinished {
            call_id: "c1".into(),
            is_error: false,
        },
    ));
    assert_eq!(
        texts(&app),
        vec!["▾ ● subagent  review (working) · step 2/30".to_string()]
    );
}

#[test]
fn classic_look_shows_every_line_of_a_subagent_node() {
    let mut app = App::new(); // look compacto: solo la cabecera en las demás herramientas
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "voy a mirar".into(),
        }])),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            input: json!({ "command": "cargo test" }),
        }])),
    ));
    // Desplegado a mano: el nodo enseña sus dos hijos con guías de árbol.
    app.expanded_nodes.insert(9);
    app.refresh_child_node(9);
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working)".to_string(),
            "├─ › voy a mirar".to_string(),
            "└─ … bash cargo test".to_string(),
        ]
    );
}

#[test]
fn a_grandchild_tool_call_hangs_from_its_own_indented_node() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        9,
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentChanged(child_descriptor(
        10,
        TaskState::Working,
        SessionId(9),
    )));
    app.apply(child_event(
        SessionId(9),
        10,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            input: json!({ "command": "cargo test" }),
        }])),
    ));
    let lineas = texts(&app);
    assert!(
        lineas.contains(&"  └─ … bash cargo test".to_string()),
        "{lineas:?}"
    );
    // El nodo del hijo no recibe la llamada del nieto.
    assert!(!lineas.iter().any(|t| t.starts_with("└─")), "{lineas:?}");
}

#[test]
fn expanded_node_ends_with_the_result_line_once_finished() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mut hecho = sub("review", TaskState::Completed, SessionId(1));
    hecho.artifacts = vec![codeloom_core::a2a::Artifact::text_artifact(
        "\nprimera línea\nsegunda",
    )];
    app.apply(LiveEvent::SubagentChanged(hecho));
    app.expanded_nodes.insert(9);
    app.refresh_child_node(9);
    // Sin registro de hijo, solo queda la línea del resultado; se toma la primera no vacía.
    assert_eq!(
        texts(&app),
        vec![
            "▾ ✓ subagent  review (completed)".to_string(),
            "└─ ✓ result: primera línea".to_string(),
        ]
    );
}

#[test]
fn right_expands_the_active_subagent_node_and_left_folds_it() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "voy a mirar".into(),
        }])),
    ));
    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            input: json!({ "command": "cargo test" }),
        }])),
    ));
    app.focus = Focus::Transcript;
    app.block = Some(0);
    crate::run::handle_key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(texts(&app).len(), 1, "plegado, el nodo ocupa una línea");

    crate::run::handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working)".to_string(),
            "├─ › voy a mirar".to_string(),
            "└─ … bash cargo test".to_string(),
        ]
    );

    crate::run::handle_key(&mut app, KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(
        texts(&app),
        vec!["▸ ● subagent  review (working)".to_string()]
    );

    crate::run::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(texts(&app).len(), 3, "Enter vuelve a desplegar el nodo");
    crate::run::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        texts(&app),
        vec!["▸ ● subagent  review (working)".to_string()],
        "Enter vuelve a plegar el mismo nodo"
    );
}

#[test]
fn expanding_without_an_active_node_does_nothing() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    // Transcript vacío: sin bloque efectivo no hay nodo ni pánico.
    app.focus = Focus::Transcript;
    app.block = Some(3);
    assert!(!app.set_node_expanded(true));
    crate::run::handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    crate::run::handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.expanded_nodes.is_empty());

    // Un mensaje normal bajo el bloque activo no es un nodo.
    app.push_user("hola");
    app.block = Some(0);
    assert!(!app.set_node_expanded(true));
    assert!(app.expanded_nodes.is_empty());

    // Con un nodo en el transcript, el bloque sobre el mensaje sigue sin tocarlo.
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.block = Some(0);
    assert!(!app.set_node_expanded(true));
    assert!(app.expanded_nodes.contains(&9));
    assert!(texts(&app).iter().any(|t| t.starts_with('▾')));
}

#[test]
fn expanded_node_follows_new_child_events() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.focus = Focus::Transcript;
    app.block = Some(0);
    assert!(app.set_node_expanded(true));
    // Sin mensajes del hijo, desplegado solo queda la cabecera.
    assert_eq!(texts(&app).len(), 1);

    app.apply(child_event(
        SessionId(1),
        9,
        LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::Text {
            text: "hola".into(),
        }])),
    ));
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working)".to_string(),
            "└─ › hola".to_string(),
        ]
    );

    assert!(app.set_node_expanded(false));
    assert_eq!(texts(&app).len(), 1);
}

/// Mensaje del asistente con una única llamada `name` de entrada `input`, para que la
/// herramienta fije sus hechos como lo hace una sesión restaurada o en vivo.
fn tool_call_message(id: &str, name: &str, input: serde_json::Value) -> LiveEvent {
    LiveEvent::MessageAdded(Message::assistant(vec![ContentPart::ToolCall {
        id: id.into(),
        name: name.into(),
        input,
    }]))
}

#[test]
fn job_output_on_a_child_job_hangs_from_its_node_and_leaves_no_line() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(tool_call_message(
        "j1",
        "job_output",
        json!({"id": "subagent-1"}),
    ));
    assert!(
        !texts(&app).iter().any(|t| t.starts_with("job_output")),
        "la llamada al job del hijo no debe dejar línea suelta: {:?}",
        texts(&app)
    );
    app.focus = Focus::Transcript;
    app.block = Some(0);
    assert!(app.set_node_expanded(true));
    assert_eq!(
        texts(&app).last().map(String::as_str),
        Some("└─ ↳ parent: job_output"),
        "la llamada debe colgar del nodo: {:?}",
        texts(&app)
    );
}

#[test]
fn job_output_on_an_unknown_id_stays_a_normal_line() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(tool_call_message(
        "j2",
        "job_output",
        json!({"id": "job-ajeno"}),
    ));
    assert!(
        texts(&app).iter().any(|t| t.starts_with("job_output")),
        "un job ajeno al árbol sigue siendo una línea del padre: {:?}",
        texts(&app)
    );
}

#[test]
fn folded_node_counts_the_parent_queries() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(tool_call_message(
        "j1",
        "job_output",
        json!({"id": "subagent-1"}),
    ));
    app.apply(tool_call_message(
        "j2",
        "job_kill",
        json!({"id": "subagent-1"}),
    ));
    app.apply(tool_call_message(
        "s1",
        "subagent_control",
        json!({"action": "interrupt", "child_id": 9}),
    ));
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working) · 3 queries".to_string(),
            "├─ ↳ parent: job_output".to_string(),
            "├─ ↳ parent: job_kill".to_string(),
            "└─ ↳ parent: subagent_control".to_string()
        ]
    );
    // Volver a sincronizar las mismas llamadas no las cuenta de nuevo.
    app.sync_all_tools();
    assert_eq!(
        texts(&app),
        vec![
            "▾ ● subagent  review (working) · 3 queries".to_string(),
            "├─ ↳ parent: job_output".to_string(),
            "├─ ↳ parent: job_kill".to_string(),
            "└─ ↳ parent: subagent_control".to_string()
        ]
    );
}
