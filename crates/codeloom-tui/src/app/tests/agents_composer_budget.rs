use super::*;

#[test]
fn with_context_stores_cwd_and_model() {
    let app = App::with_context("/tmp/proj", "mistral-nemo");
    assert_eq!(app.cwd, "/tmp/proj");
    assert_eq!(app.model, "mistral-nemo");
    assert_eq!(app.status, "ready");
}

#[test]
fn sidebar_has_logo_and_session_sections() {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.session_id = Some(SessionId(42));
    let sb = app.sidebar();
    assert_eq!(sb.logo.as_ref().unwrap().name, "CodeLoom");
    let titles: Vec<&str> = sb.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["AGENT", "PROJECT", "FLOCK", "SESSION"]);
    assert_eq!(sb.sections[1].lines[0], "/proj");
    assert_eq!(sb.sections[3].lines[0], "session #42");
}

#[test]
fn show_agents_lists_the_registry_and_approves_a_project_agent() {
    use codeloom_core::registry::{
        AgentRegistry, Approval, BuiltInSource, FileSource, ProfileLevel, TrustStore,
    };
    let mut app = App::new();
    app.show_agents();
    assert!(app.agents_open);
    assert!(app.agent_rows.is_empty());

    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    std::fs::write(
        proyecto.path().join("front.md"),
        "---
name: front
description: Interfaz
---

Cuerpo de front.
",
    )
    .unwrap();
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            proyecto.path().to_path_buf(),
            ProfileLevel::Project,
        )),
    ])
    .with_trust(
        TrustStore::open(datos.path().to_path_buf()).unwrap(),
        proyecto.path().to_path_buf(),
        false,
    );
    app.set_agent_registry(Arc::new(registro));
    app.show_agents();
    assert_eq!(app.agent_rows.len(), 2);
    assert_eq!(app.agent_rows[0].name, "default");
    assert_eq!(app.agent_rows[1].name, "front");
    assert_eq!(app.agent_rows[1].approval, Approval::Pending);
    assert!(app.agent_rows[1].body.contains("Cuerpo de front."));

    app.agents_move(1);
    app.agents_move(5);
    assert_eq!(app.agents_selected, 1, "la selección no sale de la lista");
    app.agents_decide(true);
    assert_eq!(app.agent_rows[1].approval, Approval::Approved);
    assert_eq!(app.agents_status.as_deref(), Some("Approved `front`"));
    app.agents_decide(false);
    assert_eq!(app.agent_rows[1].approval, Approval::Pending);

    // `default` no es de proyecto: no se puede aprobar y se explica por qué.
    app.agents_move(-1);
    app.agents_decide(true);
    assert!(app
        .agents_status
        .as_deref()
        .is_some_and(|s| s.starts_with("Not changed:")));
}

#[test]
fn paste_inserts_normalized_text_without_sending() {
    let mut app = App::new();
    app.paste("a\r\nb\rc");
    assert_eq!(app.composer.text(), "a\nb\nc");
}

#[test]
fn image_attachment_checks_signature_and_adds_badge() {
    let mut app = App::new();
    app.attach_image_bytes(b"\x89PNG\r\n\x1a\nimage", "clipboard.png")
        .unwrap();
    assert!(
        matches!(app.draft_images.as_slice(), [ContentPart::Image { mime_type, .. }] if mime_type == "image/png")
    );
    assert!(app.composer.is_empty());
    assert!(app
        .attach_image_bytes(b"not an image", "clipboard.png")
        .is_err());
}

#[test]
fn large_paste_shows_marker_but_submits_original_text() {
    let mut app = App::new();
    let pasted = "x".repeat(1001);
    app.paste(&pasted);
    assert_eq!(app.composer.text(), "[Pasted Content 1001 chars]");
    assert_eq!(app.submit(), Some(pasted));
}

#[test]
fn paste_is_ignored_without_composer_focus() {
    let mut app = App::new();
    app.toggle_focus();
    app.paste("x");
    assert_eq!(app.composer.text(), "");
}

#[test]
fn turn_start_does_not_mutate_pending_prompt_count() {
    let mut app = App::new();
    app.queued = 2;
    app.apply(codeloom_core::event::LiveEvent::TurnStarted);
    assert_eq!(app.queued, 2);
}

