//! `/effort` a nivel de API: fija el nivel de razonamiento por el mismo camino que `/model`
//! (`settings_save_requested`) y, sin argumento, abre un selector pequeño. Cabe señalar que las
//! reglas de turno en marcha y de ajustes ausentes son las de `/settings`, para que no divergan.

use super::*;
use crate::commands::{Command, COMMANDS};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Aviso para un modelo sin niveles de razonamiento ni `[provider] efforts` declarados.
const SIN_NIVELES: &str =
    "This model has no reasoning levels. Declare them in [provider] efforts of loomcoding.toml";

/// Ajustes de Anthropic con el modelo y el esfuerzo indicados.
fn ajustes(model: &str, effort: Option<&str>) -> ProviderSettings {
    ProviderSettings {
        kind: loomcoding_core::config::ProviderKind::Anthropic,
        model: model.into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        effort: effort.map(str::to_string),
    }
}

/// Aplicación con `claude-think` (niveles low, medium y high) y el catálogo de razonamiento.
fn app_con_niveles(effort: Option<&str>) -> App {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(ajustes("claude-think", effort));
    app
}

fn esfuerzo(app: &App) -> Option<String> {
    app.provider_settings.as_ref().unwrap().effort.clone()
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
fn effort_high_sets_the_level_saves_and_announces_it() {
    let mut app = app_con_niveles(None);
    assert_eq!(app.set_effort_now(Some("high")), Ok(()));
    assert_eq!(esfuerzo(&app).as_deref(), Some("high"));
    assert!(app.settings_save_requested);
    assert_eq!(app.slash_feedback.as_deref(), Some("effort: high"));
}

#[test]
fn default_in_any_case_and_with_spaces_clears_the_level() {
    for texto in ["default", "DEFAULT", "  default  "] {
        let mut app = app_con_niveles(Some("high"));
        assert_eq!(app.set_effort_now(Some(texto)), Ok(()), "{texto:?}");
        assert_eq!(esfuerzo(&app), None, "{texto:?}");
        assert!(app.settings_save_requested, "{texto:?}");
        assert_eq!(
            app.slash_feedback.as_deref(),
            Some("effort: default"),
            "{texto:?}"
        );
    }
}

#[test]
fn no_argument_also_means_default() {
    let mut app = app_con_niveles(Some("low"));
    assert_eq!(app.set_effort_now(None), Ok(()));
    assert_eq!(esfuerzo(&app), None);
    assert!(app.settings_save_requested);
}

#[test]
fn an_unknown_level_lists_the_real_options_and_changes_nothing() {
    let mut app = app_con_niveles(Some("low"));
    assert_eq!(
        app.set_effort_now(Some("ultra")),
        Err("Usage: /effort [default|low|medium|high]".into())
    );
    assert_eq!(esfuerzo(&app).as_deref(), Some("low"));
    assert!(!app.settings_save_requested);
}

#[test]
fn a_model_without_levels_asks_to_declare_them() {
    let mut app = App::new();
    app.set_catalog(crate::app::reasoning_sample_catalog());
    app.set_provider_settings(ajustes("claude-plain", None));
    assert_eq!(app.set_effort_now(Some("high")), Err(SIN_NIVELES.into()));
    assert_eq!(app.open_effort_picker_now(), Err(SIN_NIVELES.into()));
    assert!(!app.settings_save_requested);
    assert!(app.quick_effort_picker.is_none());
}

#[test]
fn without_provider_settings_effort_is_unavailable() {
    let mut app = App::new();
    let unavailable = Err("Provider settings are unavailable".to_string());
    assert_eq!(app.set_effort_now(Some("high")), unavailable);
    assert_eq!(app.open_effort_picker_now(), unavailable);
    assert!(app.quick_effort_picker.is_none());
    assert!(!app.settings_save_requested);
}

#[test]
fn a_turn_in_flight_blocks_the_effort_change() {
    let mut app = app_con_niveles(None);
    app.turn_in_flight = true;
    let err = app.set_effort_now(Some("high")).unwrap_err();
    assert!(err.contains("active turn"), "{err}");
    assert_eq!(esfuerzo(&app), None);
    assert!(!app.settings_save_requested);
}

#[test]
fn a_save_in_progress_blocks_the_effort_change() {
    let mut app = app_con_niveles(None);
    app.settings_saving = true;
    assert_eq!(
        app.set_effort_now(Some("high")),
        Err("Settings are still being saved".into())
    );
    assert_eq!(esfuerzo(&app), None);
    assert!(!app.settings_save_requested);
}

#[test]
fn the_same_level_in_other_case_is_acknowledged_without_resending() {
    let mut app = app_con_niveles(Some("high"));
    assert_eq!(app.set_effort_now(Some("HIGH")), Ok(()));
    assert!(!app.settings_save_requested);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("effort: high (unchanged)")
    );
    assert_eq!(esfuerzo(&app).as_deref(), Some("high"));
}

