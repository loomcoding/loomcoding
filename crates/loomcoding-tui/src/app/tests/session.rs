use super::*;

#[test]
fn deltas_accumulate_into_one_item() {
    let mut app = App::new();
    app.apply(loomcoding_core::event::LiveEvent::Delta { text: "a".into() });
    app.apply(loomcoding_core::event::LiveEvent::Delta { text: "b".into() });
    assert_eq!(app.transcript.entry_count(), 1);
    assert_eq!(app.transcript.line(0).unwrap().plain(), "ab");
}

#[test]
fn submit_clears_composer() {
    let mut app = App::new();
    app.push_char('h');
    app.push_char('i');
    assert_eq!(app.submit(), Some("hi".to_string()));
    assert!(app.composer.is_empty());
    assert_eq!(app.submit(), None);
}

#[test]
fn user_line_comes_from_the_event_not_from_submit() {
    let mut app = App::new();
    app.push_char('h');
    app.push_char('i');
    assert_eq!(app.submit(), Some("hi".to_string()));
    assert!(app.transcript.is_empty());
    app.apply(loomcoding_core::event::LiveEvent::MessageAdded(
        loomcoding_core::message::Message::human("hi"),
    ));
    assert_eq!(
        app.transcript.line(0).unwrap().kind,
        loomcoding_cloth::state::LineKind::User
    );
}

#[test]
fn development_workflow_events_show_phase_agent_and_pending_decision() {
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(LiveEvent::DevelopmentWorkflowChanged(
        loomcoding_core::workflow::WorkflowSummary {
            id: loomcoding_core::workflow::WorkflowId(3),
            parent: SessionId(7),
            phase: loomcoding_core::workflow::WorkflowPhase::AwaitingPlanApproval,
            planner: "planner".into(),
            implementer: "builder".into(),
            reviewer: "reviewer".into(),
            active_child: None,
            correction_round: 1,
            spec_path: Some("docs/spec.md".into()),
            plan_path: Some("docs/plan.md".into()),
            updated_at_ms: 0,
        },
    ));

    let line = app.transcript.line(0).unwrap().plain();
    assert!(line.contains("aprobar spec+plan"));
    assert!(line.contains("coordinador"));
    assert!(line.contains("ronda 1"));
}

fn gauntlet_summary(
    phase: loomcoding_core::gauntlet::GauntletPhase,
    active_child: Option<SessionId>,
) -> loomcoding_core::gauntlet::GauntletSummary {
    loomcoding_core::gauntlet::GauntletSummary {
        id: loomcoding_core::gauntlet::GauntletId(3),
        parent: SessionId(7),
        phase,
        round: 1,
        max_rounds: 2,
        builder: "constructor".into(),
        critic: "critico".into(),
        active_child,
        updated_at_ms: 0,
    }
}

#[test]
fn gauntlet_events_show_phase_agent_with_its_a2a_state_round_and_actor() {
    use loomcoding_core::gauntlet::GauntletPhase;
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(LiveEvent::SubagentChanged(
        loomcoding_core::session::SubagentDescriptor {
            phase: loomcoding_core::subagent_phase::SubagentPhase::Streaming,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: SessionId(7),
            child: SessionId(11),
            job_id: "job-11".into(),
            label: "gauntlet critic".into(),
            status: loomcoding_core::task::TaskStatus::new(
                loomcoding_core::task::TaskState::Working,
            ),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: Some("critico".into()),
            agent_origin: None,
            reference_task_ids: Vec::new(),
        },
    ));
    let before = app.transcript.len();
    app.apply(LiveEvent::GauntletChanged {
        summary: gauntlet_summary(GauntletPhase::Judging, Some(SessionId(11))),
        actor: "critic".into(),
        detail: "critic launched".into(),
    });
    let line = app.transcript.line(before).unwrap().plain();
    assert!(line.contains("gauntlet #3"), "{line}");
    assert!(line.contains("juzgando"), "{line}");
    assert!(line.contains("critico (working)"), "{line}");
    assert!(line.contains("ronda 1/2"), "{line}");
    assert!(line.contains("critic: critic launched"), "{line}");
}

