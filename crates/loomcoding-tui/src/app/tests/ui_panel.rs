//! Pruebas de las filas del panel `/ui`: nombres, valores, ciclos y avisos.
//! Viven aparte para que `theme_and_settings.rs` no siga creciendo.

use super::*;
use crate::app::ui_panel::UiRow;
use loomcoding_cloth::widgets::{ChromeStyle, LayoutStyle, StyleBundle, ThemePreset, ToolStyle};

/// Ruta de `ui.toml` dentro de un directorio temporal, con la app ya apuntando a ella.
fn app_con_prefs(dir: &tempfile::TempDir) -> (App, std::path::PathBuf) {
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    (app, path)
}

#[test]
fn ui_row_all_lists_the_nine_rows_in_panel_order() {
    assert_eq!(
        UiRow::ALL,
        [
            UiRow::Theme,
            UiRow::Style,
            UiRow::Layout,
            UiRow::Rail,
            UiRow::Effects,
            UiRow::Spinner,
            UiRow::Mascot,
            UiRow::Sidebar,
            UiRow::Mouse,
        ]
    );
}

#[test]
fn ui_row_names_round_trip_and_unknown_text_is_none() {
    for row in UiRow::ALL {
        assert_eq!(UiRow::from_name(row.name()), Some(row));
    }
    assert_eq!(UiRow::Mascot.name(), "mascot");
    assert_eq!(UiRow::from_name("otro"), None);
    assert_eq!(UiRow::from_name(""), None);
    assert_eq!(UiRow::from_name("Theme"), None);
}

#[test]
fn ui_row_labels_are_non_empty_and_distinct() {
    let labels: Vec<&str> = UiRow::ALL.iter().map(|row| row.label()).collect();
    assert!(labels.iter().all(|label| !label.is_empty()));
    let mut unique = labels.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), labels.len(), "{labels:?}");
    assert_eq!(UiRow::Layout.label(), "Distribución");
    assert_eq!(UiRow::Mascot.label(), "Muñeco");
}

#[test]
fn ui_row_flags_match_the_spec() {
    let submenus: Vec<UiRow> = UiRow::ALL
        .into_iter()
        .filter(|row| row.has_submenu())
        .collect();
    assert_eq!(submenus, [UiRow::Theme, UiRow::Effects, UiRow::Spinner]);
    let toggles: Vec<UiRow> = UiRow::ALL
        .into_iter()
        .filter(|row| row.is_toggle())
        .collect();
    assert_eq!(toggles, [UiRow::Mascot, UiRow::Sidebar, UiRow::Mouse]);
}

#[test]
fn ui_row_value_shows_the_default_theme_name() {
    let app = App::new();
    assert_eq!(app.ui_row_value(UiRow::Theme), app.theme_preset.name());
}

#[test]
fn ui_row_value_follows_set_style_and_set_layout() {
    let mut app = App::new();
    app.set_style(StyleBundle::Flat);
    assert_eq!(app.ui_row_value(UiRow::Style), "flat");
    app.set_layout(LayoutStyle::Minimal);
    assert_eq!(app.ui_row_value(UiRow::Layout), "minimal");
}

#[test]
fn ui_row_value_reports_a_mixed_look_as_custom() {
    let mut app = App::new();
    app.set_style(StyleBundle::Classic);
    app.theme.look.chrome = ChromeStyle::Flat;
    assert_eq!(app.ui_row_value(UiRow::Style), "custom");
}

#[test]
fn ui_row_value_toggles_read_on_and_off() {
    let mut app = App::new();
    app.set_mascot(true);
    assert_eq!(app.ui_row_value(UiRow::Mascot), "on");
    app.set_mascot(false);
    assert_eq!(app.ui_row_value(UiRow::Mascot), "off");

    app.set_mouse_capture(true);
    assert_eq!(app.ui_row_value(UiRow::Mouse), "on");
    app.set_mouse_capture(false);
    assert_eq!(app.ui_row_value(UiRow::Mouse), "off");

    app.sidebar_expanded = false;
    assert_eq!(app.ui_row_value(UiRow::Sidebar), "off");
    app.sidebar_expanded = true;
    assert_eq!(app.ui_row_value(UiRow::Sidebar), "on");
}

#[test]
fn ui_row_value_effects_summarises_rain_and_logo_without_io() {
    let mut app = App::new();
    app.set_logo_fx(true);
    assert_eq!(app.ui_row_value(UiRow::Effects), "matrix off · logo on");
    app.set_logo_fx(false);
    assert_eq!(app.ui_row_value(UiRow::Effects), "matrix off · logo off");
    app.matrix_rain.start();
    assert_eq!(app.ui_row_value(UiRow::Effects), "matrix on · logo off");
}

#[test]
fn ui_row_value_spinner_defaults_to_the_random_label() {
    let app = App::new();
    assert_eq!(
        app.ui_row_value(UiRow::Spinner),
        SpinnerChoice::Random.label()
    );
}

