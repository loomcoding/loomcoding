//! Tests: theme.

use super::support::*;
use super::*;

#[test]
fn changing_theme_keeps_the_component_when_it_exists() {
    let mut show = Showcase::new();
    show.select(first_demo(&show, "Composer", "Batten"), false);
    show.set_theme_column(column_index("Crush"));
    assert_eq!(show.selected(), first_demo(&show, "Composer", "Crush"));
    assert_eq!(show.theme_preset(), ThemePreset::Crush);
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn changing_theme_selects_the_first_when_it_does_not() {
    let mut show = Showcase::new();
    // Plan card solo tiene demo de Crush: en Batten no existe.
    show.set_theme_column(column_index("Crush"));
    show.select(first_demo(&show, "Plan card", "Crush"), false);
    show.set_theme_column(column_index("Batten"));
    assert_eq!(show.selected(), show.filtered_indices()[0]);
    assert_ne!(show.component_at(show.selected()), Some("Plan card"));
    assert_eq!(show.theme_preset(), ThemePreset::Batten);
}

#[test]
fn left_right_cycle_the_theme_with_wrap() {
    let mut show = Showcase::new();
    assert_eq!(show.theme_column(), 0);
    press(&mut show, KeyCode::Left); // vuelta: Batten -> CodeLoom
    assert_eq!(show.theme_column(), matrix::COLUMNS.len() - 1);
    assert_eq!(show.theme_preset(), ThemePreset::CodeLoom);
    press(&mut show, KeyCode::Right); // vuelta: CodeLoom -> Batten
    assert_eq!(show.theme_column(), 0);
    press(&mut show, KeyCode::Right);
    assert_eq!(show.theme_column(), 1);
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn left_right_do_not_change_the_theme_in_demo_focus() {
    let mut show = Showcase::new();
    show.select(first_demo(&show, "Composer", "Batten"), true);
    assert_eq!(show.focus(), Focus::Demo);
    press(&mut show, KeyCode::Left);
    press(&mut show, KeyCode::Right);
    assert_eq!(show.theme_column(), 0);
    assert_eq!(show.focus(), Focus::Demo);
}

#[test]
fn f7_still_cycles_the_theme() {
    let mut show = Showcase::new();
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::F(7),
        KeyModifiers::NONE,
    );
    assert_eq!(show.theme_column(), 1);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::F(7),
        KeyModifiers::SHIFT,
    );
    assert_eq!(show.theme_column(), 0);
    // Con el foco en el demo, F7 también cambia de tema.
    press(&mut show, KeyCode::Tab);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::F(7),
        KeyModifiers::NONE,
    );
    assert_eq!(show.theme_column(), 1);
    assert_eq!(show.focus(), Focus::Demo);
}
