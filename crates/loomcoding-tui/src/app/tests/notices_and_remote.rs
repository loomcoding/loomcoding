use super::*;

#[test]
fn provider_notices_are_hidden_by_default_and_drive_the_spinner() {
    let mut app = App::new();
    assert!(!app.verbose_provider_notices);
    app.apply(loomcoding_core::event::LiveEvent::RuntimeNotice {
        message: "provider request started for `m`".into(),
    });
    assert_eq!(app.provider_in_flight, 1);
    assert_eq!(app.transcript.entry_count(), 0);
    app.apply(loomcoding_core::event::LiveEvent::RuntimeNotice {
        message: "provider response received for `m`".into(),
    });
    assert_eq!(app.provider_in_flight, 0);
    assert_eq!(app.transcript.entry_count(), 0);
}

#[test]
fn provider_notices_are_shown_when_verbose() {
    let mut app = App::new().with_ui_config(loomcoding_core::config::UiConfig {
        verbose_provider_notices: true,
    });
    app.apply(loomcoding_core::event::LiveEvent::RuntimeNotice {
        message: "provider request started for `m`".into(),
    });
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(app.provider_in_flight, 1);
}

#[test]
fn other_runtime_notices_are_always_shown() {
    let mut app = App::new();
    app.apply(loomcoding_core::event::LiveEvent::RuntimeNotice {
        message: "subagent finished".into(),
    });
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(app.provider_in_flight, 0);
}

