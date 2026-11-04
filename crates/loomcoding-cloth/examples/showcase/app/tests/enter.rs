//! Tests: enter.

use super::support::*;
use super::*;

#[test]
fn enter_on_own_opens_the_demo_with_that_theme() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Crush");
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.focus(), Focus::Demo);
    assert_eq!(show.selected(), first_demo(&show, "Composer", "Crush"));
    assert_eq!(show.theme_preset(), ThemePreset::Crush);
}

#[test]
fn enter_on_generic_opens_the_cloth_demo_with_the_column_theme() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Pawan");
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.focus(), Focus::Demo);
    assert_eq!(show.selected(), first_demo(&show, "Composer", "Cloth"));
    assert_eq!(show.theme_preset(), ThemePreset::Pawan);
}

#[test]
fn enter_on_missing_does_nothing() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    let pawan = column_index("Pawan");
    let component = component_where(&show, pawan, |cell| cell == Cell::Missing);
    place(&mut show, component, "Pawan");
    let before = (show.selected(), show.theme_preset());
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.focus(), Focus::Matrix);
    assert_eq!((show.selected(), show.theme_preset()), before);
    assert!(!show.matrix_return);
}

// ADAPTADO: el preset ya no es un campo (se deriva de la columna); el tema de partida se fija
// con `set_theme_column` en lugar de asignar `theme_preset`.
#[test]
fn enter_on_loomcoding_applies_its_palette() {
    let mut show = Showcase::new();
    show.set_theme_column(column_index("Pawan"));
    press(&mut show, KeyCode::Char('m'));
    let loomcoding = column_index("Loomcoding");
    let component = component_where(&show, loomcoding, |cell| matches!(cell, Cell::Own(_)));
    place(&mut show, component, "Loomcoding");
    press(&mut show, KeyCode::Enter);
    assert_eq!(show.focus(), Focus::Demo);
    assert_eq!(show.selected(), first_demo(&show, component, "Loomcoding"));
    assert_eq!(show.theme_preset(), ThemePreset::Loomcoding);
}

#[test]
fn esc_from_a_normal_demo_still_returns_to_the_index() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Tab);
    assert_eq!(show.focus(), Focus::Demo);
    press(&mut show, KeyCode::Esc);
    assert_eq!(show.focus(), Focus::Index);
}
