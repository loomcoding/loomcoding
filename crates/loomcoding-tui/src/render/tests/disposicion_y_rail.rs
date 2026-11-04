use super::*;

#[test]
fn shows_sidebar_when_wide() {
    let mut app = App::with_context("/proj", "mistral-nemo");
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("mistral-nemo"));
    assert!(content.contains("0 subagents running"));
    // Sin subagentes en curso el resumen es `0 subagents running`, sin paréntesis que ocultar.
    assert!(!content.contains("subagents running ("), "{content}");
    assert!(content.contains("Loomcoding"));
}

#[test]
fn shows_modified_files_when_wide() {
    let mut app = App::with_context("/proj", "m");
    app.apply(loomcoding_core::event::LiveEvent::FileDiff {
        path: "src/main.rs".into(),
        added: 3,
        removed: 1,
    });
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("ficheros modificados"));
    assert!(content.contains("src/main.rs"));
}

#[test]
fn session_picker_renders_only_matching_previews() {
    let mut app = App::new();
    app.set_sessions(vec![
        loomcoding_core::session::SessionInfo {
            id: loomcoding_core::session::SessionId(1),
            preview: "Build frontend".into(),
            message_count: 2,
        },
        loomcoding_core::session::SessionInfo {
            id: loomcoding_core::session::SessionId(2),
            preview: "Fix provider config".into(),
            message_count: 4,
        },
    ]);
    app.session_id = Some(loomcoding_core::session::SessionId(2));
    app.session_searching = true;
    app.session_query = "provider".into();
    let content = buffer_of(&mut app, 80, 24);
    assert!(content.contains("Fix provider config"));
    assert!(content.contains("active"));
    assert!(content.contains("1 matches"));
    assert!(!content.contains("Build frontend"));
}

#[test]
fn expanded_tool_output_uses_available_transcript_height() {
    let mut tool = loomcoding_cloth::state::ToolItem::new("call-1", "view");
    tool.set_result(false);
    tool.set_output(
        (0..20)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    tool.expanded = true;
    let mut app = App::new();
    app.turn_in_flight = true;
    app.tools.push_back(tool);
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("line-19"));
}

fn chatty_app() -> App {
    let mut app = App::new();
    for i in 0..60 {
        app.push_user(&format!("msg-{i:02}"));
    }
    app
}

#[test]
fn newest_messages_are_visible_when_overflowing() {
    let mut app = chatty_app();
    let content = buffer_of(&mut app, 60, 20);
    assert!(content.contains("msg-59"));
    assert!(!content.contains("msg-00"));
}

#[test]
fn page_up_scrolls_back_and_stops_following() {
    let mut app = chatty_app();
    buffer_of(&mut app, 60, 20);
    crate::run::handle_key(
        &mut app,
        crossterm::event::KeyCode::PageUp,
        crossterm::event::KeyModifiers::NONE,
    );
    let content = buffer_of(&mut app, 60, 20);
    assert!(!app.viewport.is_following());
    assert!(!content.contains("msg-59"));
}

/// Job de prueba con el dueño y el tipo indicados.
fn job_snapshot(
    id: &str,
    label: &str,
    kind: &str,
    status: loomcoding_core::job::JobStatus,
    owner: loomcoding_core::session::SessionId,
) -> loomcoding_core::job::JobSnapshot {
    loomcoding_core::job::JobSnapshot {
        id: loomcoding_core::job::JobId(id.into()),
        kind: kind.into(),
        label: label.into(),
        owner,
        status,
        progress: None,
        output: String::new(),
        output_start: 0,
        output_end: 0,
        detail: None,
        result: None,
    }
}

