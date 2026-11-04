use super::*;

/// Última fila de la pantalla (el pie) tras pintar la app entera.
fn last_row_of(app: &mut App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    let buffer = term.backend().buffer();
    let start = usize::from(h - 1) * usize::from(w);
    buffer.content[start..start + usize::from(w)]
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn development_workflow_status_is_visible_in_the_chat_viewport() {
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(7));
    app.apply(
        loomcoding_core::event::LiveEvent::DevelopmentWorkflowChanged(
            loomcoding_core::workflow::WorkflowSummary {
                id: loomcoding_core::workflow::WorkflowId(3),
                parent: loomcoding_core::session::SessionId(7),
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
        ),
    );

    let screen = buffer_of(&mut app, 120, 30);
    assert!(
        screen.contains("flujo #3 · aprobar spec+plan · coordinador · ronda 1"),
        "{screen}"
    );
}

#[test]
fn a_gauntlet_run_reads_as_a_story_in_the_chat_viewport() {
    use loomcoding_core::gauntlet::{GauntletId, GauntletPhase as P, GauntletSummary};
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(7));
    let paso = |app: &mut App, phase: P, round: u8, actor: &str, detail: &str| {
        app.apply(loomcoding_core::event::LiveEvent::GauntletChanged {
            summary: GauntletSummary {
                id: GauntletId(3),
                parent: loomcoding_core::session::SessionId(7),
                phase,
                round,
                max_rounds: 2,
                builder: "constructor".into(),
                critic: "critico".into(),
                active_child: None,
                updated_at_ms: 0,
            },
            actor: actor.into(),
            detail: detail.into(),
        });
    };
    paso(&mut app, P::Building, 1, "parent", "gauntlet started");
    paso(&mut app, P::Judging, 1, "critic", "critic launched");
    paso(
        &mut app,
        P::Rebuilding,
        1,
        "critic",
        "gap: los márgenes están descuadrados",
    );
    paso(
        &mut app,
        P::AwaitingFinalAcceptance,
        2,
        "critic",
        "no gap, confirmed with the order swapped",
    );
    paso(&mut app, P::Completed, 2, "user", "final acceptance");
    let screen = buffer_of(&mut app, 120, 30);
    // El transcript es estrecho y parte las líneas: se comprueban los trozos, no la línea entera.
    for esperado in [
        "gauntlet #3 · construyendo",
        "parent: gauntlet started",
        "gauntlet #3 · juzgando",
        "gauntlet #3 · rehaciendo",
        "gap: los márgenes están descuadrados",
        "gauntlet #3 · aceptar resultado",
        "confirmed with the order swapped",
        "gauntlet #3 · completado",
        "user: final acceptance",
    ] {
        assert!(
            screen.contains(esperado),
            "{esperado}
{screen}"
        );
    }
}

/// App con el tema indicado, un modelo de ventana conocida y una última petición de 142 000
/// tokens. `model` elige entre `windowed` (200 000) y `bare` (sin ventana).
fn footer_app(preset: loomcoding_cloth::widgets::ThemePreset, model: &str) -> App {
    let mut app = App::new();
    app.set_theme(preset);
    app.set_catalog(crate::app::windowed_sample_catalog());
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: model.into(),
        base_url: "https://acme.example/v1".into(),
        api_key_env: "ACME_KEY".into(),
        effort: None,
    });
    app.apply(loomcoding_core::event::LiveEvent::Usage {
        input_tokens: 142_000,
        output_tokens: 1,
    });
    app
}

#[test]
fn provider_activity_turns_the_composer_prompt_into_a_spinner() {
    use loomcoding_cloth::widgets::ThemePreset;
    use loomcoding_core::event::LiveEvent;
    for preset in [ThemePreset::Cloth, ThemePreset::Crush] {
        let mut app = footer_app(preset, "windowed");
        assert!(!app.composer.is_working(), "{preset:?}");

        app.apply(LiveEvent::RuntimeNotice {
            message: "provider request started for `windowed`".into(),
        });
        assert!(app.composer.is_working(), "{preset:?}");
        // El estilo se elige por petición y sale del catálogo de dotmax.
        assert!(
            loomcoding_cloth::widgets::activity::activity_style_names()
                .contains(&app.composer.activity_style()),
            "{preset:?}: {}",
            app.composer.activity_style()
        );

        app.apply(LiveEvent::RuntimeNotice {
            message: "provider response received for `windowed`".into(),
        });
        assert!(!app.composer.is_working(), "{preset:?}");
    }
}

