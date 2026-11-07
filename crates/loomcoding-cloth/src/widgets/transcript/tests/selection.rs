//! Tests: selection.

use super::support::*;
use super::*;

#[test]
fn selected_lines_are_highlighted() {
    use crate::state::Selection;
    let mut t = Transcript::new();
    for s in ["uno", "dos", "tres"] {
        t.push(Line::new(LineKind::Assistant, s));
    }
    let mut term = Terminal::new(TestBackend::new(30, 5)).unwrap();
    let mut vp = Viewport::new();
    let mut layout = Layout::new();
    let sel = Selection::at(1);
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
    let buf = term.backend().buffer();
    assert_eq!(buf[(0, 1)].bg, ratatui::style::Color::Indexed(238));
    assert_ne!(buf[(0, 0)].bg, ratatui::style::Color::Indexed(238));
}

#[test]
fn copied_job_lines_keep_their_prefix() {
    let mut t = Transcript::new();
    t.push(Line::new(LineKind::Job, "[Job 3 completed] hecho"));
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 40, 3);
    let sel = select(CellPos { row: 0, col: 0 }, CellPos { row: 0, col: 39 });
    let texto = selected_text(&t, &layout, &Theme::test(), &sel);
    assert!(
        texto.starts_with("job: [Job 3 completed]"),
        "la copia pierde el actor: {texto:?}"
    );
}

#[test]
fn hit_cell_maps_screen_to_document_with_scroll() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Banner, format!("line {i:02}")));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 30, 5);
    vp.scroll_up(10);
    draw(&t, &mut vp, &mut layout, 30, 5);
    let top = vp.top();
    assert!(top >= 2, "precondición: la vista debe estar desplazada");
    let area = Rect::new(0, 0, 30, 5);
    assert_eq!(
        hit_cell(&layout, &vp, area, 3, 2),
        Some(CellPos {
            row: top + 2,
            col: 3
        })
    );
}

#[test]
fn hit_cell_clamps_outside_the_area() {
    let t = banners(&["uno", "dos", "tres"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 10);
    let area = Rect::new(2, 1, 20, 8);
    // Por encima y a la izquierda: primera fila visible, columna 0.
    assert_eq!(
        hit_cell(&layout, &vp, area, 0, 0),
        Some(CellPos { row: 0, col: 0 })
    );
    // Por debajo y a la derecha: última fila del documento, última columna.
    assert_eq!(
        hit_cell(&layout, &vp, area, 500, 500),
        Some(CellPos { row: 2, col: 19 })
    );
}

#[test]
fn hit_cell_none_when_empty() {
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&Transcript::new(), &mut vp, &mut layout, 20, 5);
    assert_eq!(hit_cell(&layout, &vp, Rect::new(0, 0, 20, 5), 1, 1), None);
    draw(&banners(&["uno"]), &mut vp, &mut layout, 20, 5);
    assert_eq!(hit_cell(&layout, &vp, Rect::new(0, 0, 0, 5), 0, 0), None);
}

