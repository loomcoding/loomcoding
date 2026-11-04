//! Marcas sobre el marco: recolorea bordes y señala el panel enfocado.

use super::*;

/// Glifos de borde que el indicador de foco recolorea (nunca el texto del título).
pub(super) const BORDER_GLYPHS: [&str; 6] = ["─", "│", "╭", "╮", "╰", "╯"];

/// Recolorea con el acento del tema el borde de la celda (x, y), si lo es.
pub(super) fn recolor_border_cell(buf: &mut Buffer, x: u16, y: u16, accent: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        if BORDER_GLYPHS.contains(&cell.symbol()) {
            cell.set_style(accent);
        }
    }
}

/// Recolorea el borde izquierdo de una ventana (sidebar y rail solo tienen ese).
pub(super) fn recolor_left_edge(buf: &mut Buffer, area: Rect, accent: Style) {
    for y in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.left(), y)) {
            if cell.symbol() == "│" {
                cell.set_style(accent);
            }
        }
    }
}

/// Recolorea el marco completo de una ventana con caja.
pub(super) fn recolor_frame(buf: &mut Buffer, area: Rect, accent: Style) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let (left, right) = (area.left(), area.right() - 1);
    let (top, bottom) = (area.top(), area.bottom() - 1);
    for x in left..=right {
        recolor_border_cell(buf, x, top, accent);
        recolor_border_cell(buf, x, bottom, accent);
    }
    for y in top..=bottom {
        recolor_border_cell(buf, left, y, accent);
        recolor_border_cell(buf, right, y, accent);
    }
}

/// Marca con un filete de acento la columna de margen de una ventana sin caja.
pub(super) fn mark_margin(buf: &mut Buffer, area: Rect, accent: Style) {
    for y in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.left(), y)) {
            cell.set_symbol("▎");
            cell.set_style(accent);
        }
    }
}

/// Indicador de foco: resalta la ventana con `app.focus`. El composer no se
/// marca, porque ya muestra su propio cursor. Con el chrome `Flat` (Crush) no
/// hay caja: el margen del transcript recibe un filete y el rail recolorea su
/// borde izquierdo.
pub(super) fn mark_focused_panes(
    frame: &mut Frame,
    panes: &[(Focus, Rect)],
    app: &App,
    flat: bool,
) {
    let accent = app.theme.styles.accent;
    let buf = frame.buffer_mut();
    for (pane, area) in panes {
        if *pane != app.focus || *pane == Focus::Composer || area.is_empty() {
            continue;
        }
        match pane {
            Focus::Sidebar | Focus::Rail => recolor_left_edge(buf, *area, accent),
            // Con `Flat` el transcript no pinta filete: sus líneas por bloque ya marcan el foco.
            Focus::Transcript if flat => {}
            _ if flat => mark_margin(buf, *area, accent),
            _ => recolor_frame(buf, *area, accent),
        }
    }
}

/// Color del marcador grueso del bloque activo, según el rol de su primera línea:
/// el acento para el usuario y un verde suave (éxito) para el resto.
pub(super) fn active_marker_style(kind: loomcoding_cloth::state::LineKind, app: &App) -> Style {
    use loomcoding_cloth::state::LineKind;
    if matches!(kind, LineKind::User) {
        app.theme.styles.accent
    } else {
        app.theme.styles.success
    }
}
