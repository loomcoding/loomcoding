//! Hipervínculos OSC 8 sobre las líneas visibles.

use super::*;

pub(super) fn osc8(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

/// Envuelve con OSC 8 los enlaces de las líneas que se ven enteras. Técnica
/// de hyperrat, un tramo por fila: la primera celda lleva la secuencia con
/// el texto del tramo y ancho forzado; el resto se deja en blanco y fuera
/// del diff (el terminal ya escribió ahí el texto).
pub(super) fn apply_links(
    buf: &mut Buffer,
    area: Rect,
    transcript: &Transcript,
    layout: &Layout,
    top: usize,
    theme: &Theme,
) {
    if !theme.hyperlinks {
        return;
    }
    let bottom = top + area.height as usize;
    let Some((mut i, _)) = layout.line_at_row(top) else {
        return;
    };
    while i < transcript.len() && layout.row_of(i) < bottom {
        let (start, h) = (layout.row_of(i), layout.height_of(i));
        if let Some(line) = transcript.line(i) {
            let header = usize::from(
                transcript.locate(i).is_some_and(|(_, row)| row == 0)
                    && matches!(line.kind, LineKind::User | LineKind::Assistant),
            );
            let body_start = start + header;
            let body_height = h.saturating_sub(header);
            if !line.links.is_empty() && body_start >= top && body_start + body_height <= bottom {
                let y0 = area.y + (body_start - top) as u16;
                link_line(buf, area, y0, body_height as u16, line);
            }
        }
        i += 1;
    }
}

/// Celda visible: posición, texto y ancho.
pub(super) struct VisibleCell {
    pub(super) x: u16,
    pub(super) y: u16,
    pub(super) symbol: String,
    pub(super) width: usize,
}

pub(super) fn link_line(buf: &mut Buffer, area: Rect, y0: u16, h: u16, line: &Line) {
    let mut cells = Vec::new();
    for y in y0..y0 + h {
        let mut x = area.x;
        while x < area.x + area.width {
            let symbol = buf[(x, y)].symbol().to_string();
            let width = symbol.width().max(1);
            cells.push(VisibleCell {
                x,
                y,
                symbol,
                width,
            });
            x += width as u16;
        }
    }
    // Texto sin espacios con el índice de la celda de cada símbolo: el wrap
    // mueve o rellena espacios, así que se compara sin ellos.
    let mut ns = String::new();
    let mut ns_cells: Vec<(usize, usize)> = Vec::new();
    for (j, c) in cells.iter().enumerate() {
        if !c.symbol.trim().is_empty() {
            ns_cells.push((ns.len(), j));
            ns.push_str(&c.symbol);
        }
    }
    let strip = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let prefix_len = strip(prefix(line.kind)).len();
    for link in &line.links {
        let Some(label) = line.text.get(link.start..link.start + link.len) else {
            continue;
        };
        let label = strip(label);
        if label.is_empty() {
            continue;
        }
        let nth = strip(&line.text[..link.start])
            .matches(label.as_str())
            .count();
        let Some(pos) = ns
            .get(prefix_len..)
            .and_then(|rest| rest.match_indices(label.as_str()).nth(nth))
            .map(|(p, _)| p + prefix_len)
        else {
            continue;
        };
        let end = pos + label.len();
        let covered: Vec<usize> = ns_cells
            .iter()
            .filter(|(off, _)| *off >= pos && *off < end)
            .map(|&(_, j)| j)
            .collect();
        let (Some(&first), Some(&last)) = (covered.first(), covered.last()) else {
            continue;
        };
        let mut j = first;
        while j <= last {
            let y = cells[j].y;
            let mut k = j;
            while k < last && cells[k + 1].y == y {
                k += 1;
            }
            // Tramo de la fila: de la primera a la última celda cubierta en ella.
            let row_last = covered
                .iter()
                .copied()
                .filter(|&c| cells[c].y == y)
                .max()
                .unwrap_or(j);
            let k = k.min(row_last);
            let text: String = cells[j..=k].iter().map(|c| c.symbol.as_str()).collect();
            let width: usize = cells[j..=k].iter().map(|c| c.width).sum();
            if let Some(w) = NonZeroU16::new(width as u16) {
                buf[(cells[j].x, y)]
                    .set_symbol(&osc8(&link.url, &text))
                    .set_diff_option(CellDiffOption::ForcedWidth(w));
                for c in &cells[j + 1..=k] {
                    buf[(c.x, c.y)]
                        .set_symbol(" ")
                        .set_diff_option(CellDiffOption::Skip);
                }
            }
            // Siguiente fila con celdas cubiertas.
            j = match covered.iter().copied().find(|&c| cells[c].y > y) {
                Some(next) => next,
                None => break,
            };
        }
    }
}