/// Aplicación Crush con ficheros, herramienta, job y subagente de la sesión 1.
fn loaded_crush_app() -> App {
    use loomcoding_core::event::LiveEvent;
    use loomcoding_core::job::JobStatus;
    use loomcoding_core::session::{SessionId, SubagentDescriptor};
    let mut app = crush_ctx();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::FileDiff {
        path: "src/main.rs".into(),
        added: 3,
        removed: 1,
    });
    app.apply(LiveEvent::ToolStarted {
        call_id: "c1".into(),
        name: "View".into(),
    });
    app.apply(LiveEvent::JobChanged(job_snapshot(
        "build-1",
        "build",
        "shell",
        JobStatus::Running,
        SessionId(1),
    )));
    app.apply(LiveEvent::SubagentChanged(SubagentDescriptor {
        phase: SubagentPhase::Queued,
        last_activity_at_ms: 0,
        steps: 0,
        tool_calls: 0,
        tokens_in: 0,
        tokens_out: 0,
        parent: SessionId(1),
        child: SessionId(9),
        job_id: "subagent-1".into(),
        label: "review".into(),
        status: TaskStatus::new(TaskState::Working),
        artifacts: Vec::new(),
        notice_delivered: false,
        agent: None,
        agent_origin: None,
        reference_task_ids: Vec::new(),
    }));
    app
}

#[test]
fn token_usage_panel_shows_breakdown_and_cache_only_with_data() {
    let mut app = App::new();
    let sin_datos = buffer_of(&mut app, 140, 50);
    assert!(sin_datos.contains("Token usage"), "layout sin panel de uso");
    assert!(!sin_datos.contains("Context ~"));
    assert!(!sin_datos.contains("Cache read"));

    app.apply(loomcoding_core::event::LiveEvent::ContextBreakdown {
        request_id: 1,
        breakdown: loomcoding_core::agent::compact::ContextBreakdown {
            system: 100,
            tools: 40,
            messages: 20,
            framing: 3,
        },
    });
    app.usage_records = vec![record_with_cache(900, 40)];
    let con_datos = buffer_of(&mut app, 140, 50);
    assert!(con_datos.contains("Context ~163"));
    assert!(con_datos.contains("Cache read 900"));
}

#[test]
fn crush_without_width_for_the_rail_uses_the_whole_screen() {
    let mut app = crush_ctx();
    let content = buffer_of(&mut app, 70, 24);
    assert!(!content.contains("MODEL"));
    assert!(content.contains("CODEL∞M"));
}

#[test]
fn toggle_sidebar_follows_the_look() {
    let mut app = crush_ctx();
    assert!(buffer_of(&mut app, 120, 30).contains("MODEL"));
    app.toggle_sidebar();
    assert!(app.rail_hidden && !app.sidebar_expanded);
    assert!(!buffer_of(&mut app, 120, 30).contains("MODEL"));
    app.toggle_sidebar();
    assert!(buffer_of(&mut app, 120, 30).contains("MODEL"));
    let mut classic = App::with_context("/proj", "m");
    classic.toggle_sidebar();
    assert!(classic.sidebar_expanded && !classic.rail_hidden);
}

#[test]
fn crush_header_is_one_flat_line() {
    let mut app = crush_ctx();
    let content = buffer_of(&mut app, 120, 30);
    // `buffer_of` no inserta saltos de línea: la primera fila son las primeras 120 celdas.
    let first: String = content.chars().take(120).collect();
    assert!(first.contains("CODEL∞M"));
    assert!(first.contains("/proj"));
    assert!(first.contains("ready"));
    assert!(!content.contains("WORKSPACE"));
}

#[test]
fn crush_footer_shows_short_key_hints() {
    let mut app = crush_ctx();
    let content = buffer_of(&mut app, 120, 30);
    assert!(content.contains("enter send"));
    assert!(content.contains("ctrl+j newline"));
}

#[test]
fn rail_lists_files_tools_jobs_and_subagents() {
    let mut app = loaded_crush_app();
    let content = buffer_of(&mut app, 120, 40);
    for expected in [
        "FILES",
        "src/main.rs",
        "TOOLS",
        "View",
        "JOBS",
        "build",
        "SUBAGENTS",
        "review",
    ] {
        assert!(content.contains(expected), "falta {expected:?} en el rail");
    }
}

#[test]
fn rail_caps_long_lists() {
    let mut app = crush_ctx();
    for i in 0..40 {
        app.apply(loomcoding_core::event::LiveEvent::FileDiff {
            path: format!("src/file{i:02}.rs"),
            added: 1,
            removed: 0,
        });
    }
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("… +32 more"));
    assert!(!content.contains("file20.rs"));
}

