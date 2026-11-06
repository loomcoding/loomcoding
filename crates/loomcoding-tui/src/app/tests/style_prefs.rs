//! Tests del estilo elegido por el usuario (`set_style`) frente al tema y al layout.
//! Viven aparte de `theme_and_settings.rs` para que ese fichero no siga creciendo.

use super::*;
use loomcoding_cloth::widgets::{LayoutStyle, StyleBundle, ThemePreset};

#[test]
fn set_style_changes_the_bundle_and_keeps_the_layout() {
    let mut app = App::new();
    app.set_theme(ThemePreset::Cloth);
    app.set_layout(LayoutStyle::Minimal);
    app.set_style(StyleBundle::Flat);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(app.theme.look.layout, LayoutStyle::Minimal);
    assert_eq!(app.style_override, Some(StyleBundle::Flat));
}

#[test]
fn set_style_persists_and_reloads_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    app.set_style(StyleBundle::Classic);
    assert_eq!(crate::prefs::load_style(&path), Some(StyleBundle::Classic));

    let mut reboot = App::new();
    reboot.set_prefs_path(path.clone());
    assert_eq!(reboot.theme.look.bundle(), Some(StyleBundle::Classic));
    assert_eq!(reboot.style_override, Some(StyleBundle::Classic));
}

#[test]
fn a_save_error_warns_but_still_applies_the_style() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    // Un directorio con el nombre del fichero hace fallar la escritura.
    std::fs::create_dir(&path).unwrap();
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    app.set_style(StyleBundle::Flat);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    let aviso = app.slash_feedback.as_deref().unwrap_or_default();
    assert!(aviso.starts_with("Style not saved"), "{aviso}");
}

#[test]
fn set_theme_after_set_style_keeps_the_style_and_the_layout() {
    let mut app = App::new();
    app.set_layout(LayoutStyle::Cards);
    app.set_style(StyleBundle::Classic);
    app.set_theme(ThemePreset::Crush);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
    assert_eq!(app.theme.look.layout, LayoutStyle::Cards);
}

#[test]
fn without_a_style_override_a_preset_change_brings_its_own_style() {
    let mut app = App::new();
    app.set_theme(ThemePreset::Cloth);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
    app.set_theme(ThemePreset::Crush);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(app.style_override, None);
}

#[test]
fn an_old_ui_toml_without_style_keeps_the_preset_style() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "theme = \"cloth\"\nmouse = true\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.theme_preset, ThemePreset::Cloth);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
    assert_eq!(app.style_override, None);
}

#[test]
fn a_saved_style_overrides_the_preset_style_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "theme = \"crush\"\nstyle = \"classic\"\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
}

#[test]
fn an_invalid_saved_style_is_ignored_without_breaking_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "theme = \"crush\"\nstyle = \"raro\"\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(app.style_override, None);
}

#[test]
fn a_saved_layout_still_wins_over_the_preset_when_a_style_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(
        &path,
        "theme = \"crush\"\nlayout = \"cards\"\nstyle = \"classic\"\n",
    )
    .unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert_eq!(app.theme.look.layout, LayoutStyle::Cards);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
    assert_eq!(app.layout_override, Some(LayoutStyle::Cards));
}