#[test]
fn mcp_capability_without_schema_tokens_deserializes_with_zero() {
    let server: McpServerCapability =
        serde_json::from_str(r#"{"id":"fs","initialized":true,"tool_count":3,"issue":null}"#)
            .unwrap();
    assert_eq!(server.schema_tokens, 0);
}

/// Aviso que llega por la sesión 1.
fn notice_for(text: &str) -> LiveEvent {
    LiveEvent::SessionMessageAdded {
        session: SessionId(1),
        message: Message::user(text),
    }
}

#[test]
fn activity_header_names_the_exact_a2a_state_in_parentheses() {
    for state in TaskState::ALL {
        let mut app = crush_app();
        app.session_id = Some(SessionId(1));
        app.apply(LiveEvent::SubagentChanged(sub(
            "review",
            state,
            SessionId(1),
        )));
        let nombre = agent_state(state).label();
        let glifo = agent_state(state).glyph();
        let expansion = if state.is_terminal() { "▸" } else { "▾" };
        assert_eq!(
            heads(&app),
            vec![(
                format!("{expansion} {glifo} subagent  review ({nombre})"),
                subagent_tool_status(state)
            )],
            "{state:?}"
        );
    }
}

/// Hijo de la sesión 1 con el agente y el origen indicados.
fn con_agente(agent: Option<&str>, origin: Option<&str>) -> SubagentDescriptor {
    let mut hijo = sub("review", TaskState::Working, SessionId(1));
    hijo.agent = agent.map(str::to_owned);
    hijo.agent_origin = origin.map(str::to_owned);
    hijo
}

#[test]
fn a_remote_row_shows_why_it_cannot_be_used() {
    use loomcoding_core::registry::{Execution, RemoteFetched, RemoteSpec};
    let remoto = |problema: Option<&str>| {
        Execution::Remote(RemoteSpec {
            endpoint: "https://ext.example".into(),
            fetched: Some(RemoteFetched {
                card_hash: "h".into(),
                jsonrpc_url: "https://ext.example/a2a".into(),
                tenant: None,
                requires_auth: true,
                security: None,
                auth_problem: problema.map(str::to_string),
                streaming: false,
            }),
            credentials: Default::default(),
        })
    };

    let (texto, _) = execution_label(&remoto(Some("falta la variable de entorno AGENTE_KEY")));
    assert_eq!(
        texto,
        "remote · auth: falta la variable de entorno AGENTE_KEY"
    );

    let (texto, _) = execution_label(&remoto(None));
    assert_eq!(texto, "remote");
}

#[test]
fn a_remote_reason_is_flattened_and_bounded() {
    use loomcoding_core::registry::{Execution, RemoteFetched, RemoteSpec};
    let largo = format!(
        "línea uno
línea dos {}",
        "x".repeat(500)
    );
    let remoto = Execution::Remote(RemoteSpec {
        endpoint: "https://ext.example".into(),
        fetched: Some(RemoteFetched {
            card_hash: "h".into(),
            jsonrpc_url: "https://ext.example/a2a".into(),
            tenant: None,
            requires_auth: true,
            security: None,
            auth_problem: Some(largo),
            streaming: false,
        }),
        credentials: Default::default(),
    });

    let (texto, _) = execution_label(&remoto);

    assert!(!texto.contains(char::from(10)), "{texto}");
    assert!(texto.chars().count() <= "remote · auth: ".chars().count() + 120);
}

#[test]
fn agent_tag_names_the_agent_and_flags_project_and_remote_origins() {
    assert_eq!(agent_tag(&con_agente(None, None)), None);
    assert_eq!(
        agent_tag(&con_agente(Some("default"), Some("built-in"))).as_deref(),
        Some("[default]")
    );
    assert_eq!(
        agent_tag(&con_agente(Some("rust"), Some("global:C:/agentes/rust.md"))).as_deref(),
        Some("[rust]")
    );
    assert_eq!(
        agent_tag(&con_agente(
            Some("front"),
            Some("project:.loomcoding/agents/front.md")
        ))
        .as_deref(),
        Some("[front·project]")
    );
    assert_eq!(
        agent_tag(&con_agente(
            Some("ext"),
            Some("remote:https://ext.example/a2a")
        ))
        .as_deref(),
        Some("[ext·remote]")
    );
    assert_eq!(
        agent_tag(&con_agente(
            Some("revisor"),
            Some("catalog:E:/catalogo/.agents/agents/revisor.md")
        ))
        .as_deref(),
        Some("[revisor·catalog]")
    );
}

#[test]
fn activity_header_keeps_the_a2a_state_last() {
    for state in TaskState::ALL {
        let mut app = crush_app();
        app.session_id = Some(SessionId(1));
        let mut hijo = sub("review", state, SessionId(1));
        hijo.agent = Some("front".into());
        app.apply(LiveEvent::SubagentChanged(hijo));
        let nombre = agent_state(state).label();
        let glifo = agent_state(state).glyph();
        let expansion = if state.is_terminal() { "▸" } else { "▾" };
        assert_eq!(
            heads(&app),
            vec![(
                format!("{expansion} {glifo} subagent  review [front] ({nombre})"),
                subagent_tool_status(state)
            )],
            "{state:?}"
        );
    }
}

#[test]
fn state_counts_are_exact_and_ordered() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    for (child, state) in [
        (10, TaskState::Failed),
        (11, TaskState::InputRequired),
        (12, TaskState::Working),
        (13, TaskState::Working),
    ] {
        app.subagents
            .insert(child, child_descriptor(child, state, SessionId(1)));
    }
    // Un subagente de otra sesión no entra en el recuento.
    app.subagents
        .insert(20, child_descriptor(20, TaskState::Working, SessionId(2)));
    assert_eq!(
        app.subagent_state_counts(),
        vec![
            (TaskState::Working, 2),
            (TaskState::InputRequired, 1),
            (TaskState::Failed, 1),
        ]
    );
    assert_eq!(
        app.running_summary(),
        "3 subagents running (working 2, input-required 1)"
    );
    assert_eq!(app.finished_summary(), " (subagents: failed 1)");
    assert_eq!(
        formato_recuento(&[(TaskState::Completed, 2), (TaskState::Canceled, 1)]),
        "completed 2, canceled 1"
    );

    // Sin subagentes en curso no hay paréntesis que ocultar en el resumen en curso.
    let mut terminados = App::new();
    terminados.session_id = Some(SessionId(1));
    terminados
        .subagents
        .insert(10, child_descriptor(10, TaskState::Completed, SessionId(1)));
    assert_eq!(terminados.running_summary(), "0 subagents running");
    assert_eq!(terminados.finished_summary(), " (subagents: completed 1)");
    assert_eq!(App::new().finished_summary(), "");
}

