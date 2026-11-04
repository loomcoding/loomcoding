//! Tests: editing.

use super::*;

#[test]
fn types_and_submits() {
    let mut c = Composer::new();
    c.push_char('h');
    c.push_char('i');
    assert_eq!(c.text(), "hi");
    assert_eq!(c.submit(), Some("hi".to_string()));
    assert!(c.is_empty());
    assert_eq!(c.submit(), None);
}

#[test]
fn whitespace_only_is_not_submitted() {
    let mut c = Composer::new();
    c.push_char(' ');
    c.newline();
    assert_eq!(c.submit(), None);
    assert_eq!(c.text(), "");
}

#[test]
fn backspace_removes_last_char() {
    let mut c = Composer::new();
    c.push_char('a');
    c.push_char('b');
    c.backspace();
    assert_eq!(c.text(), "a");
    c.backspace();
    c.backspace();
    assert!(c.is_empty());
}

#[test]
fn newline_inserts_newline() {
    let mut c = Composer::new();
    c.push_char('a');
    c.newline();
    c.push_char('b');
    assert_eq!(c.text(), "a\nb");
}

#[test]
fn inserts_in_the_middle() {
    let mut c = Composer::new();
    c.insert_str("hl");
    c.move_left();
    c.insert_char('e');
    assert_eq!(c.text(), "hel");
    assert_eq!(c.cursor(), 2);
}

#[test]
fn backspace_and_delete_around_the_cursor() {
    let mut c = Composer::new();
    c.insert_str("abc");
    c.move_left();
    c.move_left(); // cursor en 1
    c.backspace();
    assert_eq!(c.text(), "bc");
    c.delete();
    assert_eq!(c.text(), "c");
}

#[test]
fn handles_accented_input_without_corrupting() {
    let mut c = Composer::new();
    c.insert_str("áé");
    c.move_left();
    c.insert_char('x');
    assert_eq!(c.text(), "áxé");
}

#[test]
fn set_text_puts_the_cursor_at_the_end() {
    let mut c = Composer::new();
    c.insert_str("old");
    c.set_text("new\nline");
    assert_eq!(c.text(), "new\nline");
    assert_eq!(c.cursor(), 8);
}

#[test]
fn normalizes_windows_line_endings_before_editing() {
    let mut c = Composer::new();
    c.insert_str("first\r\nsecond");
    assert_eq!(c.text(), "first\nsecond");
    c.set_text("first\r\nsecond");
    assert_eq!(c.text(), "first\nsecond");
    c.set_text("\r\n");
    assert_eq!(c.text(), "\n");
    c.move_end();
    c.move_up();
    c.move_down();
    c.backspace();
    assert!(c.is_empty());
}