#[test]
fn verbose_notices_keep_the_composer_prompt_static() {
    use loomcoding_cloth::widgets::ThemePreset;
    use loomcoding_core::event::LiveEvent;
    let mut app = footer_app(ThemePreset::Crush, "windowed");
    app.set_provider_notices_verbose(true);
    app.apply(LiveEvent::RuntimeNotice {
        message: "provider request started for `windowed`".into(),
    });
    assert!(!app.composer.is_working());
}

#[test]
fn footer_shows_the_meter_when_wide_enough() {
    use loomcoding_cloth::widgets::ThemePreset;
    // Pie clásico (Cloth) y pie plano (Crush): los dos reservan el medidor.
    for preset in [ThemePreset::Cloth, ThemePreset::Crush] {
        let mut app = footer_app(preset, "windowed");
        let row = last_row_of(&mut app, 80, 24);
        assert!(row.contains("━━━━━━━━━━"), "medidor en {preset:?}: {row}");
        assert!(row.contains("142k/200k"), "fracción en {preset:?}: {row}");
        assert!(row.contains("71 %"), "porcentaje en {preset:?}: {row}");
    }
}

#[test]
fn footer_hides_the_meter_when_narrow() {
    use loomcoding_cloth::widgets::ThemePreset;
    for preset in [ThemePreset::Cloth, ThemePreset::Crush] {
        let mut app = footer_app(preset, "windowed");
        let row = last_row_of(&mut app, 79, 24);
        assert!(!row.contains("━"), "sin medidor en {preset:?}: {row}");
        assert!(
            !row.contains("142k/200k"),
            "sin fracción en {preset:?}: {row}"
        );
    }
}

#[test]
fn footer_shows_the_count_when_window_unknown() {
    use loomcoding_cloth::widgets::ThemePreset;
    let mut app = footer_app(ThemePreset::Cloth, "bare");
    let row = last_row_of(&mut app, 80, 24);
    assert!(row.contains("ctx 142k (sin ventana)"), "{row}");
    assert!(!row.contains("━"), "sin barra sin ventana: {row}");
}

#[test]
fn background_events_drive_the_status_and_the_arrow() {
    use loomcoding_cloth::widgets::ThemePreset;
    use loomcoding_core::event::LiveEvent;
    let mut app = footer_app(ThemePreset::Cloth, "windowed");
    // Sin resumen anticipado no hay texto de estado ni flecha en el pie. La flecha de
    // «ahorro pendiente» se mira junto al porcentaje, porque el pie ya lleva otras «↓».
    let row = last_row_of(&mut app, 80, 24);
    assert!(!row.contains("resumiendo"), "{row}");
    assert!(!row.contains("71 % ↓"), "{row}");

    // Arranca el resumen: la línea de estado cuenta «resumiendo…» mientras corre.
    app.apply(LiveEvent::BackgroundCompactionStarted {
        tokens_before: 120_000,
    });
    let row = last_row_of(&mut app, 80, 24);
    assert!(row.contains("resumiendo…"), "{row}");
    assert!(!row.contains("71 % ↓"), "{row}");

    // El resumen está listo: desaparece el texto de estado y el medidor lleva la flecha.
    app.apply(LiveEvent::BackgroundCompactionReady { summary_bytes: 400 });
    let row = last_row_of(&mut app, 80, 24);
    assert!(row.contains("71 % ↓"), "{row}");
    assert!(!row.contains("resumiendo"), "{row}");
}

#[test]
fn compacted_clears_the_ready_mark() {
    use loomcoding_cloth::widgets::ThemePreset;
    use loomcoding_core::event::{Activation, LiveEvent};
    let mut app = footer_app(ThemePreset::Cloth, "windowed");
    app.apply(LiveEvent::BackgroundCompactionStarted {
        tokens_before: 120_000,
    });
    app.apply(LiveEvent::BackgroundCompactionReady { summary_bytes: 400 });
    assert!(app.context_meter().ready);

    // Al activarse la compactación, la flecha desaparece y el resumen pasa a ser memoria.
    app.apply(LiveEvent::ContextCompacted {
        replaced: 4,
        tokens_before: 160_000,
        summary_bytes: 100,
        activation: Activation::Sync,
    });
    assert!(!app.context_meter().ready);
    let row = last_row_of(&mut app, 80, 24);
    assert!(row.contains("resumen 25"), "{row}");
    assert!(!row.contains("resumen 25 ↓"), "{row}");
    assert!(!row.contains("resumiendo"), "{row}");
}