#[test]
fn active_agents_counts_only_non_terminal() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    for (indice, state) in TaskState::ALL.into_iter().enumerate() {
        let child = 10 + indice as i64;
        app.subagents
            .insert(child, child_descriptor(child, state, SessionId(1)));
    }
    // Un subagente en marcha de otra sesión no cuenta.
    app.subagents
        .insert(99, child_descriptor(99, TaskState::Working, SessionId(2)));
    assert_eq!(app.active_agents(), 4);
}

#[test]
fn rejected_canceled_and_auth_required_do_not_wake_the_parent() {
    for state in [
        TaskState::Rejected,
        TaskState::Canceled,
        TaskState::AuthRequired,
    ] {
        let mut app = App::new();
        app.session_id = Some(SessionId(1));
        app.apply(LiveEvent::SubagentChanged(child_descriptor(
            9,
            state,
            SessionId(1),
        )));
        app.apply(LiveEvent::SubagentSettled {
            parent: SessionId(1),
            child: SessionId(9),
            status: state,
            summary: None,
        });
        assert!(!app.wake_pending, "{state:?} no debe despertar al padre");
    }
}

#[test]
fn input_required_still_wakes_the_parent() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentInputRequested {
        parent: SessionId(1),
        child: SessionId(9),
        label: "obrero".into(),
    });
    assert!(app.wake_pending);
}

#[test]
fn job_snapshot_keeps_the_waiting_state_of_the_child() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::InputRequired,
        SessionId(1),
    )));
    // El job sigue en marcha: su snapshot no debe devolver al hijo a `working`.
    let mut en_marcha = job("subagent-1", JobStatus::Running, SessionId(1));
    en_marcha.kind = "subagent".into();
    app.apply(LiveEvent::JobChanged(en_marcha));
    assert_eq!(
        app.subagents[&9].status.state,
        TaskState::InputRequired,
        "el estado de espera es de la tarea, no del job"
    );
}

#[test]
fn foreign_and_subagent_kind_jobs_are_ignored() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::JobChanged(job(
        "build-1",
        JobStatus::Running,
        SessionId(2),
    )));
    let mut subagent_job = job("subagent-2", JobStatus::Running, SessionId(1));
    subagent_job.kind = "subagent".into();
    app.apply(LiveEvent::JobChanged(subagent_job));
    assert!(app.transcript.is_empty());
    assert_eq!(app.transcript.entry_count(), 0);
}

#[test]
fn live_notice_is_suppressed_when_the_item_exists() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    let mut done = sub("review", TaskState::Completed, SessionId(1));
    done.artifacts = vec![loomcoding_core::a2a::Artifact::text_artifact("listo")];
    app.apply(LiveEvent::SubagentChanged(done));
    let count = app.transcript.entry_count();
    app.apply(notice_for("[Subagent review completed]\nlisto"));
    assert!(!has_user_line(&app));
    assert_eq!(app.transcript.entry_count(), count);

    let mut finished = job("build-1", JobStatus::Completed, SessionId(1));
    finished.result = Some(JobResult {
        content: "ok".into(),
    });
    app.apply(LiveEvent::JobChanged(finished));
    let count = app.transcript.entry_count();
    app.apply(notice_for("[Job build-1 completed] build\nok"));
    assert!(!has_user_line(&app));
    assert_eq!(app.transcript.entry_count(), count);
}

#[test]
fn notice_before_event_does_not_duplicate() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(notice_for("[Subagent review completed]\nlisto"));
    let mut done = sub("review", TaskState::Completed, SessionId(1));
    done.artifacts = vec![loomcoding_core::a2a::Artifact::text_artifact("listo")];
    app.apply(LiveEvent::SubagentChanged(done));
    assert_eq!(app.transcript.entry_count(), 2);
    assert!(!has_user_line(&app));
}

