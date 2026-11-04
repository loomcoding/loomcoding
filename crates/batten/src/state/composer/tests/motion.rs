//! Tests: motion.

use super::*;

#[test]
fn home_and_end_go_to_line_bounds() {
    let mut c = Composer::new();
    c.insert_str("uno\ndos");
    c.move_home();
    assert_eq!(c.cursor(), 4);
    c.move_end();
    assert_eq!(c.cursor(), 7);
}

#[test]
fn up_and_down_keep_the_column() {
    let mut c = Composer::new();
    c.insert_str("aaaa\nb");
    c.move_end(); // final de "b" (indice 6)
    c.move_up();
    assert_eq!(c.cursor_row_col(), (0, 1));
    c.move_down();
    assert_eq!(c.cursor_row_col(), (1, 1));
}

#[test]
fn cursor_row_col_counts_display_width() {
    let mut c = Composer::new();
    c.insert_str("星x");
    assert_eq!(c.cursor_row_col(), (0, 3));
}

#[test]
fn lines_counts_logical_lines() {
    let mut c = Composer::new();
    assert_eq!(c.lines(), 1);
    c.insert_str("a\nb\nc");
    assert_eq!(c.lines(), 3);
}

#[test]
fn cursor_and_deletion_keep_combining_and_emoji_graphemes_intact() {
    let mut c = Composer::new();
    c.insert_str("e\u{301}👩‍💻");
    assert_eq!(c.cursor(), 2);
    c.move_left();
    c.backspace();
    assert_eq!(c.text(), "👩‍💻");
    assert_eq!(c.cursor_row_col(), (0, 0));
}

#[test]
fn vertical_cursor_keeps_display_column_with_wide_graphemes() {
    let mut c = Composer::new();
    c.set_text("星x\n1234");
    c.move_up();
    assert_eq!(c.cursor_row_col(), (0, 3));
}