#[test]
fn a_discarded_background_summary_clears_the_status_and_the_arrow() {
    use loomcoding_cloth::widgets::ThemePreset;
    use loomcoding_core::event::LiveEvent;
    let mut app = footer_app(ThemePreset::Cloth, "windowed");
    app.apply(LiveEvent::BackgroundCompactionStarted {
        tokens_before: 120_000,
    });
    assert!(last_row_of(&mut app, 80, 24).contains("resumiendo"));

    app.apply(LiveEvent::BackgroundCompactionDiscarded);
    let row = last_row_of(&mut app, 80, 24);
    assert!(!row.contains("resumiendo"), "{row}");

    // También cuando ya estaba listo y se tira por caduco.
    app.apply(LiveEvent::BackgroundCompactionReady { summary_bytes: 400 });
    assert!(app.context_meter().ready);
    app.apply(LiveEvent::BackgroundCompactionDiscarded);
    assert!(!app.context_meter().ready);
}

#[test]
fn renders_items_and_input() {
    let mut app = App::new();
    app.push_user("hello");
    app.push_char('x');
    let content = buffer_of(&mut app, 40, 10);
    assert!(content.contains("hello"));
    assert!(content.contains('x'));
    assert!(content.contains("CodeL∞M"));
    assert!(content.contains("CODEL∞M / WORKSPACE"));
}

#[test]
fn assistant_markdown_is_rendered() {
    let mut app = App::new();
    app.push_assistant("## Hecho\n\n- **uno**\n- `dos`");
    let content = buffer_of(&mut app, 60, 20);
    assert!(content.contains("Hecho"));
    assert!(!content.contains("##"));
    assert!(content.contains("• uno"));
    assert!(!content.contains("**"));
}

#[test]
fn header_shows_queued_prompts() {
    let mut app = App::new();
    app.queued = 2;
    assert!(buffer_of(&mut app, 60, 10).contains("2 queued"));
}

#[test]
fn header_job_counts_are_scoped_to_the_open_session() {
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(2));
    for (id, owner, status) in [
        ("shell-1", 2, loomcoding_core::job::JobStatus::Running),
        ("shell-2", 2, loomcoding_core::job::JobStatus::Failed),
        ("shell-3", 1, loomcoding_core::job::JobStatus::Running),
        ("subagent-1", 2, loomcoding_core::job::JobStatus::Running),
    ] {
        let id = loomcoding_core::job::JobId(id.into());
        let kind = if id.0.starts_with("subagent-") {
            "subagent"
        } else {
            "shell"
        };
        app.jobs.insert(
            id.clone(),
            loomcoding_core::job::JobSnapshot {
                id,
                kind: kind.into(),
                label: "test".into(),
                owner: loomcoding_core::session::SessionId(owner),
                status,
                progress: None,
                output: String::new(),
                output_start: 0,
                output_end: 0,
                detail: None,
                result: None,
            },
        );
    }
    for (child, status) in [
        (99, loomcoding_core::task::TaskState::Working),
        (100, loomcoding_core::task::TaskState::Failed),
    ] {
        app.subagents.insert(
            child,
            loomcoding_core::session::SubagentDescriptor {
                phase: SubagentPhase::Queued,
                last_activity_at_ms: 0,
                steps: 0,
                tool_calls: 0,
                tokens_in: 0,
                tokens_out: 0,
                parent: loomcoding_core::session::SessionId(2),
                child: loomcoding_core::session::SessionId(child),
                job_id: format!("subagent-{child}"),
                label: "test".into(),
                status: loomcoding_core::task::TaskStatus::new(status),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            },
        );
    }
    assert_eq!(job_counts(&app), (1, 1, 1, 1));
}

