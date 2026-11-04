//! Tests: classic.

use super::*;

#[test]
fn classic_height_grows_to_three_lines_then_stays_capped() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("a");
    assert_eq!(height_for_look(&theme, &c, 40, 20), 3);
    c.set_text("a\nb");
    assert_eq!(height_for_look(&theme, &c, 40, 20), 4);
    c.set_text("a\nb\nc");
    assert_eq!(height_for_look(&theme, &c, 40, 20), 5);
    c.set_text("a\nb\nc\nd");
    assert_eq!(height_for_look(&theme, &c, 40, 20), 5);
    assert_eq!(height_for_look(&theme, &c, 40, 4), 4);
}

#[test]
fn classic_wraps_long_input_and_moves_cursor_to_next_visual_row() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("abcdefghijkl");
    assert_eq!(height_for_look(&theme, &c, 14, 20), 4);

    let mut term = Terminal::new(TestBackend::new(14, 5)).unwrap();
    term.draw(|f| render_focused(f, f.area(), &c, true, &theme))
        .unwrap();
    let screen = term
        .backend()
        .buffer()
        .content
        .chunks(14)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("abcdefghij"));
    assert!(screen.contains("kl"));
    let cursor = term.backend_mut().get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (5, 2));
}

#[test]
fn classic_keeps_the_last_three_lines_visible_after_overflow() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("one\ntwo\nthree\nfour\nfive");
    let mut term = Terminal::new(TestBackend::new(24, 5)).unwrap();
    term.draw(|f| render_focused(f, f.area(), &c, true, &theme))
        .unwrap();
    let screen = term
        .backend()
        .buffer()
        .content
        .chunks(24)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("three"));
    assert!(screen.contains("four"));
    assert!(screen.contains("five"));
    assert!(!screen.contains("one"));
    assert_eq!(term.backend_mut().get_cursor_position().unwrap().y, 3);
}