#[test]
fn default_when_already_unset_is_acknowledged_without_resending() {
    let mut app = app_con_niveles(None);
    assert_eq!(app.set_effort_now(Some("default")), Ok(()));
    assert!(!app.settings_save_requested);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("effort: default (unchanged)")
    );
}

#[test]
fn declared_levels_count_when_the_catalog_publishes_none() {
    let mut app = App::new();
    app.set_provider_settings(ajustes("local-model", None));
    app.set_declared_efforts(vec!["none".into(), "low".into(), "xhigh".into()]);
    assert_eq!(app.set_effort_now(Some("XHIGH")), Ok(()));
    assert_eq!(esfuerzo(&app).as_deref(), Some("xhigh"));
    assert!(app.settings_save_requested);
    assert_eq!(app.open_effort_picker_now(), Ok(()));
    assert_eq!(app.quick_effort_picker, Some(3));
}

#[test]
fn the_picker_starts_on_the_current_level() {
    let mut app = app_con_niveles(Some("medium"));
    assert_eq!(app.open_effort_picker_now(), Ok(()));
    // Fila 0 es «default»; la 2 corresponde a «medium».
    assert_eq!(app.quick_effort_picker, Some(2));
}

#[test]
fn the_picker_starts_on_default_when_unset() {
    let mut app = app_con_niveles(None);
    assert_eq!(app.open_effort_picker_now(), Ok(()));
    assert_eq!(app.quick_effort_picker, Some(0));
}

#[test]
fn the_picker_moves_without_wrapping_around() {
    let mut app = app_con_niveles(Some("high"));
    app.open_effort_picker_now().unwrap();
    for _ in 0..5 {
        app.effort_picker_move(true);
    }
    assert_eq!(app.quick_effort_picker, Some(3));
    app.effort_picker_move(false);
    app.effort_picker_move(false);
    app.effort_picker_move(false);
    app.effort_picker_move(false);
    app.effort_picker_move(false);
    assert_eq!(app.quick_effort_picker, Some(0));
}

#[test]
fn the_picker_draws_default_the_levels_and_the_current_mark() {
    let mut app = app_con_niveles(Some("medium"));
    app.open_effort_picker_now().unwrap();
    let content = pintado(&mut app, 100, 30);
    for word in ["default", "low", "medium", "high", "●"] {
        assert!(content.contains(word), "falta {word}: {content}");
    }
}

#[test]
fn the_picker_survives_tiny_areas() {
    let mut app = app_con_niveles(Some("low"));
    app.open_effort_picker_now().unwrap();
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 5)] {
        let _ = pintado(&mut app, w, h);
    }
}

#[test]
fn effort_is_listed_with_its_short_description() {
    let entry = COMMANDS
        .iter()
        .find(|(command, _, _)| *command == Command::Effort)
        .expect("/effort está en la lista de comandos");
    assert_eq!(entry.1, "/effort");
    assert_eq!(
        entry.2,
        "Change reasoning effort: /effort <level|default> (no level: pick)"
    );
}

#[test]
fn the_popup_keeps_effort_open_with_an_argument() {
    let mut app = App::new();
    app.composer.set_text("/effort high");
    assert_eq!(app.slash_query(), Some("/effort"));
    app.composer.set_text("/ef");
    assert!(app
        .slash_matches()
        .iter()
        .any(|(command, name, _)| *command == Command::Effort && *name == "/effort"));
}

#[test]
fn help_lists_effort_inside_an_80_by_24_frame() {
    let mut app = App::new();
    app.help_open = true;
    let content = pintado(&mut app, 80, 24);
    assert!(content.contains("/effort"), "{content}");
    // Si el contenido no cabe, el marco inferior no se dibuja y falta el cierre.
    assert!(content.contains("Ctrl+C/Q"), "{content}");
    assert!(content.contains("Press F1 or Esc to close"), "{content}");
}