#[test]
fn paint_selects_partial_first_and_last_rows() {
    let t = banners(&["hello", "world", "fin"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 10, 4);
    let area = Rect::new(0, 0, 10, 4);
    // Desde (fila 0, col 2) hasta (fila 2, col 1, incluida).
    let sel = select(CellPos { row: 0, col: 2 }, CellPos { row: 2, col: 1 });
    let mut buf = Buffer::empty(area);
    paint_text_selection(
        &mut buf,
        area,
        &layout,
        &vp,
        &sel,
        Style::default().bg(Color::Red),
    );
    let red = |x: u16, y: u16| buf[(x, y)].bg == Color::Red;
    assert!(!red(0, 0) && !red(1, 0), "la primera fila empieza en col 2");
    assert!(red(2, 0) && red(9, 0));
    assert!(red(0, 1) && red(9, 1), "las filas intermedias van enteras");
    assert!(
        red(0, 2) && red(1, 2) && !red(2, 2),
        "la última fila incluye la col 1"
    );
    assert!(!red(0, 3), "fila fuera del documento");
}

#[test]
fn paint_selection_ignores_rows_out_of_view() {
    let mut t = Transcript::new();
    for i in 0..50 {
        t.push(Line::new(LineKind::Banner, format!("line {i:02}")));
    }
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 30, 5);
    vp.scroll_up(10);
    draw(&t, &mut vp, &mut layout, 30, 5);
    let top = vp.top();
    assert!(top >= 2, "precondición: la vista debe estar desplazada");
    let area = Rect::new(0, 0, 30, 5);
    let style = Style::default().bg(Color::Red);

    let above = select(CellPos { row: 0, col: 0 }, CellPos { row: 2, col: 3 });
    let mut buf = Buffer::empty(area);
    paint_text_selection(&mut buf, area, &layout, &vp, &above, style);
    assert!(buf.content.iter().all(|c| c.bg != Color::Red));

    // Empieza antes de la vista y acaba una fila dentro: solo se ve su tramo.
    let through = select(
        CellPos {
            row: top - 2,
            col: 0,
        },
        CellPos {
            row: top + 1,
            col: 4,
        },
    );
    let mut buf = Buffer::empty(area);
    paint_text_selection(&mut buf, area, &layout, &vp, &through, style);
    let red = |x: u16, y: u16| buf[(x, y)].bg == Color::Red;
    assert!(red(0, 0) && red(28, 0), "fila superior visible entera");
    assert!(!red(29, 0), "no se pinta bajo la barra de scroll");
    assert!(red(4, 1) && !red(5, 1), "la última fila incluye la col 4");
    assert!(!red(0, 2), "la selección no llega a la tercera fila");
}

#[test]
fn selected_text_multirow_trims_trailing_spaces() {
    let t = banners(&["uno", "dos", "tres"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 5);
    let sel = select(CellPos { row: 0, col: 1 }, CellPos { row: 2, col: 2 });
    assert_eq!(
        selected_text(&t, &layout, &Theme::test(), &sel),
        "no\ndos\ntre"
    );
}

#[test]
fn selected_text_clamps_rows_past_the_end() {
    let t = banners(&["uno", "dos", "tres"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 5);
    let sel = select(CellPos { row: 0, col: 0 }, CellPos { row: 99, col: 3 });
    assert_eq!(
        selected_text(&t, &layout, &Theme::test(), &sel),
        "uno\ndos\ntres"
    );
}

#[test]
fn selected_text_includes_wrapped_rows_joined_by_newline() {
    let long = "palabra ".repeat(10);
    let t = banners(&[long.as_str()]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 10);
    let h = layout.height_of(0);
    assert!(h > 1, "precondición: la línea debe ajustarse");
    let width = layout.width();
    let sel = select(
        CellPos { row: 0, col: 0 },
        CellPos {
            row: h - 1,
            col: width,
        },
    );
    let out = selected_text(&t, &layout, &Theme::test(), &sel);
    assert_eq!(out.split('\n').count(), h, "una fila por línea visual");
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    assert_eq!(squash(&out), squash(&long));
}

#[test]
fn selected_text_does_not_split_wide_chars() {
    let t = banners(&["日本語", "a🚀b"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 5);
    let theme = Theme::test();
    let full = select(CellPos { row: 0, col: 0 }, CellPos { row: 0, col: 6 });
    assert_eq!(selected_text(&t, &layout, &theme, &full), "日本語");
    let emoji = select(CellPos { row: 1, col: 0 }, CellPos { row: 1, col: 4 });
    assert_eq!(selected_text(&t, &layout, &theme, &emoji), "a🚀b");
    // Empieza en la mitad derecha de «日»: esa celda no se copia a medias.
    let half = select(CellPos { row: 0, col: 1 }, CellPos { row: 0, col: 6 });
    assert_eq!(selected_text(&t, &layout, &theme, &half), "本語");
}

#[test]
fn selected_text_empty_selection_is_empty() {
    let t = banners(&["uno"]);
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    draw(&t, &mut vp, &mut layout, 20, 5);
    let sel = TextSelection::new(CellPos { row: 0, col: 1 });
    assert_eq!(selected_text(&t, &layout, &Theme::test(), &sel), "");
}
