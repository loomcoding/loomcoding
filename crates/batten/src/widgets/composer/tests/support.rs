//! Piezas comunes de los tests.

use super::*;

pub(super) fn buffer(c: &Composer, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| render(f, f.area(), c, &Theme::test()))
        .unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

/// Pinta `render_crush` en un terminal de prueba y devuelve la pantalla
/// (una línea por fila) y la posición del cursor (x, y).
pub(super) fn draw(c: &Composer, w: u16, h: u16, theme: &Theme) -> (String, Option<(u16, u16)>) {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        render_crush(
            f,
            f.area(),
            c,
            true,
            "Describe what you want to build…",
            theme,
        )
    })
    .unwrap();
    let screen = term
        .backend()
        .buffer()
        .content
        .chunks(usize::from(w).max(1))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    let pos = term
        .backend_mut()
        .get_cursor_position()
        .ok()
        .map(|p| (p.x, p.y));
    (screen, pos)
}

/// Convierte el búfer de un terminal de prueba en una cadena por fila.
pub(super) fn screen_rows(term: &Terminal<TestBackend>, w: u16) -> Vec<String> {
    term.backend()
        .buffer()
        .content
        .chunks(usize::from(w).max(1))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect()
}

/// Pinta `render_crush` y devuelve sus filas de pantalla.
pub(super) fn crush_rows(c: &Composer, w: u16, h: u16, theme: &Theme) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        render_crush(
            f,
            f.area(),
            c,
            true,
            "Describe what you want to build…",
            theme,
        )
    })
    .unwrap();
    screen_rows(&term, w)
}

/// Pinta `render_focused_with_model` (Classic) y devuelve sus filas.
pub(super) fn classic_rows(c: &Composer, w: u16, h: u16, theme: &Theme) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render_focused_with_model(f, f.area(), c, true, "", theme))
        .unwrap();
    screen_rows(&term, w)
}

/// Pinta `render_for_look` con el cursor puesto y devuelve la posición del
/// cursor que fijó el renderer.
pub(super) fn cursor_after_render(c: &Composer, area: Rect, theme: &Theme) -> Option<(u16, u16)> {
    let mut term = Terminal::new(TestBackend::new(area.right(), area.bottom())).unwrap();
    term.draw(|f| render_for_look(f, area, c, true, "", theme))
        .unwrap();
    term.backend_mut()
        .get_cursor_position()
        .ok()
        .map(|p| (p.x, p.y))
}
