use super::*;

#[test]
fn help_overlay_shows_supported_shortcuts() {
    let mut app = App::new();
    app.help_open = true;
    let content = buffer_of(&mut app, 80, 24);
    assert!(content.contains("Keyboard shortcuts"));
    assert!(content.contains("Ctrl+J"));
    assert!(content.contains("Ctrl+B"));
}

#[test]
fn agents_overlay_shows_the_list_and_the_prompt_of_the_selected_agent() {
    let proyecto = tempfile::tempdir().unwrap();
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
    let registro = loomcoding_core::registry::AgentRegistry::new(vec![
        Box::new(loomcoding_core::registry::BuiltInSource),
        Box::new(loomcoding_core::registry::FileSource::new(
            proyecto.path().to_path_buf(),
            loomcoding_core::registry::ProfileLevel::Project,
        )),
    ]);
    let mut app = App::new();
    app.set_agent_registry(std::sync::Arc::new(registro));
    app.show_agents();
    let content = buffer_of(&mut app, 200, 30);
    assert!(content.contains("Agents"), "{content}");
    assert!(
        content.contains("default: Agente genérico integrado"),
        "{content}"
    );
    assert!(
        content.contains("front: Interfaz [awaiting approval] project:"),
        "{content}"
    );
    // Verificar que aparece la línea de directorio
    assert!(content.contains("(directory:"), "{content}");

    app.agents_move(1);
    let content = buffer_of(&mut app, 200, 30);
    assert!(content.contains("Prompt of `front`"), "{content}");
    assert!(content.contains("Cuerpo de front."), "{content}");
    assert!(content.contains("a approve"), "{content}");
}

#[test]
fn hides_sidebar_when_narrow() {
    let mut app = App::with_context("/proj", "m");
    let content = buffer_of(&mut app, 60, 20);
    assert!(!content.contains("subagentes"));
}

#[test]
fn session_picker_keeps_selected_row_visible_when_list_overflows() {
    let mut app = App::new();
    app.set_sessions(
        (0..8)
            .map(|index| loomcoding_core::session::SessionInfo {
                id: loomcoding_core::session::SessionId(index),
                preview: format!("Session {index}"),
                message_count: index as usize,
            })
            .collect(),
    );
    app.session_selected = 7;
    let content = buffer_of(&mut app, 80, 12);
    assert!(content.contains("Session 7"));
    assert!(content.contains("Ctrl+F filter"));
}

#[test]
fn ctrl_b_sidebar_is_visible_at_medium_width() {
    let mut app = App::with_context("/proj", "m");
    app.sidebar_expanded = true;
    let content = buffer_of(&mut app, 90, 30);
    assert!(content.contains("PROJECT"));
    assert!(content.contains("0 subagents running"));
    assert!(content.contains("Conversation"));
}

#[test]
fn queue_shows_pause_controls_and_edits_with_its_own_cursor() {
    let mut app = App::new();
    app.queue_open = true;
    app.queue_paused = true;
    app.pending_prompts.push_back("original".into());
    let content = buffer_of(&mut app, 100, 30);
    assert!(content.contains("paused"));
    assert!(content.contains("Enter edit"));
    assert!(content.contains("Ctrl+X"));
    app.queue_editing = true;
    app.queue_editor.set_text("edited\nsecond line");
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|frame| render(frame, &mut app)).unwrap();
    assert!(terminal.backend().cursor_visible());
    let content = buffer_of(&mut app, 100, 30);
    assert!(content.contains("second line"));
    assert!(content.contains("Esc discard edit"));
}

#[test]
fn overlays_render_across_terminal_sizes_and_hide_the_composer_cursor() {
    for overlay in 0..6 {
        let mut app = App::new();
        match overlay {
            0 => app.session_loading = true,
            1 => app.session_picker_open = true,
            2 => app.queue_open = true,
            3 => app.help_open = true,
            4 => {
                app.set_provider_settings(crate::app::ProviderSettings {
                    kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
                    model: "test-model".into(),
                    base_url: "https://example.test/v1".into(),
                    api_key_env: "API_KEY".into(),
                    effort: None,
                });
                app.open_settings();
            }
            _ => {
                let (tx, _rx) = tokio::sync::oneshot::channel();
                app.enqueue_permission(
                    loomcoding_core::permission::PermissionRequest {
                        tool: "bash".into(),
                        action: "run".into(),
                        detail: "cargo test".into(),
                    },
                    tx,
                );
            }
        }
        for (width, height) in [(0, 0), (1, 1), (10, 3), (30, 8), (60, 20), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render(frame, &mut app)).unwrap();
            assert!(
                !terminal.backend().cursor_visible(),
                "overlay {overlay} at {width}x{height}"
            );
        }
        if (1..=4).contains(&overlay) {
            assert!(buffer_of(&mut app, 20, 8).contains("enlarge"));
        }
    }
    let mut app = App::new();
    app.help_open = true;
    let help = buffer_of(&mut app, 80, 24);
    assert!(help.contains("Shift+Tab"));
    assert!(help.contains("Press F1 or Esc to close"));
}

