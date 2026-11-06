//! `/model` a nivel de API: fija el modelo por el mismo camino que `/settings`, sin abrir su
//! formulario. Cabe señalar que las reglas de turno en marcha y de ajustes ausentes se copian
//! de `/settings`, para que ambos caminos no puedan divergir.

use super::*;
use crate::commands::{Command, COMMANDS};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Ajustes de Anthropic con el modelo indicado, sin esfuerzo y con la URL del catálogo de muestra.
fn settings_for(model: &str) -> ProviderSettings {
    ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: model.into(),
        base_url: "https://api.anthropic.com".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: None,
    }
}

/// Aplicación con ajustes y sin catálogo.
fn app_con_ajustes() -> App {
    let mut app = App::new();
    app.set_provider_settings(settings_for("model-a"));
    app
}

/// Aplicación con ajustes y el catálogo de muestra (dos modelos de Anthropic).
fn app_con_catalogo() -> App {
    let mut app = app_con_ajustes();
    app.set_catalog(crate::app::picker_sample_catalog());
    app
}

fn modelo(app: &App) -> String {
    app.provider_settings.as_ref().unwrap().model.clone()
}

#[test]
fn model_with_an_id_sets_the_model_saves_and_announces_it() {
    let mut app = app_con_ajustes();
    assert_eq!(app.set_model_now("model-b"), Ok(()));
    assert_eq!(modelo(&app), "model-b");
    assert!(app.settings_save_requested);
    assert_eq!(app.slash_feedback.as_deref(), Some("model: model-b"));
}

#[test]
fn an_id_outside_the_catalog_is_accepted() {
    let mut app = app_con_catalogo();
    assert_eq!(app.set_model_now("Ornith-1.5-9B"), Ok(()));
    assert_eq!(modelo(&app), "Ornith-1.5-9B");
    assert!(app.settings_save_requested);
}

#[test]
fn the_current_id_is_acknowledged_without_resending() {
    let mut app = app_con_ajustes();
    assert_eq!(app.set_model_now("model-a"), Ok(()));
    assert!(!app.settings_save_requested);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("model: model-a (unchanged)")
    );
}

#[test]
fn empty_or_spaced_ids_are_rejected_and_change_nothing() {
    let mut app = app_con_ajustes();
    for id in ["", "   ", "a b", "a\nb"] {
        assert_eq!(
            app.set_model_now(id),
            Err("Usage: /model <id>".into()),
            "id {id:?}"
        );
        assert_eq!(modelo(&app), "model-a");
        assert!(!app.settings_save_requested);
    }
}

#[test]
fn without_provider_settings_the_command_is_unavailable() {
    let mut app = App::new();
    let unavailable = Err("Provider settings are unavailable".to_string());
    assert_eq!(app.set_model_now("x"), unavailable);
    assert_eq!(app.open_model_picker_now(), unavailable);
    assert!(!app.settings_save_requested);
    assert!(!app.quick_model_picker);
}

#[test]
fn a_turn_in_flight_blocks_the_change_like_settings_does() {
    let mut app = app_con_ajustes();
    app.turn_in_flight = true;
    let err = app.set_model_now("model-b").unwrap_err();
    assert!(err.contains("active turn"), "{err}");
    assert_eq!(modelo(&app), "model-a");
    assert!(!app.settings_save_requested);
}

#[test]
fn without_a_catalog_the_picker_asks_for_an_id() {
    let mut app = app_con_ajustes();
    assert_eq!(
        app.open_model_picker_now(),
        Err("No model catalog for this provider: use /model <id>".into())
    );
    assert!(app.settings_picker.is_none());
    assert!(!app.quick_model_picker);
    assert!(!app.settings_open);
}

#[test]
fn with_a_catalog_the_quick_picker_opens_without_the_settings_form() {
    let mut app = app_con_catalogo();
    assert_eq!(app.open_model_picker_now(), Ok(()));
    assert!(app.quick_model_picker);
    assert_eq!(app.settings_picker, Some(SettingsPicker::Model));
    assert!(!app.settings_open);
    assert!(!app.settings_save_requested);
    assert!(app.picker_items().len() >= 2);
}

#[test]
fn a_model_that_lacks_the_current_effort_resets_it_and_says_so() {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        base_url: "https://api.anthropic.com/v1".into(),
        effort: Some("high".into()),
        ..settings_for("claude-think")
    });
    assert_eq!(app.set_model_now("claude-plain"), Ok(()));
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.effort, None);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("model: claude-plain · effort reset")
    );
    assert!(app.settings_save_requested);
}

#[test]
fn an_effort_the_new_model_still_supports_is_kept_silently() {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(ProviderSettings {
        base_url: "https://api.anthropic.com/v1".into(),
        effort: Some("low".into()),
        ..settings_for("claude-mid")
    });
    assert_eq!(app.set_model_now("claude-think"), Ok(()));
    let settings = app.provider_settings.as_ref().unwrap();
    assert_eq!(settings.effort.as_deref(), Some("low"));
    assert_eq!(app.slash_feedback.as_deref(), Some("model: claude-think"));
}

#[test]
fn the_popup_keeps_model_open_with_an_argument() {
    let mut app = App::new();
    app.composer.set_text("/model x");
    assert_eq!(app.slash_query(), Some("/model"));
    app.composer.set_text("/mo");
    assert!(app
        .slash_matches()
        .iter()
        .any(|(command, name, _)| *command == Command::Model && *name == "/model"));
}

#[test]
fn model_is_listed_with_its_short_description() {
    let entry = COMMANDS
        .iter()
        .find(|(command, _, _)| *command == Command::Model)
        .expect("/model está en la lista de comandos");
    assert_eq!(entry.1, "/model");
    assert_eq!(
        entry.2,
        "Change the model: /model <id> (no id: pick from the catalog)"
    );
}

/// Texto completo de un fotograma de `app` de `w`×`h` celdas.
fn pintado(app: &mut App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|frame| crate::render::render(frame, app))
        .unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn quick_picker_draws_the_catalog_models() {
    let mut app = app_con_catalogo();
    app.open_model_picker_now().unwrap();
    let content = pintado(&mut app, 100, 30);
    assert!(content.contains("Claude Sonnet"), "{content}");
    assert!(content.contains("Claude Opus"), "{content}");
}

#[test]
fn quick_picker_survives_tiny_areas() {
    let mut app = app_con_catalogo();
    app.open_model_picker_now().unwrap();
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5)] {
        let _ = pintado(&mut app, w, h);
    }
}

#[test]
fn help_lists_model_in_an_80_by_24_frame() {
    let mut app = App::new();
    app.help_open = true;
    let content = pintado(&mut app, 80, 24);
    assert!(content.contains("/model"), "{content}");
}