#[test]
fn header_names_the_a2a_states_of_its_subagent_counts() {
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(1));
    for (child, state) in [(9, TaskState::Working), (10, TaskState::Completed)] {
        app.subagents.insert(
            child,
            loomcoding_core::session::SubagentDescriptor {
                phase: SubagentPhase::Queued,
                last_activity_at_ms: 0,
                steps: 0,
                tool_calls: 0,
                tokens_in: 0,
                tokens_out: 0,
                parent: loomcoding_core::session::SessionId(1),
                child: loomcoding_core::session::SessionId(child),
                job_id: format!("subagent-{child}"),
                label: "review".into(),
                status: TaskStatus::new(state),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            },
        );
    }
    let texto_de = |dibujar: fn(&mut Frame, Rect, &App)| {
        let mut term = Terminal::new(TestBackend::new(260, 3)).unwrap();
        term.draw(|f| dibujar(f, f.area(), &app)).unwrap();
        term.backend()
            .buffer()
            .content
            .iter()
            .map(|celda| celda.symbol())
            .collect::<String>()
    };
    let cabecera = texto_de(draw_workspace_header);
    assert!(
        cabecera.contains("1 subagents running (working 1)"),
        "falta el recuento en curso: {cabecera}"
    );
    // Solo la cabecera de espacio de trabajo cuenta los terminados.
    assert!(
        cabecera.contains(" (subagents: completed 1)"),
        "falta el recuento de terminados: {cabecera}"
    );
    let plana = texto_de(draw_flat_header);
    assert!(
        plana.contains("1 subagents running (working 1)"),
        "falta el recuento en curso: {plana}"
    );
}

#[test]
fn header_shows_state_counts_without_truncation_at_100_columns() {
    let mut app = App::new();
    app.session_id = Some(loomcoding_core::session::SessionId(1));
    for (child, state) in [(9, TaskState::Working), (10, TaskState::InputRequired)] {
        app.subagents.insert(
            child,
            loomcoding_core::session::SubagentDescriptor {
                phase: SubagentPhase::Queued,
                last_activity_at_ms: 0,
                steps: 0,
                tool_calls: 0,
                tokens_in: 0,
                tokens_out: 0,
                parent: loomcoding_core::session::SessionId(1),
                child: loomcoding_core::session::SessionId(child),
                job_id: format!("subagent-{child}"),
                label: "review".into(),
                status: TaskStatus::new(state),
                artifacts: Vec::new(),
                notice_delivered: false,
                agent: None,
                agent_origin: None,
                reference_task_ids: Vec::new(),
            },
        );
    }
    let mut term = Terminal::new(TestBackend::new(100, 3)).unwrap();
    term.draw(|f| draw_workspace_header(f, f.area(), &app))
        .unwrap();
    let buffer = term.backend().buffer().clone();
    let fila_resumen: String = buffer.content[100..200]
        .iter()
        .map(|celda| celda.symbol())
        .collect();
    assert!(
        fila_resumen.contains("2 subagents running (working 1, input-required 1)"),
        "el resumen se recorta a 100 columnas: {fila_resumen:?}"
    );
}

#[test]
fn header_has_no_parentheses_without_subagents() {
    let mut app = App::new();
    let mut term = Terminal::new(TestBackend::new(100, 3)).unwrap();
    term.draw(|f| draw_workspace_header(f, f.area(), &app))
        .unwrap();
    let sin_subagentes: String = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|celda| celda.symbol())
        .collect();
    assert!(
        !sin_subagentes.contains('('),
        "hay paréntesis sin subagentes: {sin_subagentes}"
    );

    // Solo terminados: el resumen en curso es `0 subagents running`, sin paréntesis.
    app.session_id = Some(loomcoding_core::session::SessionId(1));
    app.subagents.insert(
        9,
        loomcoding_core::session::SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: loomcoding_core::session::SessionId(1),
            child: loomcoding_core::session::SessionId(9),
            job_id: "subagent-9".into(),
            label: "review".into(),
            status: TaskStatus::new(TaskState::Failed),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        },
    );
    let mut term = Terminal::new(TestBackend::new(100, 3)).unwrap();
    term.draw(|f| draw_workspace_header(f, f.area(), &app))
        .unwrap();
    let texto: String = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|celda| celda.symbol())
        .collect();
    // El paréntesis que sigue es el de los terminados; el cero en curso no lleva ninguno.
    assert!(
        texto.contains("0 subagents running (subagents: failed 1)"),
        "falta el resumen: {texto}"
    );
    assert!(
        !texto.contains("0 subagents running (working")
            && !texto.contains("0 subagents running (input"),
        "el cero no debe nombrar estados: {texto}"
    );
}

#[test]
fn workspace_header_marks_errors_instead_of_ready() {
    let mut app = App::new();
    app.status = "error: provider unavailable".into();
    assert!(buffer_of(&mut app, 80, 20).contains("ERROR"));
}

#[test]
fn prompt_queue_previews_the_selected_full_prompt() {
    let mut app = App::new();
    app.pending_prompts
        .push_back("Review the changed files\nThen summarize any risks".into());
    app.queued = 1;
    app.queue_open = true;
    let content = buffer_of(&mut app, 80, 20);
    assert!(content.contains("Preview"));
    assert!(content.contains("Then summarize any risks"));
}
