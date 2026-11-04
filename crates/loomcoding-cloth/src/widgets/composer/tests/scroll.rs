//! Tests: scroll.

use super::support::*;
use super::*;

#[test]
fn crush_view_top_stays_when_cursor_moves_inside_the_viewport() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("1\n2\n3\n4\n5\n6\n7\n8");
    c.set_cursor(8); // línea "5", fila 4
    c.set_view_top(3);
    let _ = crush_rows(&c, 20, 3, &theme);
    assert_eq!(c.view_top(), 3);
    // Se mueve a la fila 3 (dentro de la vista): la vista no debe desplazarse.
    c.set_cursor(6);
    let rows = crush_rows(&c, 20, 3, &theme);
    assert_eq!(c.view_top(), 3);
    assert!(rows[0].contains('4'), "{rows:?}");
}

#[test]
fn crush_view_top_follows_the_cursor_past_the_edges() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("1\n2\n3\n4\n5\n6\n7\n8");
    c.set_view_top(3);
    c.set_cursor(0); // por encima de la vista
    let rows = crush_rows(&c, 20, 3, &theme);
    assert_eq!(c.view_top(), 0);
    assert!(rows[0].contains('1'), "{rows:?}");
    c.set_cursor(14); // línea "8", por debajo de la vista
    let rows = crush_rows(&c, 20, 3, &theme);
    assert_eq!(c.view_top(), 5);
    assert!(rows[2].contains('8'), "{rows:?}");
}

#[test]
fn crush_typing_at_the_end_scrolls_like_before() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    for step in 0..10 {
        c.insert_str(&format!("{step}\n"));
        let _ = crush_rows(&c, 20, 3, &theme);
        let row = crush_layout(&c, 20).cursor.0;
        // Cálculo anterior a la tarea: el cursor queda en la última fila.
        assert_eq!(c.view_top(), row.saturating_sub(2), "paso {step}");
    }
}

#[test]
fn composer_scrollbar_is_thin_and_thumb_reaches_both_ends() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text(
        &(0..30)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let area = Rect::new(0, 0, 20, 5);
    let mut term = Terminal::new(TestBackend::new(20, 5)).unwrap();

    c.set_cursor(0);
    term.draw(|f| render_for_look(f, area, &c, true, "", &theme))
        .unwrap();
    let top = (0..5)
        .find(|&y| term.backend().buffer()[(19, y)].symbol() == "▏")
        .expect("el thumb debe comenzar arriba");
    assert_eq!(top, 0);

    c.set_cursor(c.text().graphemes(true).count());
    term.draw(|f| render_for_look(f, area, &c, true, "", &theme))
        .unwrap();
    let bottom = (0..5)
        .find(|&y| term.backend().buffer()[(19, y)].symbol() == "▏")
        .expect("el thumb debe permanecer visible");
    assert!(bottom > top, "thumb arriba={top}, abajo={bottom}");
}

#[test]
fn wheel_scroll_moves_through_wrapped_rows_in_crush() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut composer = Composer::new();
    composer.set_text(&"x".repeat(500));
    composer.set_cursor(250);
    let area = Rect::new(0, 0, 20, 8);
    let before = composer.cursor();

    scroll_cursor_by_rows(&theme, &mut composer, area, true, 100);

    assert!(composer.cursor() > before);
    assert!(composer.cursor() > 450);
}

#[test]
fn classic_view_top_stays_when_cursor_moves_inside_the_viewport() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("1\n2\n3\n4\n5\n6\n7\n8");
    c.set_cursor(8); // línea "5", fila lógica 4
    c.set_view_top(3);
    let _ = classic_rows(&c, 20, 5, &theme); // 3 filas útiles dentro del marco
    assert_eq!(c.view_top(), 3);
    c.set_cursor(6); // línea "4", dentro de la vista
    let rows = classic_rows(&c, 20, 5, &theme);
    assert_eq!(c.view_top(), 3);
    assert!(rows[1].contains('4'), "{rows:?}");
}

#[test]
fn classic_typing_at_the_end_scrolls_like_before() {
    let theme = Theme::test();
    let mut c = Composer::new();
    for step in 0..10 {
        c.insert_str(&format!("{step}\n"));
        let _ = classic_rows(&c, 20, 5, &theme);
        let (row, _) = c.cursor_row_col();
        assert_eq!(c.view_top(), row.saturating_sub(2), "paso {step}");
    }
}