#[test]
fn rail_does_not_panic_on_tiny_heights() {
    let mut app = loaded_crush_app();
    for (width, height) in [(120, 1), (120, 2), (120, 3), (80, 4), (200, 5)] {
        let _ = buffer_of(&mut app, width, height);
    }
}

/// Tarea de prueba para el rail.
fn todo_item(
    content: &str,
    status: loomcoding_core::todo::TodoStatus,
) -> loomcoding_core::todo::TodoItem {
    loomcoding_core::todo::TodoItem {
        content: content.into(),
        status,
    }
}

#[test]
fn rail_lists_todos_with_progress() {
    use loomcoding_core::event::LiveEvent;
    use loomcoding_core::session::SessionId;
    use loomcoding_core::todo::TodoStatus::{Completed, Pending};
    let mut app = crush_ctx();
    app.session_id = Some(SessionId(1));
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items: vec![
            todo_item("Escribir el parser", Completed),
            todo_item("Pintar el rail", Pending),
        ],
    });
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("TODOS"));
    assert!(content.contains("1/2"));
    assert!(content.contains("Pintar el rail"));
}

#[test]
fn rail_caps_long_todo_lists() {
    use loomcoding_core::event::LiveEvent;
    use loomcoding_core::session::SessionId;
    use loomcoding_core::todo::TodoStatus::Pending;
    let mut app = crush_ctx();
    app.session_id = Some(SessionId(1));
    let items = (1..=20)
        .map(|n| todo_item(&format!("tarea {n:02}"), Pending))
        .collect();
    app.apply(LiveEvent::TodosChanged {
        session: SessionId(1),
        items,
    });
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("… +12 more"));
    assert!(!content.contains("tarea 20"));
}

/// Rectángulo registrado en el último fotograma para una ventana.
fn rect_of(app: &App, pane: Focus) -> Rect {
    app.pane_rects
        .iter()
        .find(|(p, _)| *p == pane)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("falta la ventana {pane:?}"))
}

#[test]
fn render_records_pane_rects_classic_wide() {
    let mut app = App::new();
    let _ = terminal_of(&mut app, 120, 40);
    let panes: Vec<Focus> = app.pane_rects.iter().map(|(p, _)| *p).collect();
    for pane in [
        Focus::Sidebar,
        Focus::Transcript,
        Focus::Agent,
        Focus::Activity,
        Focus::Mcp,
        Focus::Lsp,
        Focus::Skills,
        Focus::Composer,
    ] {
        assert!(panes.contains(&pane), "falta {pane:?}");
    }
    assert!(!panes.contains(&Focus::Rail));
    assert_eq!(app.composer_area, Some(rect_of(&app, Focus::Composer)));
}

#[test]
fn render_records_pane_rects_crush_with_and_without_rail() {
    let mut app = crush_ctx();
    let _ = terminal_of(&mut app, 120, 40);
    let panes: Vec<Focus> = app.pane_rects.iter().map(|(p, _)| *p).collect();
    assert!(panes.contains(&Focus::Rail));
    assert!(panes.contains(&Focus::Transcript));
    assert!(panes.contains(&Focus::Composer));
    assert!(!panes.contains(&Focus::Sidebar));
    assert!(!panes.contains(&Focus::Agent) && !panes.contains(&Focus::Activity));
    // Sin ancho para el rail, el mapa se reescribe y ya no lo lista.
    let _ = terminal_of(&mut app, 70, 24);
    let panes: Vec<Focus> = app.pane_rects.iter().map(|(p, _)| *p).collect();
    assert!(!panes.contains(&Focus::Rail));
    assert!(panes.contains(&Focus::Transcript) && panes.contains(&Focus::Composer));
}

#[test]
fn focused_panel_border_uses_the_accent_style() {
    let mut app = App::new();
    let _ = terminal_of(&mut app, 120, 40);
    let agent = rect_of(&app, Focus::Agent);
    let edge = (agent.x, agent.y + 1);
    let accent = app.theme.styles.accent.fg.expect("el acento tiene color");

    let unfocused = terminal_of(&mut app, 120, 40);
    assert_ne!(unfocused.backend().buffer()[edge].fg, accent);

    app.focus = Focus::Agent;
    let focused = terminal_of(&mut app, 120, 40);
    assert_eq!(focused.backend().buffer()[edge].fg, accent);
}

