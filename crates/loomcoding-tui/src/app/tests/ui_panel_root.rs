//! Pruebas de la raíz del panel `/ui` (la ventana `/temas` con sus nueve filas): niveles,
//! teclas y avisos. Viven aparte para que `theme_and_settings.rs` no siga creciendo.

use super::*;
use crate::app::ui_panel::UiRow;

/// Posición de una fila en la raíz, según el orden del panel.
fn fila(row: UiRow) -> usize {
    UiRow::ALL
        .iter()
        .position(|r| *r == row)
        .expect("la fila pertenece al panel")
}

/// App con la ventana abierta y `ui.toml` en un directorio temporal, para no tocar el real.
fn abierta_con_prefs(dir: &tempfile::TempDir) -> App {
    let mut app = App::new();
    app.set_prefs_path(dir.path().join("ui.toml"));
    app.open_temas();
    app
}

#[test]
fn open_temas_starts_on_root_with_nine_rows_and_interface_title() {
    let mut app = App::new();
    app.open_temas();
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_eq!(TemasLevel::Root.rows(), 9);
    assert_eq!(TemasLevel::Root.title(), "Interfaz");
}

#[test]
fn root_selection_is_bounded_by_the_nine_rows() {
    let mut app = App::new();
    app.open_temas();
    app.temas_move(-1);
    assert_eq!(app.temas_selected, 0);
    app.temas_move(100);
    assert_eq!(app.temas_selected, 8);
    app.temas_move(1);
    assert_eq!(app.temas_selected, 8);
}

#[test]
fn right_and_left_change_layout_without_entering_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = abierta_con_prefs(&dir);
    app.temas_selected = fila(UiRow::Layout);
    let antes = app.ui_row_value(UiRow::Layout);
    app.temas_adjust(true);
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_ne!(app.ui_row_value(UiRow::Layout), antes);
    let aviso = app.slash_feedback.clone().unwrap_or_default();
    assert!(aviso.starts_with("layout: "), "{aviso}");
    let guardado = std::fs::read_to_string(dir.path().join("ui.toml")).unwrap();
    assert!(guardado.contains("layout"), "{guardado}");
    app.temas_adjust(false);
    assert_eq!(app.ui_row_value(UiRow::Layout), antes);
}

#[test]
fn right_and_left_alternate_style_without_entering() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = abierta_con_prefs(&dir);
    app.temas_selected = fila(UiRow::Style);
    let antes = app.ui_row_value(UiRow::Style);
    app.temas_adjust(true);
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_ne!(app.ui_row_value(UiRow::Style), antes);
    app.temas_adjust(false);
    assert_eq!(app.ui_row_value(UiRow::Style), antes);
}

#[test]
fn right_toggles_mascot_without_entering() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = abierta_con_prefs(&dir);
    app.temas_selected = fila(UiRow::Mascot);
    let antes = app.mascot;
    app.temas_adjust(true);
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_ne!(app.mascot, antes);
    assert!(app.slash_feedback.is_some());
}

#[test]
fn enter_on_submenu_rows_opens_their_level() {
    let casos = [
        (UiRow::Effects, TemasLevel::Effects),
        (UiRow::Theme, TemasLevel::Theme),
        (UiRow::Spinner, TemasLevel::Spinner),
    ];
    for (row, nivel) in casos {
        let dir = tempfile::tempdir().unwrap();
        let mut app = abierta_con_prefs(&dir);
        app.temas_selected = fila(row);
        app.temas_enter();
        assert_eq!(app.temas_level, nivel, "{row:?}");
        assert_eq!(app.temas_selected, 0, "{row:?}");
    }
}

#[test]
fn enter_on_a_plain_row_cycles_forward_without_entering() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = abierta_con_prefs(&dir);
    app.temas_selected = fila(UiRow::Style);
    let antes = app.ui_row_value(UiRow::Style);
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_ne!(app.ui_row_value(UiRow::Style), antes);
}

#[test]
fn esc_climbs_back_to_the_row_that_opened_the_level() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = abierta_con_prefs(&dir);

    app.temas_selected = fila(UiRow::Effects);
    app.temas_enter();
    assert_eq!(app.temas_level, TemasLevel::Effects);
    app.temas_back();
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_eq!(app.temas_selected, fila(UiRow::Effects));

    app.temas_selected = fila(UiRow::Theme);
    app.temas_enter();
    app.temas_back();
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_eq!(app.temas_selected, fila(UiRow::Theme));

    app.temas_selected = fila(UiRow::Spinner);
    app.temas_enter();
    app.temas_back();
    assert_eq!(app.temas_level, TemasLevel::Root);
    assert_eq!(app.temas_selected, fila(UiRow::Spinner));

    app.temas_back();
    assert!(!app.temas_open);
}

#[test]
fn failed_save_keeps_the_notice_left_by_the_setter() {
    let dir = tempfile::tempdir().unwrap();
    // Un fichero normal como «directorio» padre: la escritura siempre falla.
    let bloqueo = dir.path().join("bloqueo");
    std::fs::write(&bloqueo, "x").unwrap();
    let mut app = App::new();
    app.set_prefs_path(bloqueo.join("ui.toml"));
    app.open_temas();
    app.temas_selected = fila(UiRow::Layout);
    app.temas_adjust(true);
    let aviso = app.slash_feedback.clone().unwrap_or_default();
    assert!(aviso.starts_with("Layout not saved"), "{aviso}");
}
