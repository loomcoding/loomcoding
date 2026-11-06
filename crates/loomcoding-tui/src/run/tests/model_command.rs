//! `/model` desde el compositor y su selector rápido: `Enter` aplica, `Esc` cancela sin cambios,
//! un argumento inválido deja el texto escrito, y el resultado del guardado llega al compositor.

use super::*;

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

/// Aplicación con ajustes de Anthropic y el catálogo de muestra.
fn app_con_catalogo() -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: "model-a".into(),
        base_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    });
    app.set_catalog(crate::app::picker_sample_catalog());
    app
}

fn modelo(app: &App) -> String {
    app.provider_settings.as_ref().unwrap().model.clone()
}

#[test]
fn model_with_an_argument_runs_from_the_composer() {
    let mut app = app_con_catalogo();
    assert_eq!(escribir(&mut app, "/model qwen3-local"), None);
    assert_eq!(modelo(&app), "qwen3-local");
    assert!(app.settings_save_requested);
    assert_eq!(app.slash_feedback.as_deref(), Some("model: qwen3-local"));
    assert!(app.composer.is_empty());
}

#[test]
fn a_bad_argument_keeps_the_typed_text_and_changes_nothing() {
    let mut app = app_con_catalogo();
    assert_eq!(escribir(&mut app, "/model a b"), None);
    assert_eq!(app.slash_feedback.as_deref(), Some("Usage: /model <id>"));
    assert_eq!(app.composer.text(), "/model a b");
    assert_eq!(modelo(&app), "model-a");
    assert!(!app.settings_save_requested);
}

#[test]
fn a_turn_in_flight_keeps_the_text_and_says_why() {
    let mut app = app_con_catalogo();
    app.turn_in_flight = true;
    assert_eq!(escribir(&mut app, "/model model-b"), None);
    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|text| text.contains("active turn")));
    assert_eq!(app.composer.text(), "/model model-b");
    assert_eq!(modelo(&app), "model-a");
}

#[test]
fn model_without_argument_opens_the_quick_picker_and_enter_applies() {
    let mut app = app_con_catalogo();
    assert_eq!(escribir(&mut app, "/model"), None);
    assert!(app.quick_model_picker);
    assert!(!app.settings_open);
    assert!(app.composer.is_empty());
    let target = app.picker_items()[1].id.clone();
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!app.quick_model_picker);
    assert!(app.settings_picker.is_none());
    assert!(!app.settings_open);
    assert_eq!(modelo(&app), target);
    assert!(app.settings_save_requested);
}

#[test]
fn escape_closes_the_quick_picker_without_changes() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/model");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.quick_model_picker);
    assert!(app.settings_picker.is_none());
    assert_eq!(modelo(&app), "model-a");
    assert!(!app.settings_save_requested);
    assert!(!app.settings_open);
}

#[test]
fn quick_picker_accepts_a_typed_id_that_matches_nothing() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/model");
    for c in "Ornith-1.5-9B".chars() {
        handle_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(!app.quick_model_picker);
    assert_eq!(modelo(&app), "Ornith-1.5-9B");
    assert!(app.settings_save_requested);
}

#[test]
fn the_save_outcome_reaches_the_composer_feedback() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/model model-b");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut pending = None;
    send_provider_settings(&mut app, &tx, &mut pending);
    let command = rx.try_recv().unwrap();
    assert_eq!(command.settings.model, "model-b");
    command
        .reply
        .send(Err("Cannot write config".into()))
        .unwrap();
    app.settings_saved(pending.take().unwrap().try_recv().unwrap());
    assert_eq!(app.slash_feedback.as_deref(), Some("Cannot write config"));
}

#[test]
fn a_paste_while_the_quick_picker_is_open_does_not_reach_the_composer() {
    let mut app = app_con_catalogo();
    escribir(&mut app, "/model");
    assert!(app.quick_model_picker, "el selector rápido debía abrirse");
    handle_event(&mut app, Event::Paste("texto pegado".into()));
    assert!(
        app.composer.is_empty(),
        "el pegado no debe colarse en el compositor: {:?}",
        app.composer.text()
    );
}