#[test]
fn focused_rail_marks_its_edge_in_crush() {
    let mut app = crush_ctx();
    let _ = terminal_of(&mut app, 120, 40);
    let rail = rect_of(&app, Focus::Rail);
    let accent = app.theme.styles.accent.fg.expect("el acento tiene color");
    app.focus = Focus::Rail;
    let term = terminal_of(&mut app, 120, 40);
    assert_eq!(term.backend().buffer()[(rail.x, rail.y + 1)].fg, accent);
}

#[test]
fn selected_transcript_cells_carry_the_selection_style() {
    use loomcoding_cloth::state::{CellPos, TextSelection};
    let mut app = App::new();
    for i in 0..40 {
        app.push_user(&format!("linea {i:02}"));
    }
    let _ = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let last = app.layout.total_rows() - 1;
    let mut sel = TextSelection::new(CellPos { row: 0, col: 0 });
    sel.set_head(CellPos { row: last, col: 0 });
    app.text_sel = Some(sel);
    app.text_sel_width = app.layout.width();
    let term = terminal_of(&mut app, 100, 30);
    let sel_bg = app
        .theme
        .styles
        .selection
        .bg
        .expect("la selección tiene fondo");
    let cell = &term.backend().buffer()[(area.x, area.y)];
    assert_eq!(cell.bg, sel_bg);
}

#[test]
fn transcript_focus_no_longer_paints_the_full_margin_filete() {
    let mut app = crush_ctx();
    app.push_user("hola");
    let _ = terminal_of(&mut app, 120, 40);
    app.focus = Focus::Transcript;
    let term = terminal_of(&mut app, 120, 40);
    let area = app.transcript_area.expect("el transcript fija su área");
    let margin = area.x - 1;
    // La fila 0 es el bloque: su guía está en el carril de árbol, no en el hueco del margen.
    assert_eq!(term.backend().buffer()[(margin, area.y)].symbol(), " ");
    // Una fila por debajo del contenido no pertenece a ningún bloque: sin filete.
    let below = area.y + area.height - 1;
    assert_eq!(term.backend().buffer()[(margin, below)].symbol(), " ");
    for y in area.y..area.bottom() {
        assert_ne!(term.backend().buffer()[(margin, y)].symbol(), "▎");
    }
}

#[test]
fn marker_not_drawn_when_transcript_unfocused() {
    let mut app = crush_ctx();
    for i in 0..3 {
        app.push_user(&format!("bloque {i}"));
    }
    let _ = terminal_of(&mut app, 120, 40);
    app.focus = Focus::Composer;
    app.block = Some(1);
    let term = terminal_of(&mut app, 120, 40);
    let area = app.transcript_area.expect("el transcript fija su área");
    let margin = area.x - 1;
    let (start, end) = block_rows(&app, 1);
    for row in start..end {
        let y = area.y + (row - app.viewport.top()) as u16;
        assert_ne!(term.backend().buffer()[(margin, y)].symbol(), "█");
    }
}

#[test]
fn settings_dialog_marks_changed_fields_unsaved() {
    let mut app = App::new();
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "test-model".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    assert!(!buffer_of(&mut app, 120, 40).contains("unsaved"));
    app.provider_settings.as_mut().unwrap().model = "other-model".into();
    let content = buffer_of(&mut app, 120, 40);
    assert_eq!(content.matches("● unsaved").count(), 1);
    assert!(content.contains("other-model"));
    assert!(content.contains("Save and apply (1 change)"));
}

#[test]
fn el_destinatario_hijo_se_ve_sobre_el_composer_en_el_look_crush() {
    let mut app = loaded_crush_app();
    let sin_destino = buffer_of(&mut app, 140, 50);
    assert!(!sin_destino.contains("→ review"));
    app.cycle_composer_target();
    let con_destino = buffer_of(&mut app, 140, 50);
    assert!(
        con_destino.contains("→ review #9"),
        "el composer Crush debía nombrar a quién va lo que se escribe"
    );
}
