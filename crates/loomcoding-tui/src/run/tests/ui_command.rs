//! Pruebas de teclado de `/ui` y de los alias ocultos: lo que pasa al pulsar Enter.
//!
//! Cabe señalar que cada alias se teclea entero, aunque el popup ya no lo liste; su
//! comportamiento debe ser el de siempre.

use super::*;
use loomcoding_cloth::widgets::{LayoutStyle, StyleBundle, ThemePreset};

/// Teclea `text` y pulsa Enter; devuelve el estado del compositor tras la ejecución.
fn ejecutar(app: &mut App, text: &str) {
    app.composer.set_text(text);
    assert_eq!(
        handle_key(app, KeyCode::Enter, KeyModifiers::NONE),
        None,
        "{text} no debe enviarse como prompt"
    );
}

/// Texto completo de la pantalla pintada a `w`×`h`, una línea por fila.
fn pantalla(app: &mut App, w: u16, h: u16) -> String {
    let buf = paint_sized(app, w, h);
    let area = buf.area;
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn typed_alias_theme_still_cycles_the_theme() {
    let mut app = App::new();
    let siguiente = ThemePreset::ALL[(ThemePreset::ALL
        .iter()
        .position(|p| *p == app.theme_preset)
        .unwrap()
        + 1)
        % ThemePreset::ALL.len()];
    ejecutar(&mut app, "/theme");
    assert_eq!(app.theme_preset, siguiente);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some(format!("Theme: {}", siguiente.name()).as_str())
    );
    assert!(app.composer.is_empty());
}

#[test]
fn typed_alias_layout_still_rotates_the_layout() {
    let mut app = App::new();
    ejecutar(&mut app, "/layout");
    assert_eq!(app.theme.look.layout, LayoutStyle::ContextRail);
    assert_eq!(app.slash_feedback.as_deref(), Some("layout: rail"));
}

#[test]
fn typed_alias_temas_opens_the_panel() {
    let mut app = App::new();
    ejecutar(&mut app, "/temas");
    assert!(app.temas_open);
    assert!(app.composer.is_empty());
}

#[test]
fn typed_aliases_crush_and_classic_set_their_presets() {
    use loomcoding_cloth::widgets::Look;
    let mut app = App::new();
    ejecutar(&mut app, "/crush");
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert_eq!(app.theme.look, Look::crush());
    ejecutar(&mut app, "/classic");
    assert_eq!(app.theme_preset, ThemePreset::Cloth);
    assert_eq!(app.theme.look, Look::classic());
}

#[test]
fn typed_alias_mascot_toggles() {
    let mut app = App::new();
    assert!(app.mascot);
    ejecutar(&mut app, "/mascot");
    assert!(!app.mascot);
    assert_eq!(app.slash_feedback.as_deref(), Some("mascot: off"));
}

#[test]
fn typed_alias_mouse_toggles() {
    let mut app = App::new();
    assert!(app.mouse_capture);
    ejecutar(&mut app, "/mouse");
    assert!(!app.mouse_capture);
    assert!(app.mouse_capture_dirty);
}

#[test]
fn typed_alias_sidebar_toggles() {
    let mut app = App::new();
    let antes = app.sidebar_expanded;
    ejecutar(&mut app, "/sidebar");
    assert_ne!(app.sidebar_expanded, antes);
}

#[test]
fn ui_without_arguments_opens_the_panel_by_enter() {
    let mut app = App::new();
    ejecutar(&mut app, "/ui");
    assert!(app.temas_open);
    assert_eq!(app.temas_level, crate::app::TemasLevel::Root);
}

#[test]
fn ui_with_arguments_acts_by_enter_and_clears_the_composer() {
    let mut app = App::new();
    ejecutar(&mut app, "/ui layout rail");
    assert_eq!(app.theme.look.layout, LayoutStyle::ContextRail);
    assert_eq!(app.slash_feedback.as_deref(), Some("layout: rail"));
    assert!(app.composer.is_empty());

    ejecutar(&mut app, "/ui style flat");
    assert_eq!(app.theme.look.bundle(), Some(StyleBundle::Flat));
    assert!(app.composer.is_empty());

    ejecutar(&mut app, "/ui theme crush");
    assert_eq!(app.theme_preset, ThemePreset::Crush);
    assert!(app.composer.is_empty());

    ejecutar(&mut app, "/ui mascot off");
    assert!(!app.mascot);
    assert_eq!(app.slash_feedback.as_deref(), Some("mascot: off"));
}

#[test]
fn ui_effects_and_spinner_by_enter() {
    let mut app = App::new();
    ejecutar(&mut app, "/ui effects");
    assert!(app.temas_open);
    assert_eq!(app.temas_level, crate::app::TemasLevel::Effects);

    let mut app = App::new();
    let antes = app.ui_row_value(crate::app::UiRow::Spinner);
    ejecutar(&mut app, "/ui spinner");
    assert_ne!(app.ui_row_value(crate::app::UiRow::Spinner), antes);
    assert!(app.composer.is_empty());
}

#[test]
fn ui_errors_keep_the_typed_text_and_report_the_options() {
    let casos = [
        ("/ui theme neon", "Unknown theme: neon. Options: "),
        (
            "/ui mascot maybe",
            "Unknown mascot: maybe. Options: on, off",
        ),
        ("/ui effects x", "Use the panel: /ui effects"),
        (
            "/ui x",
            "Unknown row: x. Rows: theme, style, layout, effects, spinner, mascot, sidebar, mouse",
        ),
    ];
    for (texto, esperado) in casos {
        let mut app = App::new();
        ejecutar(&mut app, texto);
        assert_eq!(app.composer.text(), texto, "{texto}");
        let aviso = app.slash_feedback.as_deref().unwrap_or_default();
        assert!(aviso.starts_with(esperado), "{texto}: {aviso:?}");
    }
}

#[test]
fn help_names_ui_and_every_alias_inside_the_80x24_box() {
    let mut app = App::new();
    app.help_open = true;
    let content = pantalla(&mut app, 80, 24);
    assert!(content.contains("/ui"), "falta /ui: {content}");
    for alias in [
        "/theme", "/layout", "/crush", "/classic", "/mascot", "/sidebar", "/mouse",
    ] {
        assert!(content.contains(alias), "falta el alias {alias}: {content}");
    }
    assert!(
        content.contains("Ctrl+C/Q") && content.contains("Quit"),
        "la ayuda se corta a 80×24: {content}"
    );
}