#[test]
fn provider_settings_shows_saving_and_recoverable_error() {
    let mut app = App::new();
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::OpenAiCompat,
        model: "test-model".into(),
        base_url: "https://example.test/v1".into(),
        api_key_env: "API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    app.settings_saving = true;
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Applying"));
    assert!(content.contains("Not configured"));
    app.settings_saved(Err("Cannot write config".into()));
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Cannot write config"));
    assert!(content.contains("Ctrl+S"));
    let content = buffer_of(&mut app, 30, 8);
    assert!(content.contains("Provider settings"));
    assert!(content.contains("Esc close"));
}

#[test]
fn shows_permission_dialog_on_top() {
    let mut app = App::new();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    app.enqueue_permission(
        loomcoding_core::permission::PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: r#"{"command":"cargo test"}"#.into(),
        },
        tx,
    );
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Permiso"));
    assert!(content.contains("cargo test"));
    assert!(content.contains("Denegar"));
}

fn ask_question_for_render() -> loomcoding_core::question::Question {
    use loomcoding_core::question::{Question, QuestionOption};
    let option = |label: &str| QuestionOption {
        label: label.into(),
        description: String::new(),
    };
    Question {
        header: "Stack".into(),
        question: "Which framework?".into(),
        options: vec![option("Alpha"), option("Beta")],
        multi_select: false,
    }
}

#[test]
fn question_dialog_renders_and_hides_the_composer_cursor() {
    let mut app = App::new();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    app.enqueue_questions(vec![ask_question_for_render()], tx);
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Question 1/1"));
    assert!(content.contains("Which framework?"));
    assert!(content.contains("Alpha"));
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|frame| render(frame, &mut app)).unwrap();
    assert!(!terminal.backend().cursor_visible());
}

#[test]
fn permission_is_drawn_above_a_question() {
    use loomcoding_core::permission::PermissionRequest;
    let request = PermissionRequest {
        tool: "bash".into(),
        action: "run".into(),
        detail: r#"{"command":"cargo test"}"#.into(),
    };
    let render_with_question = |open_question: bool| {
        let mut app = App::new();
        if open_question {
            let (tx, _rx) = tokio::sync::oneshot::channel();
            app.enqueue_questions(vec![ask_question_for_render()], tx);
        }
        let (ptx, _prx) = tokio::sync::oneshot::channel();
        app.enqueue_permission(request.clone(), ptx);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|frame| render(frame, &mut app)).unwrap();
        terminal.backend().buffer().clone()
    };
    let alone = render_with_question(false);
    let stacked = render_with_question(true);
    let text: String = stacked.content.iter().map(|c| c.symbol()).collect();
    assert!(text.contains("Permiso"));
    assert!(text.contains("cargo test"));
    assert!(
        text.contains("Question 1/1"),
        "la pregunta debe seguir visible fuera del permiso"
    );
    // Las celdas del permiso no pueden mostrar nada de la pregunta: se pintó encima.
    let dialog = crate::permission_flow::dialog_for(&request);
    let rect = loomcoding_cloth::widgets::permission::area(Rect::new(0, 0, 120, 40), &dialog);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            let i = usize::from(y) * 120 + usize::from(x);
            assert_eq!(
                stacked.content[i].symbol(),
                alone.content[i].symbol(),
                "celda ({x},{y}) pintada por debajo del permiso"
            );
        }
    }
}

#[test]
fn picker_renders_items_with_detail() {
    use crate::app::SettingsPicker;

    let mut app = App::new();
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: "model-a".into(),
        base_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(crate::app::picker_sample_catalog());
    app.open_settings();
    app.open_picker(SettingsPicker::Model);
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Claude Sonnet"));
    assert!(content.contains("ctx 200k · $3/$15"));
    assert!(content.contains("ctx 200k · -"));
}

#[test]
fn settings_dialog_shows_effort_only_for_reasoning_models() {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(crate::app::ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: "claude-think".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.open_settings();
    assert!(buffer_of(&mut app, 120, 40).contains("Effort: default"));

    app.cycle_effort(true);
    let content = buffer_of(&mut app, 120, 40);
    assert!(content.contains("Effort: low"));
    assert_eq!(content.matches("● unsaved").count(), 1);

    app.provider_settings.as_mut().unwrap().model = "claude-plain".into();
    assert!(!buffer_of(&mut app, 120, 40).contains("Effort:"));
}

#[test]
fn the_hidden_lines_hint_is_drawn_and_clicking_it_unfolds_the_block() {
    let mut app = App::new();
    app.push_user(
        &(1..=25)
            .map(|i| format!("l{i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    // Fila del aviso: la última línea del bloque.
    let (_, fin) = block_rows(&app, 0);
    let y = area.y + (fin - 1 - app.viewport.top()) as u16;
    let fila: String = (area.x..area.right())
        .map(|x| term.backend().buffer()[(x, y)].symbol().to_string())
        .collect();
    assert!(fila.contains("(15 lines hidden)"), "{fila}");

    app.transcript_mouse_down(area.x + 2, y);

    assert_eq!(app.transcript.entry_lines(0).len(), 25);
}

#[test]
fn a2a_observatory_renders_empty_sections_and_protocol_labels() {
    let mut app = App::new();
    app.a2a_open = true;
    app.a2a_focus = 2;

    let content = buffer_of(&mut app, 120, 40);

    assert!(content.contains("A2A · PROTOCOL OBSERVATORY"));
    assert!(content.contains("AGENT CARDS"));
    assert!(content.contains("TASKS"));
    assert!(content.contains("JSON-RPC / SSE"));
    assert!(content.contains("Sin observaciones de actividad A2A."));
}