#[test]
fn token_usage_tracks_session_and_current_turn_separately() {
    let mut app = App::new();
    app.apply(LiveEvent::Usage {
        input_tokens: 10,
        output_tokens: 4,
    });
    app.apply(LiveEvent::TurnStarted);
    app.apply(LiveEvent::Usage {
        input_tokens: 7,
        output_tokens: 2,
    });
    assert_eq!((app.turn_input_tokens, app.turn_output_tokens), (7, 2));
    assert_eq!((app.input_tokens, app.output_tokens), (17, 6));
    app.apply(LiveEvent::TurnStarted);
    assert_eq!((app.turn_input_tokens, app.turn_output_tokens), (0, 0));
    assert_eq!((app.input_tokens, app.output_tokens), (17, 6));
}

#[test]
fn closed_provider_phases_accumulate_until_the_turn_ends() {
    let mut app = App::new();
    app.apply(LiveEvent::StepStarted {
        step: 1,
        max_steps: 2,
    });
    app.phase_started_at = Some(Instant::now() - Duration::from_secs(2));
    app.apply(LiveEvent::ReasoningDelta {
        text: "thinking".into(),
    });
    assert_eq!(app.phase_durations.len(), 1);
    assert_eq!(app.phase_durations[0].0, "Provider");
    assert!(app.phase_durations[0].1 >= Duration::from_secs(2));

    app.apply(LiveEvent::TurnStarted);
    assert!(app.phase_durations.is_empty());
}

#[test]
fn applies_file_diffs() {
    let mut app = App::new();
    app.apply(codeloom_core::event::LiveEvent::FileDiff {
        path: "src/main.rs".into(),
        added: 3,
        removed: 1,
    });
    assert_eq!(app.changes.len(), 1);
    assert_eq!(app.changes.totals(), (3, 1));
}

#[test]
fn provider_error_stops_thinking_state() {
    let mut app = App::new();
    app.turn_in_flight = true;
    app.thinking = true;
    app.apply(LiveEvent::Error {
        message: "provider unavailable".into(),
    });
    assert!(!app.turn_in_flight);
    assert!(!app.thinking);
    assert_eq!(app.status, "error: provider unavailable");
    assert!(system_lines(&app)
        .iter()
        .any(|line| line == "error: provider unavailable"));
    app.apply(LiveEvent::TurnEnded);
    assert_eq!(app.status, "ready");
    assert!(system_lines(&app)
        .iter()
        .any(|line| line == "error: provider unavailable"));
}

#[test]
fn an_automatic_agent_choice_is_shown_with_who_decided_and_why() {
    let mut app = idle_parent_app();
    let padre = app.session_id.unwrap();
    app.apply(LiveEvent::AgentRouted {
        parent: padre,
        agent: "revisor".into(),
        reason: "regla: coincide con «review»".into(),
        source: "rule".into(),
    });
    // La de otra sesión no se pinta aquí.
    app.apply(LiveEvent::AgentRouted {
        parent: SessionId(padre.0 + 100),
        agent: "otro".into(),
        reason: "x".into(),
        source: "rule".into(),
    });

    let lineas = system_lines(&app);
    assert_eq!(lineas.len(), 1, "{lineas:?}");
    assert!(lineas[0].contains("revisor") && lineas[0].contains("rule"));
    assert!(lineas[0].contains("review"));
}

#[test]
fn a_manual_card_refresh_reports_how_it_went() {
    let mut app = crush_app();
    app.apply(LiveEvent::AgentCardsRefreshed {
        ok: 3,
        failed: vec![],
    });
    app.apply(LiveEvent::AgentCardsRefreshed {
        ok: 1,
        failed: vec!["rutas".into(), "clima".into()],
    });

    let lineas = texts(&app);
    assert_eq!(lineas[0], "tarjetas de agentes remotos al día: 3");
    assert!(
        lineas[1].contains("1 al día") && lineas[1].contains("rutas, clima"),
        "{lineas:?}"
    );
}

#[test]
fn a2a_observation_history_stays_bounded_and_keeps_the_newest_events() {
    let mut app = App::new();
    for id in 0..(MAX_A2A_OBSERVATIONS + 2) {
        app.apply(LiveEvent::A2aObserved(A2aObservation {
            direction: A2aDirection::Outbound,
            agent: "remote".into(),
            operation: A2aOperation::GetTask,
            rpc_id: Some(id.to_string()),
            task_id: Some(id.to_string()),
            context_id: None,
            state: Some("working".into()),
            artifact: None,
            succeeded: true,
            elapsed_ms: Some(1),
        }));
    }

    assert_eq!(app.a2a_observations.len(), MAX_A2A_OBSERVATIONS);
    assert_eq!(
        app.a2a_observations.front().unwrap().task_id.as_deref(),
        Some("2")
    );
    assert_eq!(
        app.a2a_observations.back().unwrap().task_id.as_deref(),
        Some("129")
    );
}

