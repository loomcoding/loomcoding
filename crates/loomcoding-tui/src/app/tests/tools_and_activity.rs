use super::*;

/// Mensaje de rol `Tool` con el resultado de una llamada.
fn tool_result_message(call_id: &str, content: &str, is_error: bool) -> Message {
    Message::new(
        Role::Tool,
        vec![ContentPart::ToolResult {
            call_id: call_id.into(),
            content: content.into(),
            is_error,
        }],
    )
}

#[test]
fn inline_tools_appear_in_order_between_messages() {
    let mut app = crush_app();
    app.push_user("hola");
    app.push_assistant("voy a mirar");
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.push_assistant("hecho");
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    let tool = texts.iter().position(|t| t.starts_with("View")).unwrap();
    let before = texts.iter().position(|t| t == "voy a mirar").unwrap();
    let after = texts.iter().position(|t| t == "hecho").unwrap();
    assert!(before < tool && tool < after);
}

#[test]
fn text_after_a_tool_goes_below_it() {
    let mut app = crush_app();
    app.push_assistant("uno");
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.push_assistant("dos");
    assert_eq!(app.transcript.entry_count(), 3);
}

#[test]
fn tool_entry_updates_in_place_with_result_and_output() {
    let mut app = crush_app();
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "Bash".into(),
    });
    let count = app.transcript.entry_count();
    app.apply_message_added(tool_result_message(
        "c1",
        "linea 1\nlinea 2\nlinea 3",
        false,
    ));
    assert_eq!(app.transcript.entry_count(), count);
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    assert!(texts.iter().any(|t| t == "linea 1"));
    assert!(texts.iter().any(|t| t.contains("lines hidden")));
}

#[test]
fn switching_look_keeps_the_tool_header_and_restores_the_body() {
    let mut app = crush_app();
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.apply_message_added(tool_result_message(
        "c1",
        "linea 1
linea 2",
        false,
    ));
    let con_cuerpo = app
        .transcript
        .iter()
        .filter(|l| l.plain() == "linea 1")
        .count();
    assert_eq!(con_cuerpo, 1, "el look Crush muestra el cuerpo");
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Cloth);
    app.sync_all_tools();
    assert_eq!(
        app.transcript
            .iter()
            .filter(|l| l.plain().starts_with("View"))
            .count(),
        1,
        "la cabecera nunca desaparece"
    );
    assert!(!app.transcript.iter().any(|l| l.plain() == "linea 1"));
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Crush);
    app.sync_all_tools();
    assert!(app.transcript.iter().any(|l| l.plain() == "linea 1"));
}

#[test]
fn classic_look_leaves_one_header_line_per_tool_call_and_folds_the_output() {
    let mut app = App::new(); // tema Cloth = Classic
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    assert_eq!(
        app.transcript
            .iter()
            .filter(|l| l.plain().starts_with("View"))
            .count(),
        1,
        "toda llamada deja su cabecera en el transcript"
    );
    assert_eq!(app.tools.len(), 1);
    app.apply_message_added(tool_result_message(
        "c1",
        "linea 1
linea 2",
        false,
    ));
    assert!(
        !app.transcript.iter().any(|l| l.plain() == "linea 1"),
        "el cuerpo sigue en la tarjeta fija y en el inspector, no en el transcript"
    );
}

#[test]
fn a_tool_run_under_classic_appears_in_place_after_switching_to_crush() {
    let mut app = App::new(); // tema Cloth = Classic
    app.push_user("hola");
    app.push_assistant("miro");
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.push_user("siguiente");
    app.theme_preset = loomcoding_cloth::widgets::ThemePreset::Crush;
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Crush);
    app.sync_all_tools();
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    let tool = texts.iter().position(|t| t.starts_with("View")).unwrap();
    let next = texts.iter().position(|t| t == "siguiente").unwrap();
    assert!(
        tool < next,
        "la tool debe quedar antes del mensaje siguiente"
    );
}

#[test]
fn classic_look_orders_the_assistant_text_around_a_tool_chronologically() {
    let mut app = App::new();
    app.push_assistant("uno");
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.push_assistant(" dos");
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    let uno = texts.iter().position(|t| t == "uno").unwrap();
    let herramienta = texts.iter().position(|t| t.starts_with("View")).unwrap();
    let dos = texts.iter().position(|t| t.contains("dos")).unwrap();
    assert!(
        uno < herramienta && herramienta < dos,
        "el texto posterior a la llamada va debajo de ella: {texts:?}"
    );
}

