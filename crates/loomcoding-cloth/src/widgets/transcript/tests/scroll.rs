//! Tests: scroll.

use super::support::*;
use super::*;

#[test]
fn shows_only_the_viewport_window() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Assistant, format!("line {i}")));
    }
    let out = buffer(&t, 40, 5); // 5 filas -> ventana 45..50
    assert!(out.contains("line 49"));
    assert!(!out.contains("line 0"));
}

#[test]
fn shows_a_scrollbar_when_content_overflows() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Assistant, format!("line {i}")));
    }
    let out = buffer(&t, 20, 8);
    assert!(out.contains('▲') && out.contains('▼'), "faltan las flechas");
    assert!(out.contains('▒'), "falta la pista de la barra");
    assert!(out.contains('█'), "falta el thumb de la barra");
}

#[test]
fn no_scrollbar_when_content_fits() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Assistant, "hi"));
    let out = buffer(&t, 20, 5);
    assert!(!out.contains('\u{2551}') && !out.contains('\u{2588}'));
}

#[test]
fn wrapped_lines_count_as_rows_when_following() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::User, "inicio"));
    t.push(Line::new(LineKind::Assistant, "palabra ".repeat(20)));
    t.push(Line::new(LineKind::Assistant, "fin"));
    let out = buffer(&t, 20, 3);
    assert!(out.contains("fin"), "la última línea debe verse");
    assert!(!out.contains("inicio"));
}

#[test]
fn reading_up_keeps_the_view_while_content_grows() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Assistant, format!("line {i:02}")));
    }
    let mut vp = Viewport::new();
    let mut layout = Layout::new();
    draw(&t, &mut vp, &mut layout, 30, 5);
    vp.scroll_up(10);
    let before = draw(&t, &mut vp, &mut layout, 30, 5);
    for i in 50..70 {
        t.push(Line::new(LineKind::Assistant, format!("line {i:02}")));
    }
    let after = draw(&t, &mut vp, &mut layout, 30, 5);
    assert!(before.contains("line 42"));
    assert!(after.contains("line 42"));
    assert!(!after.contains("line 69"));
}

#[test]
fn the_selection_head_is_scrolled_into_view() {
    use crate::state::Selection;
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Assistant, format!("line {i:02}")));
    }
    let mut term = Terminal::new(TestBackend::new(30, 5)).unwrap();
    let mut vp = Viewport::new();
    let mut layout = Layout::new();
    let sel = Selection::at(0);
    term.draw(|f| {
        render(
            f,
            f.area(),
            &t,
            &mut vp,
            &mut layout,
            Some(&sel),
            &Theme::test(),
        )
    })
    .unwrap();
    let out: String = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(out.contains("line 00"));
    assert!(!vp.is_following());
}

#[test]
fn following_shows_new_content() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Assistant, format!("line {i:02}")));
    }
    let mut vp = Viewport::new();
    let mut layout = Layout::new();
    draw(&t, &mut vp, &mut layout, 30, 5);
    t.push(Line::new(LineKind::Assistant, "nueva"));
    assert!(draw(&t, &mut vp, &mut layout, 30, 5).contains("nueva"));
}