#[test]
fn gauntlet_without_a_running_child_is_attributed_to_the_coordinator_and_shows_the_gap() {
    use loomcoding_core::gauntlet::GauntletPhase;
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(LiveEvent::GauntletChanged {
        summary: gauntlet_summary(GauntletPhase::AwaitingFinalAcceptance, None),
        actor: "critic".into(),
        detail: "no gap, confirmed with the order swapped".into(),
    });
    let line = app.transcript.line(0).unwrap().plain();
    assert!(line.contains("aceptar resultado"), "{line}");
    assert!(line.contains("coordinador"), "{line}");
    assert!(line.contains("confirmed with the order swapped"), "{line}");
}

#[test]
fn a_replayed_gauntlet_without_actor_prints_no_dangling_separator() {
    use loomcoding_core::gauntlet::GauntletPhase;
    let mut app = App::new();
    app.session_id = Some(SessionId(7));
    app.apply(LiveEvent::GauntletChanged {
        summary: gauntlet_summary(GauntletPhase::Stopped, None),
        actor: String::new(),
        detail: String::new(),
    });
    let line = app.transcript.line(0).unwrap().plain();
    assert!(line.contains("detenido"), "{line}");
    assert!(!line.trim_end().ends_with('·'), "{line}");
    assert!(!line.contains(": "), "{line}");
}

#[test]
fn gauntlet_events_of_another_session_are_ignored() {
    use loomcoding_core::gauntlet::GauntletPhase;
    let mut app = App::new();
    app.session_id = Some(SessionId(9));
    app.apply(LiveEvent::GauntletChanged {
        summary: gauntlet_summary(GauntletPhase::Judging, None),
        actor: "critic".into(),
        detail: "x".into(),
    });
    assert!(app.transcript.is_empty());
}

#[test]
fn scoped_subagent_notices_only_reach_their_session() {
    let mut app = App::new();
    app.session_id = Some(SessionId(2));
    let message = Message::user("[Subagent review completed]\nready");
    app.apply(LiveEvent::SessionMessageAdded {
        session: SessionId(1),
        message: message.clone(),
    });
    assert!(app.transcript.is_empty());
    app.apply(LiveEvent::SessionMessageAdded {
        session: SessionId(2),
        message,
    });
    assert_eq!(
        app.transcript.line(0).unwrap().plain(),
        "[Subagent review completed; open its chat to read the response]"
    );
    assert_eq!(app.transcript.entry_count(), 1);
}

#[test]
fn loading_a_session_restores_and_scopes_durable_subagents() {
    let mut app = App::new();
    let parent = SessionId(2);
    let child = SessionId(9);
    app.load_session(parent, Vec::new());
    app.load_subagents(
        parent,
        vec![loomcoding_core::session::SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent,
            child,
            job_id: "subagent-1".into(),
            label: "review".into(),
            status: TaskStatus::new(TaskState::Failed),
            artifacts: vec![loomcoding_core::a2a::Artifact::text_artifact(
                "partial output",
            )],
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        }],
    );
    assert_eq!(
        app.subagents.get(&child.0).unwrap().status.state,
        TaskState::Failed
    );

    app.load_session(SessionId(3), Vec::new());
    assert!(app.subagents.is_empty());
}

fn rail_child(parent: SessionId, child: i64) -> SubagentDescriptor {
    SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent,
        child: SessionId(child),
        job_id: format!("subagent-{child}"),
        label: "review".into(),
        status: TaskStatus::new(TaskState::Working),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }
}

/// Sesión `parent` abierta con los hijos `children` ya cargados en el rail.
fn app_with_children(parent: SessionId, children: &[i64]) -> App {
    let mut app = App::new();
    app.load_session(parent, Vec::new());
    app.load_subagents(
        parent,
        children
            .iter()
            .map(|&child| rail_child(parent, child))
            .collect(),
    );
    app
}

#[test]
fn load_session_preserva_borrador_del_padre() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.composer.insert_str("para el padre");
    // Abrir un hijo cambia de sesión: el texto es del padre, no del hijo.
    app.load_session(SessionId(7), Vec::new());
    assert!(
        app.composer.is_empty(),
        "el hijo no hereda el borrador del padre"
    );
    app.composer.insert_str("para el hijo");
    // Volver al padre restaura su borrador, y el hijo conserva el suyo.
    app.load_session(SessionId(1), Vec::new());
    assert_eq!(app.composer.text(), "para el padre");
    app.load_session(SessionId(7), Vec::new());
    assert_eq!(app.composer.text(), "para el hijo");
}