#[test]
fn restoring_a_session_places_tools_in_conversation_order() {
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::human("hola"),
        Message::assistant(vec![
            ContentPart::Text {
                text: "respuesta".into(),
            },
            ContentPart::ToolCall {
                id: "c1".into(),
                name: "View".into(),
                input: serde_json::json!({"path":"src/main.rs"}),
            },
        ]),
        tool_result_message("c1", "contenido", false),
        Message::assistant(vec![ContentPart::Text {
            text: "despues".into(),
        }]),
    ]);
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    let answer = texts.iter().position(|t| t == "respuesta").unwrap();
    let tool = texts.iter().position(|t| t.starts_with("View")).unwrap();
    let after = texts.iter().position(|t| t == "despues").unwrap();
    assert!(answer < tool && tool < after);
}

#[test]
fn job_activity_appears_and_updates_in_place() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    let mut running = job("build-1", JobStatus::Running, SessionId(1));
    running.progress = Some(JobProgress {
        fraction: 0.5,
        label: "building".into(),
    });
    app.apply(LiveEvent::JobChanged(running));
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(
        heads(&app),
        vec![("job  build".to_string(), ToolStatus::Running)]
    );
    assert!(texts(&app).iter().any(|t| t == "50% building"));

    let mut done = job("build-1", JobStatus::Completed, SessionId(1));
    done.result = Some(JobResult {
        content: "ok".into(),
    });
    app.apply(LiveEvent::JobChanged(done));
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(
        heads(&app),
        vec![("job  build".to_string(), ToolStatus::Success)]
    );
    let lines = texts(&app);
    assert!(lines.iter().any(|t| t == "ok"));
    assert!(!lines.iter().any(|t| t == "50% building"));
}

#[test]
fn subagent_activity_shows_label_status_and_result() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    // Los nodos activos se despliegan para mostrar su conversación y estado.
    assert_eq!(
        heads(&app),
        vec![(
            "▾ ● subagent  review (working)".to_string(),
            ToolStatus::Running
        )]
    );

    let mut done = sub("review", TaskState::Completed, SessionId(1));
    done.artifacts = vec![loomcoding_core::a2a::Artifact::text_artifact("listo")];
    app.apply(LiveEvent::SubagentChanged(done));
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(
        heads(&app),
        vec![(
            "▾ ✓ subagent  review (completed)".to_string(),
            ToolStatus::Success
        )]
    );
    // ADAPTADO: plegado, el resultado no va en el cuerpo; se ve como línea `result:` del nodo
    // al desplegarlo (ver `subagent_node_is_one_folded_line_by_default`).
    assert!(!texts(&app).iter().any(|t| t == "listo"));
    app.expanded_nodes.insert(9);
    app.refresh_child_node(9);
    assert!(texts(&app).iter().any(|t| t == "└─ ✓ result: listo"));
}

#[test]
fn tui_shows_the_same_result_text_as_before() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    let mut done = sub("review", TaskState::Completed, SessionId(1));
    done.artifacts = vec![
        loomcoding_core::a2a::Artifact::text_artifact("primera"),
        loomcoding_core::a2a::Artifact::text_artifact("segunda"),
    ];
    app.apply(LiveEvent::SubagentChanged(done));
    // ADAPTADO: el nodo muestra la primera línea del resultado en su línea `result:`; el
    // resultado entero sigue en la sesión del hijo. Sigue saliendo de los artefactos.
    app.expanded_nodes.insert(9);
    app.refresh_child_node(9);
    let lines = texts(&app);
    assert!(
        lines.iter().any(|t| t == "└─ ✓ result: primera"),
        "{lines:?}"
    );
    assert!(!lines.iter().any(|t| t.contains("segunda")), "{lines:?}");
}

#[test]
fn activity_status_covers_all_eight_states() {
    let esperados = [
        (TaskState::Submitted, ToolStatus::Running),
        (TaskState::Working, ToolStatus::Running),
        (TaskState::InputRequired, ToolStatus::Running),
        (TaskState::AuthRequired, ToolStatus::Running),
        (TaskState::Completed, ToolStatus::Success),
        (TaskState::Failed, ToolStatus::Error),
        (TaskState::Rejected, ToolStatus::Error),
        (TaskState::Canceled, ToolStatus::Canceled),
    ];
    assert_eq!(esperados.len(), TaskState::ALL.len());
    for (state, esperado) in esperados {
        assert_eq!(subagent_tool_status(state), esperado, "{state:?}");
    }
}