#[test]
fn restored_notices_become_items_not_user_messages() {
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::human("hola"),
        Message::user("[Subagent review completed]\nready"),
        Message::user("[Subagent review needs input]\n¿Qué prefieres?"),
        Message::user("[Job j1 failed] build\nboom"),
    ]);
    let user_lines: Vec<String> = app
        .transcript
        .iter()
        .filter(|l| l.kind == loomcoding_cloth::state::LineKind::User)
        .map(|l| l.plain().to_string())
        .collect();
    assert_eq!(user_lines, vec!["hola".to_string()]);
    assert_eq!(
        heads(&app),
        vec![
            (
                "subagent  review (completed)".to_string(),
                ToolStatus::Success
            ),
            (
                "subagent  review (needs input)".to_string(),
                ToolStatus::Running
            ),
            ("job  build".to_string(), ToolStatus::Error),
        ]
    );
    let lines = texts(&app);
    let pos = |text: &str| lines.iter().position(|t| t == text).unwrap();
    assert!(pos("hola") < pos("subagent  review (completed)"));
    assert!(pos("subagent  review (completed)") < pos("job  build"));
    assert!(!lines.iter().any(|t| t == "ready"));
    assert!(lines.iter().any(|t| t == "boom"));
}

#[test]
fn restored_notices_with_the_same_label_each_create_an_item() {
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::user("[Subagent review completed]\nuno"),
        Message::user("[Subagent review completed]\ndos"),
    ]);
    let lines = texts(&app);
    assert_eq!(heads(&app).len(), 2);
    assert!(!lines.iter().any(|t| t == "uno"));
    assert!(!lines.iter().any(|t| t == "dos"));
    assert!(!has_user_line(&app));
}

#[test]
fn unparseable_notice_falls_back_to_a_plain_message() {
    let mut app = crush_app();
    app.apply(LiveEvent::MessageAdded(Message::human("[Job sin cerrar")));
    assert!(app.transcript.iter().any(
        |l| l.kind == loomcoding_cloth::state::LineKind::User && l.plain() == "[Job sin cerrar"
    ));
    assert!(heads(&app).is_empty());
}

#[test]
fn watch_batch_is_a_job_line_and_does_not_block_its_final_notice() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(notice_for(
        "[Job build-1 watch] FAIL uno\nwatch limit reached",
    ));
    assert!(heads(&app).is_empty());
    assert!(!has_user_line(&app));
    let lineas = texts(&app);
    assert!(lineas.iter().any(|t| t == "[Job build-1 watch] FAIL uno"));
    // Sin reconocerlo caería en el texto genérico de mensaje interno.
    assert!(!lineas.iter().any(|t| t == "[Mensaje interno]"));
    // El aviso de fin no se considera ya entregado por los lotes de vigilancia.
    app.apply(notice_for("[Job build-1 completed] build\nok"));
    assert_eq!(heads(&app).len(), 1);
}

#[test]
fn compaction_summary_renders_as_a_notice_not_as_user_text() {
    use loomcoding_core::agent::compact::summary_message;
    let mut app = crush_app();
    app.apply(LiveEvent::MessageAdded(summary_message(
        2,
        "resumen del tramo",
    )));
    assert!(!has_user_line(&app));
    assert!(!texts(&app)
        .iter()
        .any(|t| t.starts_with("[Context summary")));
    // Cabecera de Loomcoding, sin estado de A2A, y el resumen como cuerpo plegable.
    assert_eq!(
        heads(&app),
        vec![(
            "loomcoding  contexto compactado (2 mensajes resumidos)".to_string(),
            ToolStatus::Success
        )]
    );
    assert!(texts(&app).iter().any(|t| t == "resumen del tramo"));
}

#[test]
fn compaction_summary_in_classic_look_keeps_the_header() {
    use loomcoding_core::agent::compact::summary_message;
    let mut app = App::new(); // tema Cloth = Classic
    app.apply(LiveEvent::MessageAdded(summary_message(
        2,
        "resumen del tramo",
    )));
    let lines = texts(&app);
    assert!(lines
        .iter()
        .any(|t| t == "contexto compactado (2 mensajes resumidos)"));
    assert!(lines.iter().any(|t| t == "resumen del tramo"));
    assert!(!has_user_line(&app));
}