#[test]
fn recargar_la_misma_sesion_no_toca_el_borrador() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.composer.insert_str("sigue aquí");
    app.load_session(SessionId(1), Vec::new());
    assert_eq!(app.composer.text(), "sigue aquí");
}

#[test]
fn el_borrador_restaurado_conserva_el_cursor() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.composer.insert_str("hola mundo");
    app.composer.set_cursor(4);
    app.load_session(SessionId(7), Vec::new());
    app.load_session(SessionId(1), Vec::new());
    assert_eq!(app.composer.text(), "hola mundo");
    assert_eq!(app.composer.cursor(), 4);
}

#[test]
fn las_imagenes_del_borrador_viajan_con_su_sesion() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.draft_images.push(ContentPart::Image {
        mime_type: "image/png".into(),
        data: "AAAA".into(),
        filename: "a.png".into(),
    });
    app.load_session(SessionId(7), Vec::new());
    assert!(app.draft_images.is_empty(), "el hijo no hereda la imagen");
    app.load_session(SessionId(1), Vec::new());
    assert_eq!(app.draft_images.len(), 1);
}

#[test]
fn un_borrador_vacio_no_deja_entrada_guardada() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.load_session(SessionId(7), Vec::new());
    app.load_session(SessionId(1), Vec::new());
    assert!(
        app.composer_drafts.is_empty(),
        "sin texto ni imágenes no se guarda nada"
    );
}

#[test]
fn cambiar_de_sesion_no_apaga_el_indicador_de_trabajo() {
    let mut app = App::new();
    app.load_session(SessionId(1), Vec::new());
    app.composer.set_working(true);
    app.load_session(SessionId(7), Vec::new());
    assert!(
        app.composer.is_working(),
        "el estado de trabajo es del agente, no del borrador"
    );
}

#[test]
fn open_child_pushes_parent_and_requests_resume() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(app.open_child(7));
    assert_eq!(app.parent_stack, vec![parent]);
    assert_eq!(
        app.session_choice,
        Some(SessionChoice::Resume(SessionId(7)))
    );
}

#[test]
fn open_child_refuses_while_turn_in_flight() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    app.turn_in_flight = true;
    assert!(!app.open_child(7));
    assert!(app.parent_stack.is_empty());
    assert_eq!(app.session_choice, None);
}

#[test]
fn open_child_refuses_unknown_child() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(!app.open_child(99));
    assert!(app.parent_stack.is_empty());
    assert_eq!(app.session_choice, None);

    // Sin sesión abierta tampoco hay a dónde volver: se niega.
    assert!(!App::new().open_child(7));
}

#[test]
fn go_parent_pops_and_requests_resume() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(app.open_child(7));
    // La selección ya se despachó: el worker carga el hijo.
    app.session_choice = None;
    app.load_session(SessionId(7), Vec::new());

    assert!(app.go_parent());
    assert!(app.parent_stack.is_empty());
    assert_eq!(app.session_choice, Some(SessionChoice::Resume(parent)));
}

#[test]
fn restored_internal_prompt_is_not_user_and_human_message_is() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(app.open_child(7));
    app.load_session(
        SessionId(7),
        vec![
            Message::user("revisa estos archivos"),
            Message::human("hola"),
        ],
    );
    let users: Vec<_> = app
        .transcript
        .iter()
        .filter(|line| line.kind == loomcoding_cloth::state::LineKind::User)
        .map(|line| line.plain())
        .collect();
    assert_eq!(users, ["hola"]);
    assert!(texts(&app).iter().any(|text| text == "[Mensaje interno]"));
    assert!(texts(&app)
        .iter()
        .any(|text| text == "revisa estos archivos"));

    assert!(app.go_parent());
    app.load_session(parent, vec![Message::human("mensaje escrito por mí")]);
    assert!(has_user_line(&app));
    assert!(texts(&app)
        .iter()
        .any(|text| text == "mensaje escrito por mí"));
}

#[test]
fn live_internal_user_role_is_not_attributed_to_the_user() {
    let mut app = App::new();
    app.apply(LiveEvent::MessageAdded(Message::user("texto interno")));
    app.apply(LiveEvent::MessageAdded(Message::human("texto humano")));
    app.apply(LiveEvent::MessageAdded(Message::human(
        "[Subagent review completed]\ntexto que sí envié",
    )));
    let users: Vec<_> = app
        .transcript
        .iter()
        .filter(|line| line.kind == loomcoding_cloth::state::LineKind::User)
        .map(|line| line.plain())
        .collect();
    assert_eq!(
        users,
        [
            "texto humano",
            "[Subagent review completed]",
            "texto que sí envié"
        ]
    );
}

