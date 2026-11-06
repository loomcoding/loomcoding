//! `/effort` desde el compositor y su selector: `Enter` aplica, `Esc` cancela sin cambios, un
//! argumento inválido deja el texto escrito y un pegado con el selector abierto no llega al
//! compositor.

use super::*;

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

/// Aplicación con `claude-think` (low, medium, high) y el catálogo de razonamiento.
fn app_con_niveles(effort: Option<&str>) -> App {
    let mut app = App::new();
    app.set_provider_settings(ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: "claude-think".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: effort.map(str::to_string),
    });
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app
}

fn esfuerzo(app: &App) -> Option<String> {
    app.provider_settings.as_ref().unwrap().effort.clone()
}

#[test]
fn effort_with_an_argument_runs_from_the_composer() {
    let mut app = app_con_niveles(None);
    assert_eq!(escribir(&mut app, "/effort high"), None);
    assert_eq!(esfuerzo(&app).as_deref(), Some("high"));
    assert!(app.settings_save_requested);
    assert_eq!(app.slash_feedback.as_deref(), Some("effort: high"));
    assert!(app.composer.is_empty());
}

#[test]
fn a_bad_argument_keeps_the_typed_text_and_changes_nothing() {
    let mut app = app_con_niveles(None);
    assert_eq!(escribir(&mut app, "/effort ultra"), None);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Usage: /effort [default|low|medium|high]")
    );
    assert_eq!(app.composer.text(), "/effort ultra");
    assert_eq!(esfuerzo(&app), None);
    assert!(!app.settings_save_requested);
}

#[test]
fn a_turn_in_flight_keeps_the_text_and_says_why() {
    let mut app = app_con_niveles(None);
    app.turn_in_flight = true;
    assert_eq!(escribir(&mut app, "/effort high"), None);
    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|text| text.contains("active turn")));
    assert_eq!(app.composer.text(), "/effort high");
    assert_eq!(esfuerzo(&app), None);
}

#[test]
fn effort_without_argument_opens_the_picker_on_default() {
    let mut app = app_con_niveles(None);
    assert_eq!(escribir(&mut app, "/effort"), None);
    assert_eq!(app.quick_effort_picker, Some(0));
    assert!(!app.quick_model_picker);
    assert!(!app.settings_open);
    assert!(app.composer.is_empty());
}

#[test]
fn down_and_enter_apply_the_next_level() {
    let mut app = app_con_niveles(None);
    escribir(&mut app, "/effort");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.quick_effort_picker, None);
    assert_eq!(esfuerzo(&app).as_deref(), Some("low"));
    assert!(app.settings_save_requested);
    assert!(!app.settings_open);
}

#[test]
fn enter_on_the_default_row_clears_the_level() {
    let mut app = app_con_niveles(Some("high"));
    escribir(&mut app, "/effort");
    assert_eq!(app.quick_effort_picker, Some(3));
    for _ in 0..3 {
        handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    }
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.quick_effort_picker, None);
    assert_eq!(esfuerzo(&app), None);
    assert!(app.settings_save_requested);
}

#[test]
fn escape_cancels_the_picker_without_changes() {
    let mut app = app_con_niveles(None);
    escribir(&mut app, "/effort");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.quick_effort_picker, None);
    assert_eq!(esfuerzo(&app), None);
    assert!(!app.settings_save_requested);
    assert!(!app.settings_open);
}

#[test]
fn a_paste_while_the_effort_picker_is_open_does_not_reach_the_composer() {
    let mut app = app_con_niveles(None);
    escribir(&mut app, "/effort");
    assert!(
        app.quick_effort_picker.is_some(),
        "el selector debía abrirse"
    );
    handle_event(&mut app, Event::Paste("texto pegado".into()));
    assert!(
        app.composer.is_empty(),
        "el pegado no debe colarse en el compositor: {:?}",
        app.composer.text()
    );
}
