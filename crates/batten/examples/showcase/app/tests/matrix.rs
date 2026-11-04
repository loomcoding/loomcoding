//! Tests: matrix.

use super::support::*;
use super::*;

#[test]
fn m_opens_the_matrix_and_esc_closes_it() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    assert_eq!(show.focus(), Focus::Matrix);
    press(&mut show, KeyCode::Esc);
    assert_eq!(show.focus(), Focus::Index);
    press(&mut show, KeyCode::Char('m'));
    press(&mut show, KeyCode::Char('m'));
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn esc_from_a_demo_opened_from_the_matrix_returns_to_it() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Crush");
    press(&mut show, KeyCode::Enter);
    press(&mut show, KeyCode::Esc);
    assert_eq!(show.focus(), Focus::Matrix);
    assert!(!show.matrix_return);
}

#[test]
fn matrix_return_does_not_stick_after_leaving_the_demo_by_other_keys() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Crush");
    press(&mut show, KeyCode::Enter);
    assert!(show.matrix_return);
    // Tab saca del demo al índice: la marca se limpia.
    press(&mut show, KeyCode::Tab);
    assert_eq!(show.focus(), Focus::Index);
    assert!(!show.matrix_return);
    // Un demo abierto de forma normal no vuelve a la matriz con Esc.
    press(&mut show, KeyCode::Tab);
    press(&mut show, KeyCode::Esc);
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn matrix_enter_switches_theme_and_selects_the_example() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Pawan");
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.focus(), Focus::Demo);
    assert_eq!(show.theme_column(), column_index("Pawan"));
    assert_eq!(show.selected(), first_demo(&show, "Composer", "Batten"));
    assert!(show.matrix_return);
    assert!(show
        .view()
        .entries
        .iter()
        .any(|entry| entry.component == "Composer" && entry.generic));
}

/// Un filtro de texto que ocultaría el ejemplo abierto desde la matriz se limpia: si no, el
/// índice quedaría sin nada seleccionado.
#[test]
fn matrix_enter_clears_a_filter_that_would_hide_the_example() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('/'));
    for c in "zzzz".chars() {
        press(&mut show, KeyCode::Char(c));
    }
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.query(), "zzzz", "precondición: hay un filtro activo");
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Pawan");
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.query(), "");
    let selected = show.selected();
    assert!(
        show.filtered_indices().contains(&selected),
        "el ejemplo abierto debe verse en el índice"
    );
}