#[test]
fn go_parent_refuses_with_empty_stack() {
    let mut app = App::new();
    assert!(!app.go_parent());
    assert_eq!(app.session_choice, None);
}

#[test]
fn go_parent_refuses_while_turn_in_flight() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(app.open_child(7));
    app.session_choice = None;
    app.load_session(SessionId(7), Vec::new());
    app.turn_in_flight = true;

    assert!(!app.go_parent());
    assert_eq!(app.parent_stack, vec![parent]);
    assert_eq!(app.session_choice, None);
}

#[test]
fn normal_load_session_clears_parent_stack() {
    let mut app = App::new();
    app.parent_stack.push(SessionId(1));
    app.rail_selected = Some(3);
    app.load_session(SessionId(5), Vec::new());
    assert!(app.parent_stack.is_empty());
    assert_eq!(app.rail_selected, None);
}

#[test]
fn navigation_load_keeps_parent_stack() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[7]);
    assert!(app.open_child(7));
    app.session_choice = None;

    // La carga que pide `open_child` conserva la pila...
    app.load_session(SessionId(7), Vec::new());
    assert_eq!(app.parent_stack, vec![parent]);

    // ...pero cualquier otra carga posterior la vacía.
    app.load_session(SessionId(8), Vec::new());
    assert!(app.parent_stack.is_empty());
}

#[test]
fn rail_move_walks_children_without_wrapping() {
    let mut app = app_with_children(SessionId(1), &[3, 5, 9]);
    assert_eq!(app.rail_selected, None);

    app.rail_move(1);
    assert_eq!(app.rail_selected, Some(3));
    app.rail_move(1);
    assert_eq!(app.rail_selected, Some(5));
    app.rail_move(1);
    assert_eq!(app.rail_selected, Some(9));
    // Al final no da la vuelta.
    app.rail_move(1);
    assert_eq!(app.rail_selected, Some(9));

    app.rail_move(-1);
    assert_eq!(app.rail_selected, Some(5));
    app.rail_move(-1);
    assert_eq!(app.rail_selected, Some(3));
    app.rail_move(-1);
    assert_eq!(app.rail_selected, Some(3));
}

#[test]
fn rail_move_without_children_clears_selection() {
    let mut app = App::new();
    app.rail_selected = Some(3);
    app.rail_move(1);
    assert_eq!(app.rail_selected, None);
}

#[test]
fn rail_selection_dropped_when_child_disappears() {
    let parent = SessionId(1);
    let mut app = app_with_children(parent, &[3, 5]);
    app.rail_selected = Some(5);
    app.load_subagents(parent, vec![rail_child(parent, 3)]);
    assert_eq!(app.rail_selected, None);

    app.rail_selected = Some(3);
    app.load_subagents(parent, vec![rail_child(parent, 3)]);
    assert_eq!(app.rail_selected, Some(3));
}

#[test]
fn a_user_message_starts_a_new_answer_entry() {
    let mut app = App::new();
    app.push_assistant("uno");
    app.push_user("pregunta");
    app.push_assistant("dos");
    assert_eq!(app.transcript.entry_count(), 3);
    assert_eq!(app.transcript.line(2).unwrap().plain(), "dos");
}

#[test]
fn restores_transcript_history_and_tool_results() {
    let mut app = App::new();
    app.turn_input_tokens = 99;
    app.turn_output_tokens = 50;
    app.restore_messages(vec![
        Message::human("hola"),
        Message::assistant(vec![
            ContentPart::Text {
                text: "respuesta".into(),
            },
            ContentPart::ToolCall {
                id: "c1".into(),
                name: "view".into(),
                input: serde_json::json!({"path":"src/main.rs"}),
            },
        ]),
        Message::new(
            Role::Tool,
            vec![ContentPart::ToolResult {
                call_id: "c1".into(),
                content: "contenido".into(),
                is_error: false,
            }],
        ),
    ]);

    // Con el look clásico la llamada deja solo su cabecera, entre los dos mensajes: el
    // transcript es el diario de toda iteración.
    assert_eq!(app.transcript.len(), 3);
    assert_eq!(app.transcript.line(0).unwrap().plain(), "hola");
    assert_eq!(app.transcript.line(1).unwrap().plain(), "respuesta");
    assert!(app
        .transcript
        .line(2)
        .unwrap()
        .plain()
        .to_lowercase()
        .contains("view"));
    assert_eq!(app.history.prev(""), Some("hola"));
    assert_eq!(app.tools.len(), 1);
    assert_eq!(app.tools[0].output.as_deref(), Some("contenido"));
    assert_eq!(
        app.tools[0].status(),
        loomcoding_cloth::state::ToolStatus::Success
    );
    assert_eq!((app.turn_input_tokens, app.turn_output_tokens), (0, 0));
}

