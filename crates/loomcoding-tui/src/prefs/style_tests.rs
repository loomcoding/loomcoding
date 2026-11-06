//! Pruebas de la preferencia de estilo (`style = "classic"|"flat"`). Viven aparte de
//! `prefs.rs` para que ese fichero no siga creciendo.

use super::*;
use loomcoding_cloth::widgets::StyleBundle;

/// Cuenta las líneas que asignan exactamente la clave `name`.
fn asignaciones(path: &Path, name: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| is_assignment_of(line, name))
        .count()
}

#[test]
fn style_round_trips_for_both_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    assert_eq!(load_style(&path), None);
    for bundle in [StyleBundle::Flat, StyleBundle::Classic, StyleBundle::Flat] {
        save_style(&path, bundle).unwrap();
        assert_eq!(load_style(&path), Some(bundle), "style {bundle:?}");
    }
    assert_eq!(asignaciones(&path, "style"), 1);
}

#[test]
fn style_invalid_values_load_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    for contenido in [
        "style = \"otro\"\n",
        "style = \"Flat\"\n",
        "style = flat\n",
        "style = 1\n",
        "style =\n",
    ] {
        std::fs::write(&path, contenido).unwrap();
        assert_eq!(load_style(&path), None, "contenido {contenido:?}");
    }
}

#[test]
fn style_missing_key_loads_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    save_theme(&path, ThemePreset::Crush).unwrap();
    assert_eq!(load_style(&path), None);
    assert_eq!(load_style(&dir.path().join("no-existe.toml")), None);
}

#[test]
fn saving_style_keeps_the_other_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    save_theme(&path, ThemePreset::Crush).unwrap();
    save_layout(&path, LayoutStyle::Cards).unwrap();
    save_mascot(&path, false).unwrap();
    save_logo_fx(&path, true).unwrap();

    save_style(&path, StyleBundle::Flat).unwrap();

    assert_eq!(load_theme(&path), Some(ThemePreset::Crush));
    assert_eq!(load_layout(&path), Some(LayoutStyle::Cards));
    assert_eq!(load_mascot(&path), Some(false));
    assert_eq!(load_logo_fx(&path), Some(true));
    assert_eq!(load_style(&path), Some(StyleBundle::Flat));
}

#[test]
fn saving_the_other_keys_keeps_style() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    save_style(&path, StyleBundle::Flat).unwrap();

    save_theme(&path, ThemePreset::Pawan).unwrap();
    save_layout(&path, LayoutStyle::Minimal).unwrap();
    save_mascot(&path, true).unwrap();
    save_logo_fx(&path, false).unwrap();

    assert_eq!(load_style(&path), Some(StyleBundle::Flat));
    assert_eq!(asignaciones(&path, "style"), 1);
}
