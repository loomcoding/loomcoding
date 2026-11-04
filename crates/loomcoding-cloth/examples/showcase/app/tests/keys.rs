//! Tests: keys.

use super::support::*;
use super::*;

#[test]
fn navigation_clamps_and_resets_focus() {
    let mut show = dummy_show(5);
    assert_eq!(show.selected(), 0);
    show.prev();
    assert_eq!(show.selected(), 0); // clamp arriba
    show.next();
    show.next();
    assert_eq!(show.selected(), 2);
    show.focus_demo_for_test();
    assert_eq!(show.focus(), Focus::Demo);
    show.next();
    assert_eq!(show.focus(), Focus::Index); // navegar resetea el foco
    for _ in 0..100 {
        show.next();
    }
    assert_eq!(show.selected(), show.len() - 1); // clamp abajo
}

#[test]
fn tab_toggles_focus() {
    let mut show = Showcase::new();
    assert_eq!(show.focus(), Focus::Index);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Tab,
        KeyModifiers::NONE,
    );
    assert_eq!(show.focus(), Focus::Demo);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Tab,
        KeyModifiers::NONE,
    );
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn release_is_ignored() {
    let mut show = Showcase::new();
    let a = route(
        &mut show,
        KeyEventKind::Release,
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    );
    assert_eq!(a, Action::None);
    assert!(!show.should_quit());
}

#[test]
fn ctrl_c_quits_always() {
    let mut show = Showcase::new();
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Tab,
        KeyModifiers::NONE,
    ); // foco demo
    let a = route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    );
    assert_eq!(a, Action::Quit);
    assert!(show.should_quit());
}

#[test]
fn q_quits_from_index_only() {
    let mut show = Showcase::new();
    let a = route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    );
    assert_eq!(a, Action::Quit);
    assert!(show.should_quit());

    // Con foco en el demo, 'q' se reenvía al demo (no sale).
    let mut show = Showcase::new();
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Tab,
        KeyModifiers::NONE,
    );
    let a = route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Char('q'),
        KeyModifiers::NONE,
    );
    assert_eq!(a, Action::None);
    assert!(!show.should_quit());
}

#[test]
fn arrows_move_selection_in_index_focus() {
    let mut show = dummy_show(3);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Down,
        KeyModifiers::NONE,
    );
    assert_eq!(show.selected(), 1);
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Up,
        KeyModifiers::NONE,
    );
    assert_eq!(show.selected(), 0);
}

#[test]
fn route_passes_modifiers_to_the_demo() {
    let seen = std::rc::Rc::new(std::cell::Cell::new(None));
    let mut show = Showcase::with_demos(vec![Box::new(Rec(seen.clone()))]);
    show.focus_demo_for_test();
    route(
        &mut show,
        KeyEventKind::Press,
        KeyCode::Char('j'),
        KeyModifiers::CONTROL,
    );
    assert_eq!(seen.get(), Some(KeyModifiers::CONTROL));
}

#[test]
fn m_is_a_character_in_the_text_filter() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('/'));
    press(&mut show, KeyCode::Char('m'));
    assert_eq!(show.focus(), Focus::TextFilter);
    assert_eq!(show.query(), "m");
}

#[test]
fn cursor_stops_at_the_edges() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    for _ in 0..100 {
        press(&mut show, KeyCode::Up);
        press(&mut show, KeyCode::Left);
    }
    assert_eq!(show.matrix_cursor(), (0, 0));
    for _ in 0..1000 {
        press(&mut show, KeyCode::Char('j'));
        press(&mut show, KeyCode::Char('l'));
    }
    let last_row = show.matrix().rows.len() - 1;
    assert_eq!(show.matrix_cursor(), (last_row, matrix::COLUMNS.len() - 1));
    assert_eq!(show.focus(), Focus::Matrix);
}
