use super::*;

/// App con dos agentes globales usables (`rust` y `superpowers`), sin aprobación pendiente.
fn app_con_agentes() -> App {
    use loomcoding_core::registry::{AgentRegistry, BuiltInSource, FileSource, ProfileLevel};
    let dir = tempfile::tempdir().unwrap();
    for nombre in ["rust", "superpowers"] {
        std::fs::write(
            dir.path().join(format!("{nombre}.md")),
            format!(
                "---
name: {nombre}
description: x
---

Cuerpo.
"
            ),
        )
        .unwrap();
    }
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            dir.path().to_path_buf(),
            ProfileLevel::Global,
        )),
    ]);
    // La instantánea ya está cargada: el directorio temporal puede desaparecer.
    let mut app = App::new();
    app.set_agent_registry(std::sync::Arc::new(registro));
    app
}

#[test]
fn slash_agent_and_at_agent_request_a_run_and_send_no_prompt() {
    for texto in ["/agent rust revisa X", "@rust revisa X"] {
        let mut app = app_con_agentes();
        app.composer.set_text(texto);
        assert_eq!(
            handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
            None,
            "{texto}"
        );
        let pedida = app.run_agent_requested.clone().expect(texto);
        assert_eq!(pedida.agent, "rust");
        assert_eq!(pedida.prompt, "revisa X");
        assert_eq!(app.composer.text(), "");
    }
}

#[test]
fn skills_travel_with_the_run_request() {
    let mut app = app_con_agentes();
    app.composer
        .set_text("@superpowers --skills brainstorming,writing-plans diseña X");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    let pedida = app.run_agent_requested.clone().unwrap();
    assert_eq!(
        pedida.skills,
        Some(vec![
            "brainstorming".to_string(),
            "writing-plans".to_string()
        ])
    );
}

#[test]
fn an_at_sign_that_is_not_an_agent_is_still_a_normal_prompt() {
    let mut app = app_con_agentes();
    app.composer.set_text("@src/lib.rs mira esto");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        Some("@src/lib.rs mira esto".to_string())
    );
    assert!(app.run_agent_requested.is_none());
}

#[test]
fn a_malformed_slash_agent_shows_feedback_and_keeps_the_text() {
    let mut app = app_con_agentes();
    app.composer.set_text("/agent nadie hola");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert!(app.run_agent_requested.is_none());
    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|m| m.contains("unknown agent")));
    assert_eq!(app.composer.text(), "/agent nadie hola");
}

#[test]
fn tab_completes_the_agent_name_without_changing_focus() {
    for (escrito, esperado) in [("/agent ru", "/agent rust "), ("@su", "@superpowers ")] {
        let mut app = app_con_agentes();
        app.composer.set_text(escrito);
        let foco = app.focus;
        handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.composer.text(), esperado, "{escrito}");
        assert_eq!(app.focus, foco, "{escrito}");
    }
}

#[test]
fn slash_agent_is_offered_in_the_command_popup_and_prefills_the_composer() {
    let mut app = app_con_agentes();
    app.composer.set_text("/agent");
    assert!(app
        .slash_matches()
        .iter()
        .any(|(_, nombre, _)| *nombre == "/agent"));
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "/agent");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "/agent ");
    assert!(app.run_agent_requested.is_none());
}

#[test]
fn the_run_request_is_sent_to_the_worker_once() {
    let mut app = app_con_agentes();
    app.composer.set_text("/agent rust revisa X");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    send_session_commands(&mut app, &tx);
    send_session_commands(&mut app, &tx);
    assert!(matches!(rx.try_recv(), Ok(SessionCommand::RunAgent(_))));
    assert!(rx.try_recv().is_err());
}

#[test]
fn capability_widget_scroll_offsets_are_independent() {
    let mut app = App::new();
    assert!(scroll_dashboard_widget(&mut app, Focus::Mcp, true, 2));
    assert!(scroll_dashboard_widget(&mut app, Focus::Skills, true, 3));
    assert_eq!(app.capability_scroll, [2, 0, 3]);
    assert_eq!(app.activity_scroll, 0);
    assert!(!scroll_dashboard_widget(
        &mut app,
        Focus::Transcript,
        true,
        2
    ));
}