#[test]
fn classic_look_shows_the_subagent_call_with_its_agent_in_the_conversation() {
    let mut app = App::new(); // tema Cloth = Classic
    app.restore_messages(vec![
        Message::user("lee un fichero"),
        Message::assistant(vec![ContentPart::ToolCall {
            id: "c1".into(),
            name: "subagent".into(),
            input: serde_json::json!({
                "prompt": "lee rustfmt.toml",
                "label": "leer fichero",
                "agent": "rust"
            }),
        }]),
    ]);
    let texts: Vec<String> = app
        .transcript
        .iter()
        .map(|l| l.plain().to_string())
        .collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("subagent") && t.contains("[rust] leer fichero")),
        "la llamada a subagent debe verse con su agente: {texts:?}"
    );
}

#[test]
fn empty_session_list_starts_a_new_session_and_nonempty_list_opens_picker() {
    let mut app = App::new();
    app.set_sessions(Vec::new());
    assert!(app.session_loading);
    assert_eq!(app.session_choice, Some(SessionChoice::New));

    app.set_sessions(vec![SessionInfo {
        id: SessionId(1),
        preview: "pregunta anterior".into(),
        message_count: 2,
    }]);
    assert!(app.session_picker_open);
    assert!(!app.session_loading);
}

#[test]
fn updating_sessions_keeps_selected_session_or_new_choice() {
    let mut app = App::new();
    app.set_sessions(vec![
        SessionInfo {
            id: SessionId(1),
            preview: "one".into(),
            message_count: 1,
        },
        SessionInfo {
            id: SessionId(2),
            preview: "two".into(),
            message_count: 1,
        },
    ]);
    app.session_selected = 1;
    app.update_sessions(vec![
        SessionInfo {
            id: SessionId(2),
            preview: "two".into(),
            message_count: 1,
        },
        SessionInfo {
            id: SessionId(1),
            preview: "one".into(),
            message_count: 1,
        },
    ]);
    assert_eq!(app.session_selected, 0);

    app.session_selected = 2;
    app.update_sessions(vec![SessionInfo {
        id: SessionId(2),
        preview: "two".into(),
        message_count: 1,
    }]);
    assert_eq!(app.session_selected, 1);
}

#[test]
fn loading_session_clears_picker_search_state() {
    let mut app = App::new();
    app.changes.upsert(loomcoding_cloth::state::FileChange {
        path: "old.rs".into(),
        added: 1,
        removed: 0,
    });
    app.session_selected = 3;
    app.session_query = "old filter".into();
    app.session_searching = true;
    app.session_choice = Some(SessionChoice::New);
    app.session_loading = true;
    app.session_picker_open = true;
    app.session_picker_return_to_chat = true;

    app.load_session(SessionId(7), Vec::new());

    assert_eq!(app.session_id, Some(SessionId(7)));
    assert_eq!(app.session_selected, 0);
    assert!(app.session_query.is_empty());
    assert!(!app.session_searching);
    assert_eq!(app.session_choice, None);
    assert!(!app.session_loading);
    assert!(!app.session_picker_open);
    assert!(!app.session_picker_return_to_chat);
    assert!(app.changes.is_empty());
}

#[test]
fn failed_session_load_reopens_picker_when_sessions_exist() {
    let mut app = App::new();
    app.sessions = vec![SessionInfo {
        id: SessionId(1),
        preview: "previous".into(),
        message_count: 2,
    }];
    app.session_loading = true;
    app.session_picker_return_to_chat = true;

    app.session_load_failed();

    assert!(!app.session_loading);
    assert!(app.session_picker_open);
    assert!(!app.session_picker_return_to_chat);
}
