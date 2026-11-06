//! Tests: layout.

use super::support::*;
use super::*;

#[test]
fn only_changed_entries_are_measured() {
    let (mut t, last) = long_transcript(1000);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    draw(&t, &mut vp, &mut layout, 40, 10);
    let before = layout.measured();
    t.append(last, " y más");
    let out = draw(&t, &mut vp, &mut layout, 40, 10);
    assert_eq!(layout.measured(), before + 1);
    assert!(out.contains("fin y más"));
}

#[test]
fn width_change_remeasures_everything() {
    let (t, _) = long_transcript(100);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    let before = layout.measured();
    draw(&t, &mut vp, &mut layout, 60, 10);
    assert!(layout.measured() >= before + 101);
}

#[test]
fn line_at_row_handles_multi_row_lines() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Banner, "corta"));
    t.push(Line::new(LineKind::Banner, "palabra ".repeat(10)));
    t.push(Line::new(LineKind::Banner, "otra"));
    let mut layout = Layout::new();
    layout.sync(&t, 20, &Theme::test());
    let h1 = layout.height_of(1);
    assert!(h1 > 1);
    assert_eq!(layout.line_at_row(0), Some((0, 0)));
    assert_eq!(layout.line_at_row(1), Some((1, 0)));
    assert_eq!(layout.line_at_row(h1), Some((1, h1 - 1)));
    assert_eq!(layout.line_at_row(h1 + 1), Some((2, 0)));
    assert_eq!(layout.line_at_row(10_000), Some((2, 0)));
}

#[test]
fn rows_beyond_u16_are_reachable() {
    let mut t = Transcript::new();
    for i in 0..70_000 {
        t.push(Line::new(LineKind::Banner, format!("l{i:05}")));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    let out = draw(&t, &mut vp, &mut layout, 20, 5);
    assert!(out.contains("l69999"));
    vp.to_top();
    let out = draw(&t, &mut vp, &mut layout, 20, 5);
    assert!(out.contains("l00000"));
}

#[test]
fn width_change_keeps_the_anchor_line_on_top() {
    let mut t = Transcript::new();
    for i in 0..200 {
        t.push(Line::new(
            LineKind::Assistant,
            format!("a{i:03} {}", "xx ".repeat(15)),
        ));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 10);
    vp.scroll_up(100);
    draw(&t, &mut vp, &mut layout, 40, 10);
    let (line, _) = vp.anchor().unwrap();
    draw(&t, &mut vp, &mut layout, 25, 10);
    assert_eq!(vp.anchor().unwrap().0, line);
}

#[test]
fn rail_box_adds_one_row_to_the_last_line_of_a_user_entry() {
    let mut t = Transcript::new();
    t.push_entry(LineKind::User, crate::state::Format::Plain, "uno\ndos");
    t.push_entry(
        LineKind::Assistant,
        crate::state::Format::Plain,
        "respuesta",
    );
    assert_eq!(
        t.entry_lines(0).len(),
        2,
        "la entrada de usuario tiene dos líneas"
    );

    let mut theme = Theme::test();
    let mut sin = Layout::new();
    sin.sync(&t, 40, &theme);
    theme.rail_box = true;
    let mut con = Layout::new();
    con.sync(&t, 40, &theme);

    // Solo la última línea de la entrada del usuario gana una fila: el borde inferior.
    assert_eq!(con.height_of(0), sin.height_of(0));
    assert_eq!(con.height_of(1), sin.height_of(1) + 1);
    // El asistente no cambia, ni siquiera con la caja activa.
    assert_eq!(con.height_of(2), sin.height_of(2));
    assert_eq!(con.total_rows(), sin.total_rows() + 1);
    // Los índices de línea no cambian: solo las alturas.
    assert_eq!(con.row_of(2), sin.row_of(2) + 1);
}

#[test]
fn rail_box_does_not_touch_assistant_entries() {
    let mut t = Transcript::new();
    t.push_entry(LineKind::Assistant, crate::state::Format::Plain, "a\nb");
    let mut theme = Theme::test();
    theme.rail_box = true;
    let mut layout = Layout::new();
    layout.sync(&t, 40, &theme);
    let mut plano = Layout::new();
    plano.sync(&t, 40, &Theme::test());
    assert_eq!(layout.total_rows(), plano.total_rows());
}

#[test]
fn invalidate_remeasures_after_the_box_changes_height() {
    let mut t = Transcript::new();
    t.push_entry(LineKind::User, crate::state::Format::Plain, "pregunta");
    let mut theme = Theme::test();
    let mut layout = Layout::new();
    layout.sync(&t, 40, &theme);
    let antes = layout.total_rows();

    theme.rail_box = true;
    // Sin invalidar, la caché de alturas manda y no cambia nada.
    layout.sync(&t, 40, &theme);
    assert_eq!(layout.total_rows(), antes);

    layout.invalidate();
    layout.sync(&t, 40, &theme);
    assert_eq!(layout.total_rows(), antes + 1);
    assert_eq!(layout.width(), 40);
}
