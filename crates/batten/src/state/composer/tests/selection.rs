//! Tests: selection.

use super::support::*;
use super::*;

#[test]
fn extend_right_selects_graphemes() {
    let mut c = Composer::new();
    c.insert_str(&format!("ae\u{301}{FAMILIA}b"));
    c.set_cursor(0);
    c.extend_right();
    c.extend_right();
    c.extend_right();
    assert_eq!(c.anchor(), Some(0));
    assert_eq!(c.cursor(), 3);
    assert_eq!(c.selection(), Some((0, 3)));
    let esperado = format!("ae\u{301}{FAMILIA}");
    assert_eq!(c.selected_text(), Some(esperado.as_str()));
}

#[test]
fn extend_then_move_clears_selection() {
    let mut c = Composer::new();
    c.insert_str("hola");
    c.set_cursor(0);
    c.extend_right();
    c.extend_right();
    assert_eq!(c.selection(), Some((0, 2)));
    c.move_right();
    assert_eq!(c.anchor(), None);
    assert_eq!(c.selection(), None);
    assert_eq!(c.cursor(), 3);
}

#[test]
fn selection_is_ordered_regardless_of_direction() {
    let mut c = Composer::new();
    c.insert_str("abcd");
    c.extend_left();
    c.extend_left();
    assert_eq!(c.anchor(), Some(4));
    assert_eq!(c.selection(), Some((2, 4)));
    assert_eq!(c.selected_text(), Some("cd"));
    c.select_range(1, 3);
    assert_eq!(c.selection(), Some((1, 3)));
    c.select_range(3, 1);
    assert_eq!(c.selection(), Some((1, 3)));
    assert_eq!(c.selected_text(), Some("bc"));
    c.set_cursor(2);
    c.extend_home();
    assert_eq!(c.selection(), Some((0, 2)));
    c.set_cursor(2);
    c.extend_end();
    assert_eq!(c.selection(), Some((2, 4)));
}

#[test]
fn typing_replaces_the_selection() {
    let mut c = Composer::new();
    c.insert_str("hola mundo");
    c.select_range(0, 4);
    c.insert_char('X');
    assert_eq!(c.text(), "X mundo");
    assert_eq!(c.cursor(), 1);
    assert_eq!(c.selection(), None);
}

#[test]
fn backspace_and_delete_remove_only_the_selection() {
    let mut c = Composer::new();
    c.set_text("abcdef");
    c.select_range(1, 3);
    c.backspace();
    assert_eq!(c.text(), "adef");
    assert_eq!(c.cursor(), 1);
    assert_eq!(c.selection(), None);

    c.set_text("abcdef");
    c.select_range(2, 4);
    c.delete();
    assert_eq!(c.text(), "abef");
    assert_eq!(c.cursor(), 2);

    c.set_text("abcdef");
    c.select_range(4, 1);
    c.backspace();
    assert_eq!(c.text(), "aef");
    assert_eq!(c.cursor(), 1);
}

#[test]
fn paste_insert_str_replaces_the_selection() {
    let mut c = Composer::new();
    c.insert_str("hola mundo");
    c.select_range(5, 10);
    c.insert_str("tierra\r\nnueva");
    assert_eq!(c.text(), "hola tierra\nnueva");
    assert_eq!(c.cursor(), 17);
    assert_eq!(c.selection(), None);
}

#[test]
fn newline_replaces_the_selection() {
    let mut c = Composer::new();
    c.insert_str("ab cd");
    c.select_range(2, 3);
    c.newline();
    assert_eq!(c.text(), "ab\ncd");
    assert_eq!(c.cursor(), 3);
}

#[test]
fn set_text_and_submit_clear_the_selection() {
    let mut c = Composer::new();
    c.insert_str("abc");
    c.select_range(0, 2);
    c.set_text("xyz");
    assert_eq!(c.anchor(), None);
    assert_eq!(c.selection(), None);
    assert_eq!(c.text(), "xyz");
    c.select_range(0, 2);
    assert_eq!(c.submit(), Some("xyz".to_string()));
    assert_eq!(c.anchor(), None);
    assert_eq!(c.selection(), None);
}

