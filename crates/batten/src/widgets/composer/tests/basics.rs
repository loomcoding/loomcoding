//! Tests: basics.

use super::support::*;
use super::*;

#[test]
fn shows_title_and_text() {
    let mut c = Composer::new();
    c.push_char('h');
    c.push_char('i');
    let out = buffer(&c, 40, 3);
    assert!(out.contains("PROMPT"));
    assert!(out.contains("hi"));
}

#[test]
fn shows_model_shortcuts_and_live_count() {
    let mut c = Composer::new();
    c.insert_str("hi");
    let mut term = Terminal::new(TestBackend::new(72, 3)).unwrap();
    term.draw(|f| render_focused_with_model(f, f.area(), &c, true, "Qwen3-Coder", &Theme::test()))
        .unwrap();
    let out = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(out.contains("Qwen3-Coder"));
    assert!(out.contains("Ctrl+J newline"));
    assert!(out.contains("1 line  ·  2 chars"));
}

#[test]
fn empty_composer_does_not_panic() {
    let _ = buffer(&Composer::new(), 20, 3);
    let _ = buffer(&Composer::new(), 0, 0);
}

#[test]
fn places_the_cursor_at_the_end() {
    let mut c = Composer::new();
    c.insert_str("hi");
    let backend = TestBackend::new(20, 3);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, f.area(), &c, &Theme::test()))
        .unwrap();
    let pos = term.backend_mut().get_cursor_position().unwrap();
    assert_eq!((pos.x, pos.y), (5, 1));
}

#[test]
fn places_the_cursor_on_the_second_line() {
    let mut c = Composer::new();
    c.insert_str("a\nb");
    let backend = TestBackend::new(20, 4);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, f.area(), &c, &Theme::test()))
        .unwrap();
    let pos = term.backend_mut().get_cursor_position().unwrap();
    assert_eq!((pos.x, pos.y), (4, 2));
}

#[test]
fn wraps_long_lines_and_keeps_the_cursor_visible() {
    let mut c = Composer::new();
    c.set_text("abcdefghijklmnopq");
    let mut term = Terminal::new(TestBackend::new(8, 3)).unwrap();
    term.draw(|f| render(f, f.area(), &c, &Theme::test()))
        .unwrap();
    let pos = term.backend_mut().get_cursor_position().unwrap();
    assert_eq!(pos.x, 4);
    let visible = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(visible.contains("q"));
    assert!(!visible.contains("abcdef"));
}

#[test]
fn horizontal_view_keeps_wide_graphemes_whole_and_shows_text_after_cursor() {
    assert_eq!(line_view("abcd👩‍💻efgh", Some(9), 6), ("efgh".into(), 4));
    assert_eq!(line_view("abcdefghijkl", Some(6), 6), ("bcdefg".into(), 5));
}

#[test]
fn border_shows_focus() {
    use ratatui::style::Color;
    let c = Composer::new();
    for (focused, color) in [
        (true, Color::Rgb(0, 212, 255)),
        (false, Color::Indexed(240)),
    ] {
        let mut term = Terminal::new(TestBackend::new(20, 3)).unwrap();
        term.draw(|f| render_focused(f, f.area(), &c, focused, &Theme::test()))
            .unwrap();
        assert_eq!(term.backend().buffer()[(0, 0)].fg, color);
    }
}

#[test]
fn tiny_area_does_not_panic() {
    let c = Composer::new();
    let _ = buffer(&c, 0, 0);
    let _ = buffer(&c, 1, 1);
}
