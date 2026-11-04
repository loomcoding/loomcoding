//! Tests: crush.

use super::support::*;
use super::*;

#[test]
fn crush_height_grows_to_three_visual_rows_then_stays_capped() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    assert_eq!(height_for_look(&theme, &c, 20, 30), 1);
    c.set_text("a\nb");
    assert_eq!(height_for_look(&theme, &c, 20, 30), 2);
    c.set_text(&"x".repeat(100)); // muchas filas visuales por el ajuste de línea
    assert_eq!(height_for_look(&theme, &c, 20, 30), 3);
    c.set_text(&"x\n".repeat(40));
    assert_eq!(height_for_look(&theme, &c, 20, 30), 3);
    assert_eq!(height_for_look(&theme, &c, 20, 2), 2);
}

#[test]
fn crush_cursor_follows_the_wrapped_row() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("abcdefghij"); // ancho 8: "> " + 6 celdas por fila
    let (_, cursor) = draw(&c, 8, 4, &theme);
    assert_eq!(cursor, Some((2 + 4, 1))); // fila 1 ("ghij"), tras 4 celdas
}

#[test]
fn crush_keeps_the_cursor_row_visible_when_overflowing() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("1\n2\n3\n4\n5\n6");
    let (screen, cursor) = draw(&c, 20, 3, &theme);
    assert!(screen.contains('6'));
    assert!(!screen.contains("> 1"));
    assert_eq!(cursor.map(|p| p.1), Some(2));
}

#[test]
fn crush_uses_prompt_and_continuation_gutters() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("uno\ndos");
    let (screen, _) = draw(&c, 20, 3, &theme);
    assert!(screen.contains("> uno"));
    assert!(screen.contains("::: dos"));
}

#[test]
fn crush_empty_shows_the_placeholder() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let (screen, cursor) = draw(&Composer::new(), 40, 3, &theme);
    assert!(screen.contains("> Describe what you want to build"));
    assert_eq!(cursor, Some((2, 0)));
}