#[test]
fn select_range_clamps_out_of_range_positions() {
    let mut c = Composer::new();
    c.insert_str("abc");
    c.select_range(1, 99);
    assert_eq!(c.selection(), Some((1, 3)));
    assert_eq!(c.cursor(), 3);
    c.select_range(99, 99);
    assert_eq!(c.selection(), None);
    assert_eq!(c.cursor(), 3);
    c.select_range(50, 1);
    assert_eq!(c.anchor(), Some(3));
    assert_eq!(c.selection(), Some((1, 3)));
}

#[test]
fn selection_never_splits_a_grapheme() {
    let mut c = Composer::new();
    let texto = format!("e\u{301}{FAMILIA}\u{661F}x");
    c.set_text(&texto);
    c.set_cursor(0);
    c.extend_right();
    assert_eq!(c.selection(), Some((0, 1)));
    assert_eq!(c.selected_text(), Some("e\u{301}"));
    c.extend_right();
    c.extend_right();
    assert_eq!(c.selection(), Some((0, 3)));
    let esperado = format!("e\u{301}{FAMILIA}\u{661F}");
    assert_eq!(c.selected_text(), Some(esperado.as_str()));
    c.select_range(2, 3);
    assert_eq!(c.selected_text(), Some("\u{661F}"));
}

#[test]
fn selected_text_spans_multiple_lines() {
    let mut c = Composer::new();
    c.set_text("uno\ndos\ntres");
    c.select_range(2, 9);
    assert_eq!(c.selected_text(), Some("o\ndos\nt"));
    c.set_cursor(2);
    c.extend_down();
    assert_eq!(c.selection(), Some((2, 6)));
    assert_eq!(c.selected_text(), Some("o\ndo"));
}

#[test]
fn selection_bytes_matches_selection() {
    let mut c = Composer::new();
    let texto = format!("e\u{301}{FAMILIA}\u{661F}");
    c.set_text(&texto);
    c.select_range(1, 3);
    assert_eq!(c.selection(), Some((1, 3)));
    assert_eq!(c.selection_bytes(), Some((3, texto.len())));
    let esperado = format!("{FAMILIA}\u{661F}");
    assert_eq!(c.selected_text(), Some(esperado.as_str()));
}

#[test]
fn extend_up_down_follow_preferred_column() {
    let mut c = Composer::new();
    c.set_text("abcdef\nxy\nabcdef");
    c.set_cursor(3);
    c.extend_down();
    assert_eq!(c.selection(), Some((3, 9)));
    assert_eq!(c.selected_text(), Some("def\nxy"));
    // La columna preferida (3) se conserva al pasar por una línea corta.
    c.extend_down();
    assert_eq!(c.selection(), Some((3, 13)));
    assert_eq!(c.selected_text(), Some("def\nxy\nabc"));
}

#[test]
fn select_all_selects_everything() {
    let mut c = Composer::new();
    c.select_all();
    assert_eq!(c.anchor(), None);
    assert_eq!(c.selection(), None);
    c.set_text("hola\nmundo");
    c.select_all();
    assert_eq!(c.anchor(), Some(0));
    assert_eq!(c.selection(), Some((0, 10)));
    assert_eq!(c.selected_text(), Some("hola\nmundo"));
    c.clear_selection();
    assert_eq!(c.selection(), None);
    assert_eq!(c.cursor(), 10);
}

#[test]
fn no_selection_leaves_existing_behaviour_untouched() {
    let mut c = Composer::new();
    c.push_char('a');
    c.insert_str("bc");
    c.move_left();
    c.insert_char('X');
    c.newline();
    assert_eq!(c.text(), "abX\nc");
    c.move_up();
    c.move_end();
    c.move_down();
    c.move_left();
    c.move_right();
    c.move_home();
    c.move_end();
    c.backspace();
    c.backspace();
    c.delete();
    c.move_left();
    c.delete();
    assert_eq!(c.text(), "ab");
    assert_eq!(c.cursor(), 2);
    c.insert_str("\r\nz");
    c.move_home();
    c.move_up();
    c.move_down();
    assert_eq!(c.text(), "ab\nz");
    assert_eq!(c.cursor(), 3);
    assert_eq!(c.cursor_row_col(), (1, 0));
    assert_eq!(c.selection(), None);
    assert_eq!(c.submit(), Some("ab\nz".to_string()));
}