#[test]
fn refresh_agents_calls_the_hook_once_or_says_it_is_unavailable() {
    let mut app = crush_app();
    app.request_refresh_agents();
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Remote agents are not enabled ([a2a] role)")
    );

    let llamadas = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let contador = llamadas.clone();
    app.set_refresh_agents_hook(Arc::new(move || {
        contador.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    app.request_refresh_agents();

    assert_eq!(llamadas.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(app
        .slash_feedback
        .as_deref()
        .unwrap()
        .starts_with("Refreshing"));
}

#[test]
fn an_incoming_a2a_task_is_shown_as_a_system_line() {
    let mut app = crush_app();
    app.apply(LiveEvent::A2aServed {
        task: "12".into(),
        agent: "default".into(),
        state: "TASK_STATE_SUBMITTED".into(),
    });
    assert_eq!(
        texts(&app),
        vec!["A2A entrante: tarea 12 · agente default · TASK_STATE_SUBMITTED".to_string()]
    );
    assert_eq!(
        app.transcript.line(0).unwrap().kind,
        batten::state::LineKind::System
    );
}

#[test]
fn budget_warning_renders_as_a_system_line_not_as_user_text() {
    let mut app = crush_app();
    app.apply(LiveEvent::BudgetWarning {
        metric: BudgetMetric::Tokens,
        level_percent: 80,
        used: 400_123.0,
        limit: 500_000.0,
        partial: false,
    });
    assert_eq!(
        texts(&app),
        vec![
            "aviso de presupuesto: 80 % de 500k tokens de la sesión y sus subagentes (400k usados)"
                .to_string()
        ]
    );
    assert_eq!(
        app.transcript.line(0).unwrap().kind,
        batten::state::LineKind::System
    );
    assert!(!has_user_line(&app));
}

#[test]
fn a_budget_warning_crossed_by_a_child_is_shown_once_as_a_system_line() {
    let mut app = crush_app();
    app.session_id = Some(SessionId(1));
    let aviso = LiveEvent::BudgetWarning {
        metric: BudgetMetric::Tokens,
        level_percent: 80,
        used: 400_123.0,
        limit: 500_000.0,
        partial: false,
    };
    app.apply(LiveEvent::ChildEvent {
        parent: SessionId(1),
        child: SessionId(9),
        event: Box::new(aviso.clone()),
    });
    assert_eq!(
        texts(&app),
        vec![
            "aviso de presupuesto: 80 % de 500k tokens de la sesión y sus subagentes (400k usados)"
                .to_string()
        ]
    );
    assert_eq!(
        app.transcript.line(0).unwrap().kind,
        batten::state::LineKind::System
    );
    // Un aviso de otro padre no es de esta sesión y no se pinta.
    app.apply(LiveEvent::ChildEvent {
        parent: SessionId(2),
        child: SessionId(9),
        event: Box::new(aviso),
    });
    assert_eq!(texts(&app).len(), 1);
}

#[test]
fn budget_warning_for_cost_mentions_partial_prices() {
    let mut app = crush_app();
    app.apply(LiveEvent::BudgetWarning {
        metric: BudgetMetric::CostUsd,
        level_percent: 80,
        used: 8.0,
        limit: 10.0,
        partial: true,
    });
    let lineas = texts(&app);
    assert_eq!(lineas.len(), 1);
    assert!(lineas[0].contains("USD"));
    assert!(lineas[0].contains("según los precios conocidos"));
    assert_eq!(
        lineas[0],
        "aviso de presupuesto: 80 % de 10.00 USD de la sesión y sus subagentes (8.00 usados) (según los precios conocidos)"
    );

    let mut completo = crush_app();
    completo.apply(LiveEvent::BudgetWarning {
        metric: BudgetMetric::CostUsd,
        level_percent: 80,
        used: 8.0,
        limit: 10.0,
        partial: false,
    });
    let sin_parcial = texts(&completo);
    assert_eq!(
        sin_parcial[0],
        "aviso de presupuesto: 80 % de 10.00 USD de la sesión y sus subagentes (8.00 usados)"
    );
    assert!(!sin_parcial[0].contains("según los precios conocidos"));
}
