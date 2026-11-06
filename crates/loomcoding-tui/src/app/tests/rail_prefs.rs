//! Tests del carril del transcript en la aplicación: `/ui carril`, la fila «Carril» del panel y
//! `rail` en `ui.toml`. Viven aparte para que `style_prefs.rs` y `ui_panel.rs` no crezcan.

use super::*;
use crate::app::ui_panel::UiRow;
use crate::render::rail::RailStyle;
use loomcoding_cloth::widgets::ThemePreset;

/// Ruta de `ui.toml` dentro de un directorio temporal, con la app ya apuntando a ella.
fn app_con_prefs(dir: &tempfile::TempDir) -> (App, std::path::PathBuf) {
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    (app, path)
}

#[test]
fn the_default_rail_is_the_tree() {
    let app = App::new();
    assert_eq!(app.rail_style, RailStyle::Tree);
    assert_eq!(app.ui_row_value(UiRow::Rail), "tree");
}

#[test]
fn the_rail_row_cycles_both_ways_through_every_style() {
    let mut app = App::new();
    app.ui_row_cycle(UiRow::Rail, true);
    assert_eq!(app.rail_style, RailStyle::Dots);
    app.ui_row_cycle(UiRow::Rail, false);
    app.ui_row_cycle(UiRow::Rail, false);
    assert_eq!(app.rail_style, RailStyle::None);
    assert_eq!(app.ui_row_value(UiRow::Rail), "none");
}

#[test]
fn ui_carril_sets_and_saves_the_style() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    app.ui_command("carril none").unwrap();
    assert_eq!(app.rail_style, RailStyle::None);
    assert_eq!(crate::prefs::load_rail(&path), Ok(Some(RailStyle::None)));
    assert_eq!(app.slash_feedback.as_deref(), Some("carril: none"));
}

#[test]
fn ui_carril_accepts_box_and_puts_the_user_box_on_the_theme() {
    let mut app = App::new();
    app.ui_command("carril BOX").unwrap();
    assert_eq!(app.rail_style, RailStyle::Box);
    assert!(app.theme.rail_box);
    app.ui_command("carril tree").unwrap();
    assert!(!app.theme.rail_box);
}

#[test]
fn an_unknown_carril_value_lists_the_valid_ones_and_changes_nothing() {
    let mut app = App::new();
    app.set_rail(RailStyle::Dots);
    let err = app.ui_command("carril raro").unwrap_err();
    assert!(
        err.contains("Options: tree, dots, bar, box, tint, none"),
        "{err}"
    );
    assert_eq!(app.rail_style, RailStyle::Dots);
}

#[test]
fn ui_carril_without_value_cycles_the_row() {
    let mut app = App::new();
    app.ui_command("carril").unwrap();
    assert_eq!(app.rail_style, RailStyle::Dots);
}

#[test]
fn a_missing_rail_keeps_the_tree_without_warning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "theme = \"crush\"\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.rail_style, RailStyle::Tree);
    assert_eq!(app.slash_feedback, None);
}

#[test]
fn an_unknown_rail_in_ui_toml_warns_and_falls_back_to_tree() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "rail = \"raro\"\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.rail_style, RailStyle::Tree);
    let aviso = app.slash_feedback.as_deref().unwrap_or_default();
    assert!(
        aviso.starts_with("Unknown rail in ui.toml: raro"),
        "{aviso}"
    );
    assert!(
        aviso.contains("tree, dots, bar, box, tint, none"),
        "{aviso}"
    );
}

#[test]
fn the_saved_rail_is_restored_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    app.set_rail(RailStyle::Bar);

    let mut reboot = App::new();
    reboot.set_prefs_path(path);
    assert_eq!(reboot.rail_style, RailStyle::Bar);
}

#[test]
fn the_rail_survives_a_theme_change_in_ui_toml() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    app.set_rail(RailStyle::Bar);
    app.set_theme(ThemePreset::Pawan);

    assert_eq!(app.rail_style, RailStyle::Bar);
    assert_eq!(crate::prefs::load_rail(&path), Ok(Some(RailStyle::Bar)));
    assert_eq!(crate::prefs::load_theme(&path), Some(ThemePreset::Pawan));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("rail").count(), 1);
}
