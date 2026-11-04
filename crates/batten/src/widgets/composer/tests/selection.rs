//! Tests: selection.

use super::*;

#[test]
fn split_selected_without_selection() {
    assert_eq!(
        split_selected("abc\ndef", 4, 7, None),
        [("def", false), ("", false), ("", false)]
    );
}

#[test]
fn split_selected_partial_selection() {
    assert_eq!(
        split_selected("abcdef", 0, 6, Some((2, 4))),
        [("ab", false), ("cd", true), ("ef", false)]
    );
    // Los offsets son bytes: la «é» ocupa dos y el corte cae entre grafemas.
    assert_eq!(
        split_selected("héllo", 0, 6, Some((1, 3))),
        [("h", false), ("é", true), ("llo", false)]
    );
}

#[test]
fn split_selected_full_row() {
    assert_eq!(
        split_selected("abc\ndef", 4, 7, Some((4, 7))),
        [("", false), ("def", true), ("", false)]
    );
}

#[test]
fn split_selected_outside_the_row() {
    let text = "abc\ndef\nghi";
    let plain = [("def", false), ("", false), ("", false)];
    assert_eq!(split_selected(text, 4, 7, Some((0, 3))), plain);
    assert_eq!(split_selected(text, 4, 7, Some((8, 11))), plain);
}

#[test]
fn split_selected_starts_before_the_row_and_ends_inside() {
    assert_eq!(
        split_selected("abc\ndef", 4, 7, Some((2, 5))),
        [("", false), ("d", true), ("ef", false)]
    );
}

#[test]
fn crush_selection_is_highlighted_with_the_selection_style() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let sel_bg = theme.style(StyleToken::Selection).bg;
    assert!(sel_bg.is_some());
    let mut c = Composer::new();
    c.set_text("uno dos");
    c.select_range(0, 3); // "uno"
    let mut term = Terminal::new(TestBackend::new(20, 3)).unwrap();
    term.draw(|f| {
        render_crush(f, f.area(), &c, true, "", &theme);
    })
    .unwrap();
    let buf = term.backend().buffer();
    for x in 2..5 {
        assert_eq!(buf[(x, 0)].bg, sel_bg.unwrap(), "celda {x} seleccionada");
    }
    for x in 5..8 {
        assert_ne!(buf[(x, 0)].bg, sel_bg.unwrap(), "celda {x} sin seleccionar");
    }
}

#[test]
fn classic_selection_is_highlighted() {
    let theme = Theme::test();
    let sel_bg = theme.style(StyleToken::Selection).bg;
    assert!(sel_bg.is_some());
    let mut c = Composer::new();
    c.set_text("ab\ncd");
    c.select_range(1, 4); // "b\nc"
    let mut term = Terminal::new(TestBackend::new(20, 4)).unwrap();
    term.draw(|f| render_focused_with_model(f, f.area(), &c, true, "", &theme))
        .unwrap();
    let buf = term.backend().buffer();
    let selected = |x: u16, y: u16| buf[(x, y)].bg == sel_bg.unwrap();
    // Fila 1: "❯ ab" (la «b» y el salto de línea seleccionado, tras él no).
    assert!(!selected(3, 1), "la «a» no está seleccionada");
    assert!(selected(4, 1), "la «b» sí");
    assert!(selected(5, 1), "el salto de línea se marca con una celda");
    assert!(!selected(6, 1));
    // Fila 2: "│ cd" (solo la «c»).
    assert!(selected(3, 2));
    assert!(!selected(4, 2));
}