#[test]
fn ordinary_user_message_starting_with_a_bracket_is_untouched() {
    let mut app = crush_app();
    app.apply(LiveEvent::MessageAdded(Message::human("[nota] hola")));
    assert!(app
        .transcript
        .iter()
        .any(|l| l.kind == loomcoding_cloth::state::LineKind::User && l.plain() == "[nota] hola"));
    assert!(heads(&app).is_empty());
}

#[test]
fn reloading_a_session_with_a_summary_in_the_middle_shows_a_notice() {
    use loomcoding_core::agent::compact::summary_message;
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::human("primera"),
        summary_message(1, "resumen viejo"),
        Message::human("segunda"),
    ]);
    let users: Vec<String> = app
        .transcript
        .iter()
        .filter(|l| l.kind == loomcoding_cloth::state::LineKind::User)
        .map(|l| l.plain().to_string())
        .collect();
    assert_eq!(users, vec!["primera", "segunda"]);
    assert_eq!(
        heads(&app),
        vec![(
            "loomcoding  contexto compactado (1 mensaje resumido)".to_string(),
            ToolStatus::Success
        )]
    );
    assert!(texts(&app).iter().any(|t| t == "resumen viejo"));
}

#[test]
fn restored_summary_does_not_enter_the_composer_history() {
    use loomcoding_core::agent::compact::summary_message;
    let mut app = crush_app();
    app.restore_messages(vec![
        Message::human("primera"),
        summary_message(1, "resumen viejo"),
        Message::human("segunda"),
    ]);
    assert_eq!(app.history.len(), 2, "solo los dos mensajes de la persona");
    assert_eq!(app.history.prev(""), Some("segunda"));
    assert_eq!(app.history.prev(""), Some("primera"));
}

#[test]
fn classic_look_shows_notices_as_system_lines_and_the_activity_header() {
    let mut app = App::new(); // tema Cloth = Classic
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    app.apply(notice_for("[Subagent review completed]\nready"));
    let lines = texts(&app);
    assert!(lines
        .iter()
        .any(|t| t == "[Subagent review completed; open its chat to read the response]"));
    // El aviso lo emite el sistema: nunca se atribuye a la persona usuaria.
    let user_count = app
        .transcript
        .iter()
        .filter(|l| l.kind == loomcoding_cloth::state::LineKind::User)
        .count();
    assert_eq!(user_count, 0, "ningún aviso del sistema sale como «You»");
    let avisos = app
        .transcript
        .iter()
        .filter(|l| l.kind == loomcoding_cloth::state::LineKind::System)
        .count();
    assert_eq!(avisos, 1, "el resumen queda como una línea de sistema");
    assert_eq!(
        heads(&app).len(),
        1,
        "la actividad del subagente deja su cabecera"
    );
    // ADAPTADO: la entrada viva del subagente es ahora un nodo (`▸ ● …`); el aviso la reclama
    // sin crear otra. Sigue comprobándose que la cabecera nombra al subagente.
    assert!(heads(&app)[0].0.starts_with("▾ ● subagent  review"));
}

#[test]
fn switching_look_keeps_the_activity_header() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::SubagentChanged(sub(
        "review",
        TaskState::Working,
        SessionId(1),
    )));
    assert_eq!(heads(&app).len(), 1);
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Cloth);
    app.sync_all_tools();
    // El nodo activo queda desplegado por defecto, también al cambiar de look.
    let esperado = vec![(
        "▾ ● subagent  review (working)".to_string(),
        ToolStatus::Running,
    )];
    assert_eq!(
        heads(&app),
        esperado,
        "la cabecera sobrevive al look clásico"
    );
    app.theme =
        loomcoding_cloth::widgets::Theme::preset(loomcoding_cloth::widgets::ThemePreset::Crush);
    app.sync_all_tools();
    assert_eq!(heads(&app), esperado);
}
