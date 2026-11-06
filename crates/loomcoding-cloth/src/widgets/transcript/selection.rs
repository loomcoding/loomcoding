//! Selección con el ratón: celda pulsada, pintado y texto seleccionado.

use super::*;

/// Posición de documento bajo la celda `(x, y)` de la pantalla. Consulta pura
/// que acota `y` a las filas de `area` y `x` al ancho del texto, de modo que
/// nunca devuelve una posición fuera del diseño. `None` si no hay área o
/// contenido.
pub fn hit_cell(
    layout: &Layout,
    viewport: &Viewport,
    area: Rect,
    x: u16,
    y: u16,
) -> Option<CellPos> {
    if area.width == 0 || area.height == 0 || layout.total_rows() == 0 {
        return None;
    }
    let last_row = layout.total_rows() - 1;
    let bottom = area.y.saturating_add(area.height) - 1;
    let y = y.clamp(area.y, bottom);
    let row = (viewport.top() + usize::from(y - area.y)).min(last_row);
    let last_col = layout.width().max(1) - 1;
    let col = x.saturating_sub(area.x).min(last_col);
    Some(CellPos { row, col })
}

/// Resalta con `style` la selección sobre el búfer ya pintado. Solo parchea las
/// celdas de las filas visibles que caen dentro del rango: la primera fila
/// desde `start.col`, la última hasta `end.col` (incluida) y las intermedias
/// enteras. No re-renderiza el transcript.
pub fn paint_text_selection(
    buf: &mut Buffer,
    area: Rect,
    layout: &Layout,
    viewport: &Viewport,
    sel: &TextSelection,
    style: Style,
) {
    if sel.is_empty() || area.width == 0 || area.height == 0 || layout.total_rows() == 0 {
        return;
    }
    let (start, end) = sel.ordered();
    let last_row = layout.total_rows() - 1;
    if start.row > last_row {
        return;
    }
    let end_row = end.row.min(last_row);
    let top = viewport.top();
    let first = start.row.max(top);
    let last = end_row.min(top + usize::from(area.height) - 1);
    if first > last {
        return;
    }
    let text_w = layout.width().min(area.width);
    for row in first..=last {
        let dy = u16::try_from(row - top).unwrap_or(u16::MAX);
        let y = area.y + dy;
        let from = if row == start.row { start.col } else { 0 };
        let to = if row == end_row {
            end.col.saturating_add(1).min(text_w)
        } else {
            text_w
        };
        for x in from.min(text_w)..to.min(text_w) {
            if let Some(cell) = buf.cell_mut((area.x + x, y)) {
                cell.set_style(style);
            }
        }
    }
}

/// Lee de un búfer de una línea renderizada el texto de la fila `y` entre las
/// columnas `from` (incluida) y `to` (exclusiva; el llamador suma uno al extremo incluido). Un carácter ancho entra solo
/// si empieza dentro del rango: avanza por su ancho, así que su celda de
/// continuación nunca se copia suelta ni a medias.
pub(super) fn row_text(buf: &Buffer, y: u16, width: u16, from: u16, to: u16) -> String {
    let mut out = String::new();
    let mut x: u16 = 0;
    while x < width {
        let symbol = buf[(x, y)].symbol();
        if x >= from && x < to {
            out.push_str(symbol);
        }
        x = x.saturating_add(symbol.width().max(1) as u16);
    }
    out.trim_end().to_string()
}

/// Texto plano de la selección. Re-renderiza fuera de pantalla, línea a línea y
/// con el mismo recorrido que `render`, solo las líneas que cubren el rango; así
/// el texto copiado es exactamente lo que se ve. Solo se llama al copiar.
pub fn selected_text(
    transcript: &Transcript,
    layout: &Layout,
    theme: &Theme,
    sel: &TextSelection,
) -> String {
    let text_w = layout.width();
    if sel.is_empty() || layout.total_rows() == 0 || text_w == 0 {
        return String::new();
    }
    let (start, end) = sel.ordered();
    let last_row = layout.total_rows() - 1;
    if start.row > last_row {
        return String::new();
    }
    let end_row = end.row.min(last_row);
    let Some((first, _)) = layout.line_at_row(start.row) else {
        return String::new();
    };
    let Some((last, _)) = layout.line_at_row(end_row) else {
        return String::new();
    };
    let mut rows = Vec::new();
    for i in first..=last {
        let (Some(line), line_row, h) = (transcript.line(i), layout.row_of(i), layout.height_of(i))
        else {
            break;
        };
        if h == 0 || start.row > line_row + h - 1 || end_row < line_row {
            continue;
        }
        let h16 = u16::try_from(h).unwrap_or(u16::MAX);
        let (entry_start, entry_end) = posicion_en_entrada(transcript, i);
        let area = Rect::new(0, 0, text_w, h16);
        let mut buf = Buffer::empty(area);
        Paragraph::new(render_line(line, theme, entry_start, entry_end))
            .wrap(Wrap { trim: false })
            .render(area, &mut buf);
        let lo = start.row.max(line_row);
        let hi = end_row.min(line_row + h - 1);
        for row in lo..=hi {
            let y = u16::try_from(row - line_row).unwrap_or(u16::MAX);
            let from = if row == start.row { start.col } else { 0 };
            let to = if row == end_row {
                end.col.saturating_add(1).min(text_w)
            } else {
                text_w
            };
            rows.push(row_text(&buf, y, text_w, from, to));
        }
    }
    rows.join("\n")
}
