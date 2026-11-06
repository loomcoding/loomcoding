//! Pruebas de dibujo de la raíz del panel `/ui` (ventana `/temas`): etiquetas, valores,
//! indicadores y terminales pequeños.

use super::*;
use crate::app::UiRow;

/// Renderiza y devuelve el contenido del terminal línea a línea.
fn lineas_de(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let term = terminal_of(app, w, h);
    let buf = term.backend().buffer();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect()
}

#[test]
fn root_draws_the_eight_labels_and_the_current_values() {
    let mut app = App::new();
    app.open_temas();
    let texto = buffer_of(&mut app, 80, 24);
    for etiqueta in [
        "Interfaz",
        "Tema",
        "Estilo",
        "Distribución",
        "Efectos",
        "Spinner",
        "Muñeco",
        "Menú lateral",
        "Ratón",
    ] {
        assert!(texto.contains(etiqueta), "falta «{etiqueta}»");
    }
    assert!(texto.contains(app.theme.look.layout.name()));
    assert!(texto.contains(app.theme_preset.name()));
}

#[test]
fn root_marks_cyclable_rows_with_both_arrows_and_effects_with_one() {
    let mut app = App::new();
    app.open_temas();
    let lineas = lineas_de(&mut app, 80, 24);
    let de = |etiqueta: &str| {
        lineas
            .iter()
            .find(|l| l.contains(etiqueta))
            .unwrap_or_else(|| panic!("falta «{etiqueta}»"))
            .clone()
    };
    let distribucion = de(UiRow::Layout.label());
    assert!(
        distribucion.contains("‹ ") && distribucion.contains(" ›"),
        "{distribucion}"
    );
    let efectos = de(UiRow::Effects.label());
    assert!(efectos.contains('›') && !efectos.contains('‹'), "{efectos}");
}

#[test]
fn root_shows_all_eight_rows_in_a_twelve_line_terminal() {
    let mut app = App::new();
    app.open_temas();
    let lineas = lineas_de(&mut app, 80, 12);
    for row in UiRow::ALL {
        assert!(
            lineas.iter().any(|l| l.contains(row.label())),
            "falta «{}»",
            row.label()
        );
    }
}

#[test]
fn help_describes_the_interface_panel() {
    let mut app = App::new();
    app.help_open = true;
    let texto = buffer_of(&mut app, 80, 24);
    assert!(texto.contains("Interface panel"));
}

#[test]
fn tiny_and_empty_terminals_do_not_panic_with_the_root_open() {
    for (w, h) in [(20, 5), (0, 0), (1, 1), (80, 0), (0, 24), (44, 9)] {
        let mut app = App::new();
        app.open_temas();
        let _ = buffer_of(&mut app, w, h);
    }
}