#[test]
fn theme_row_cycles_forward_and_back_with_wraparound_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    let inicio = app.theme_preset;
    let total = ThemePreset::ALL.len();

    for _ in 0..total {
        app.ui_row_cycle(UiRow::Theme, true);
    }
    assert_eq!(
        app.theme_preset, inicio,
        "una vuelta completa vuelve al inicio"
    );

    app.ui_row_cycle(UiRow::Theme, true);
    let siguiente = app.theme_preset;
    assert_eq!(crate::prefs::load_theme(&path), Some(siguiente));

    app.ui_row_cycle(UiRow::Theme, false);
    assert_eq!(app.theme_preset, inicio, "hacia atrás deshace el paso");
    app.ui_row_cycle(UiRow::Theme, false);
    let indice = ThemePreset::ALL.iter().position(|p| *p == inicio).unwrap();
    assert_eq!(
        app.theme_preset,
        ThemePreset::ALL[(indice + total - 1) % total]
    );
}

#[test]
fn style_row_alternates_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    app.set_style(StyleBundle::Classic);

    app.ui_row_cycle(UiRow::Style, true);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert_eq!(crate::prefs::load_style(&path), Some(StyleBundle::Flat));

    app.ui_row_cycle(UiRow::Style, true);
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Classic));
    assert_eq!(crate::prefs::load_style(&path), Some(StyleBundle::Classic));
}

#[test]
fn layout_row_walks_rail_workspace_minimal_cards_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    app.set_layout(LayoutStyle::ContextRail);

    let hacia_delante = [
        LayoutStyle::Workspace,
        LayoutStyle::Minimal,
        LayoutStyle::Cards,
        LayoutStyle::ContextRail,
    ];
    for esperado in hacia_delante {
        app.ui_row_cycle(UiRow::Layout, true);
        assert_eq!(app.theme.look.layout, esperado);
        assert_eq!(crate::prefs::load_layout(&path), Some(esperado));
    }

    let hacia_atras = [
        LayoutStyle::Cards,
        LayoutStyle::Minimal,
        LayoutStyle::Workspace,
        LayoutStyle::ContextRail,
    ];
    for esperado in hacia_atras {
        app.ui_row_cycle(UiRow::Layout, false);
        assert_eq!(app.theme.look.layout, esperado);
        assert_eq!(crate::prefs::load_layout(&path), Some(esperado));
    }
}

#[test]
fn effects_row_is_not_cycled_and_changes_nothing() {
    let mut app = App::new();
    let antes = app.ui_row_value(UiRow::Effects);
    assert_eq!(app.ui_row_cycle(UiRow::Effects, true), None);
    assert_eq!(app.ui_row_cycle(UiRow::Effects, false), None);
    assert_eq!(app.ui_row_value(UiRow::Effects), antes);
    assert!(app.logo_fx_on);
}

#[test]
fn spinner_row_rotates_and_persists_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_con_prefs(&dir);
    assert_eq!(app.spinner_choice, SpinnerChoice::Random);

    app.ui_row_cycle(UiRow::Spinner, true);
    assert_eq!(app.spinner_choice, SpinnerChoice::Crush);
    assert_eq!(
        SpinnerChoice::from_key(&crate::prefs::load_spinner(&path).unwrap()),
        SpinnerChoice::Crush
    );

    app.ui_row_cycle(UiRow::Spinner, false);
    app.ui_row_cycle(UiRow::Spinner, false);
    assert_eq!(
        app.spinner_choice,
        SPINNER_CHOICES[SPINNER_CHOICES.len() - 1]
    );
    assert_eq!(
        SpinnerChoice::from_key(&crate::prefs::load_spinner(&path).unwrap()),
        app.spinner_choice
    );
}

#[test]
fn toggle_rows_alternate_and_twice_returns_to_the_start() {
    let mut app = App::new();
    let mascot = app.mascot;
    let mouse = app.mouse_capture;
    let sidebar = app.sidebar_expanded;

    for row in [UiRow::Mascot, UiRow::Mouse, UiRow::Sidebar] {
        let inicio = app.ui_row_value(row);
        app.ui_row_cycle(row, true);
        assert_ne!(app.ui_row_value(row), inicio, "{row:?} alterna");
        app.ui_row_cycle(row, true);
        assert_eq!(app.ui_row_value(row), inicio, "{row:?} vuelve tras dos");
    }
    assert_eq!(app.mascot, mascot);
    assert_eq!(app.mouse_capture, mouse);
    assert_eq!(app.sidebar_expanded, sidebar);
}

#[test]
fn cycle_notice_names_the_new_value() {
    let mut app = App::new();
    app.set_layout(LayoutStyle::ContextRail);
    let aviso = app.ui_row_cycle(UiRow::Layout, true);
    assert_eq!(aviso.as_deref(), Some("layout: workspace"));

    app.set_style(StyleBundle::Classic);
    let aviso = app.ui_row_cycle(UiRow::Style, true);
    assert_eq!(aviso.as_deref(), Some("style: flat"));
}

#[test]
fn style_cycle_from_a_mixed_look_lands_on_a_bundle() {
    // Un look mixto no pertenece a ningún paquete: el ciclo termina en uno concreto.
    let mut app = App::new();
    app.theme.look.tool = ToolStyle::Inline;
    app.theme.look.chrome = ChromeStyle::Panels;
    app.theme.look.composer = loomcoding_cloth::widgets::ComposerStyle::Crush;
    assert_eq!(app.theme.look.bundle(), None);
    app.ui_row_cycle(UiRow::Style, true);
    assert!(app.theme.look.bundle().is_some());
}
