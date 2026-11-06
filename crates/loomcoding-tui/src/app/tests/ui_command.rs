//! Pruebas de `/ui` y del popup de `/` sin alias: la lógica vive en `App`, sin teclas.
//!
//! Cabe señalar que las pruebas de teclado (alias tecleados enteros, errores del compositor)
//! viven en `run/tests/ui_command.rs`.

use super::*;
use crate::app::ui_panel::UiRow;
use loomcoding_cloth::widgets::{LayoutStyle, StyleBundle, ThemePreset};

/// Los ocho alias que salen del popup de `/` y siguen ejecutándose al teclearlos enteros.
const ALIASES: [&str; 8] = [
    "/theme", "/layout", "/temas", "/crush", "/classic", "/mascot", "/sidebar", "/mouse",
];

/// Nombres de los comandos que el popup muestra para el texto actual del compositor.
fn nombres_del_popup(app: &App) -> Vec<&'static str> {
    app.slash_matches().iter().map(|(_, n, _)| *n).collect()
}

#[test]
fn the_slash_popup_hides_the_aliases_and_lists_ui() {
    let mut app = App::new();
    app.composer.set_text("/");
    let nombres = nombres_del_popup(&app);
    for alias in ALIASES {
        assert!(
            !nombres.contains(&alias),
            "{alias} sigue en el popup: {nombres:?}"
        );
    }
    for visible in ["/ui", "/model", "/effort"] {
        assert!(nombres.contains(&visible), "falta {visible}: {nombres:?}");
    }
}

#[test]
fn the_popup_keeps_ui_while_it_takes_arguments() {
    let mut app = App::new();
    app.composer.set_text("/ui layout rail");
    assert_eq!(app.slash_query(), Some("/ui"));
    assert_eq!(nombres_del_popup(&app), vec!["/ui"]);
}

#[test]
fn ui_without_arguments_opens_the_panel_at_the_root() {
    let mut app = App::new();
    app.ui_command("").unwrap();
    assert!(app.temas_open);
    assert_eq!(app.temas_level, TemasLevel::Root);
}

#[test]
fn ui_layout_sets_the_value_and_reports_it() {
    let mut app = App::new();
    app.ui_command("layout rail").unwrap();
    assert_eq!(app.theme.look.layout, LayoutStyle::ContextRail);
    assert_eq!(app.slash_feedback.as_deref(), Some("layout: rail"));
    app.ui_command("layout workspace").unwrap();
    assert_eq!(app.theme.look.layout, LayoutStyle::Workspace);
    assert_eq!(app.slash_feedback.as_deref(), Some("layout: workspace"));
}

#[test]
fn ui_style_sets_a_bundle_and_without_value_alternates() {
    let mut app = App::new();
    app.ui_command("style flat").unwrap();
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(app.slash_feedback.as_deref(), Some("style: flat"));
    let antes = app.ui_row_value(UiRow::Style);
    app.ui_command("style").unwrap();
    assert_ne!(app.ui_row_value(UiRow::Style), antes);
    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|aviso| aviso.starts_with("style: ")));
}

#[test]
fn ui_theme_sets_a_preset_by_name_ignoring_case() {
    let mut app = App::new();
    app.ui_command("theme crush").unwrap();
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("theme: Crush · Charmtone")
    );
    app.ui_command("THEME Mastracode").unwrap();
    assert_eq!(app.theme_preset, ThemePreset::MastraCode);
}

#[test]
fn ui_theme_with_an_unknown_name_fails_and_changes_nothing() {
    let mut app = App::new();
    let antes = app.theme_preset;
    let err = app.ui_command("theme neon").unwrap_err();
    assert_eq!(app.theme_preset, antes);
    assert!(err.starts_with("Unknown theme: neon. Options: "), "{err}");
    assert!(err.contains("crush") && err.contains("cloth"), "{err}");
}

#[test]
fn ui_mascot_takes_on_and_off_and_rejects_anything_else() {
    let mut app = App::new();
    app.ui_command("mascot off").unwrap();
    assert!(!app.mascot);
    assert_eq!(app.slash_feedback.as_deref(), Some("mascot: off"));
    app.ui_command("mascot on").unwrap();
    assert!(app.mascot);
    assert_eq!(app.slash_feedback.as_deref(), Some("mascot: on"));
    let err = app.ui_command("mascot maybe").unwrap_err();
    assert_eq!(err, "Unknown mascot: maybe. Options: on, off");
    assert!(app.mascot);
}

#[test]
fn ui_mouse_and_sidebar_take_an_explicit_value() {
    let mut app = App::new();
    app.ui_command("mouse off").unwrap();
    assert!(!app.mouse_capture);
    assert_eq!(app.slash_feedback.as_deref(), Some("mouse: off"));
    app.ui_command("mouse on").unwrap();
    assert!(app.mouse_capture);

    app.ui_command("sidebar off").unwrap();
    assert_eq!(app.ui_row_value(UiRow::Sidebar), "off");
    assert_eq!(app.slash_feedback.as_deref(), Some("sidebar: off"));
    app.ui_command("sidebar on").unwrap();
    assert_eq!(app.ui_row_value(UiRow::Sidebar), "on");
}

#[test]
fn ui_effects_opens_its_submenu_and_refuses_a_value() {
    let mut app = App::new();
    app.ui_command("effects").unwrap();
    assert!(app.temas_open);
    assert_eq!(app.temas_level, TemasLevel::Effects);
    assert_eq!(
        app.ui_command("effects x").unwrap_err(),
        "Use the panel: /ui effects"
    );
}

#[test]
fn ui_spinner_rotates_and_refuses_a_value() {
    let mut app = App::new();
    let antes = app.ui_row_value(UiRow::Spinner);
    app.ui_command("spinner").unwrap();
    assert_ne!(app.ui_row_value(UiRow::Spinner), antes);
    assert!(app
        .slash_feedback
        .as_deref()
        .is_some_and(|aviso| aviso.starts_with("spinner: ")));
    assert_eq!(
        app.ui_command("spinner dots").unwrap_err(),
        "Use the panel: /ui spinner"
    );
}

#[test]
fn ui_with_an_unknown_row_lists_the_rows() {
    let mut app = App::new();
    assert_eq!(
        app.ui_command("x").unwrap_err(),
        "Unknown row: x. Rows: theme, style, layout, effects, spinner, mascot, sidebar, mouse"
    );
}

#[test]
fn ui_row_names_ignore_case_and_surrounding_spaces() {
    let mut app = App::new();
    app.ui_command("  LAYOUT   rail ").unwrap();
    assert_eq!(app.theme.look.layout, LayoutStyle::ContextRail);
}
