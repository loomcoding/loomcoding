//! Geometría del texto: ventana de líneas, partes seleccionadas y límites de grafema.

use super::*;

/// Ventana visible de una línea lógica, en índices de grafema `[inicio, fin)`,
/// junto con la columna del cursor dentro de la ventana.
#[cfg(test)]
pub(super) fn line_window(
    line: &str,
    cursor: Option<usize>,
    width: usize,
) -> (usize, usize, usize) {
    let graphemes = line.graphemes(true).collect::<Vec<_>>();
    if width == 0 {
        return (0, 0, 0);
    }
    let Some(cursor) = cursor.map(|cursor| cursor.min(graphemes.len())) else {
        let mut used = 0;
        let mut end = 0;
        for grapheme in &graphemes {
            let next = used + UnicodeWidthStr::width(*grapheme);
            if next > width {
                break;
            }
            used = next;
            end += 1;
        }
        return (0, end, 0);
    };

    // Keep the caret in view while filling from the left first, as Herdr does.
    let mut start = cursor;
    let mut cursor_col = 0;
    for grapheme in graphemes[..cursor].iter().rev() {
        let next = cursor_col + UnicodeWidthStr::width(*grapheme);
        if next >= width {
            break;
        }
        cursor_col = next;
        start -= 1;
    }
    let mut end = cursor;
    let mut used = cursor_col;
    for grapheme in &graphemes[cursor..] {
        let next = used + UnicodeWidthStr::width(*grapheme);
        if next > width {
            break;
        }
        used = next;
        end += 1;
    }
    (start, end, cursor_col)
}

/// Texto visible de una línea según `line_window`. Solo lo usan las pruebas:
/// el render trabaja con rangos de bytes sobre el texto completo.
#[cfg(test)]
pub(super) fn line_view(line: &str, cursor: Option<usize>, width: usize) -> (String, usize) {
    let graphemes = line.graphemes(true).collect::<Vec<_>>();
    let (start, end, col) = line_window(line, cursor, width);
    (graphemes[start..end].concat(), col)
}

/// Divide la fila `[row_start, row_end)` de `text` en (antes, seleccionado,
/// después). `sel` es la selección en bytes del texto completo. Los tres
/// tramos son subcadenas de la fila y su unión es la fila entera; sin
/// solapamiento con la selección, el tramo seleccionado queda vacío.
pub(super) fn split_selected(
    text: &str,
    row_start: usize,
    row_end: usize,
    sel: Option<(usize, usize)>,
) -> [(&str, bool); 3] {
    let plain = [(&text[row_start..row_end], false), ("", false), ("", false)];
    let Some((sel_start, sel_end)) = sel else {
        return plain;
    };
    let start = sel_start.clamp(row_start, row_end);
    let end = sel_end.clamp(row_start, row_end);
    if start == end {
        return plain;
    }
    [
        (&text[row_start..start], false),
        (&text[start..end], true),
        (&text[end..row_end], false),
    ]
}

/// Añade a `spans` los tramos no vacíos de una fila, con el estilo llano o el
/// de selección según la marca de cada tramo.
pub(super) fn push_parts<'a>(
    spans: &mut Vec<Span<'a>>,
    parts: [(&'a str, bool); 3],
    plain: Style,
    selected: Style,
) {
    for (part, is_selected) in parts {
        if !part.is_empty() {
            spans.push(Span::styled(
                part,
                if is_selected { selected } else { plain },
            ));
        }
    }
}

/// Indica si el salto de línea que cierra la fila `[.., row_end)` queda dentro
/// de la selección. Se muestra como una celda seleccionada de un espacio, solo
/// en el aspecto: el texto no cambia.
pub(super) fn newline_selected(text: &str, row_end: usize, sel: Option<(usize, usize)>) -> bool {
    sel.is_some_and(|(start, end)| start <= row_end && row_end < end)
        && text.as_bytes().get(row_end) == Some(&b'\n')
}

/// Fila superior visible que mantiene el cursor a la vista desplazando lo
/// mínimo desde `view_top`. Con el cursor en la última fila equivale a
/// `cursor_row - (visible - 1)`, el cálculo que se usaba antes.
pub(super) fn stable_top(
    view_top: usize,
    cursor_row: usize,
    visible: usize,
    total_rows: usize,
) -> usize {
    let visible = visible.max(1);
    let lowest = cursor_row.saturating_sub(visible - 1);
    view_top
        .clamp(lowest, cursor_row)
        .min(total_rows.saturating_sub(visible))
        // Red de seguridad: si el cursor cae fuera de las filas, nunca lo
        // dejamos por encima de su ventana mínima.
        .max(lowest)
}

pub(super) fn content_width(width: u16) -> u16 {
    if width > 1 {
        width - 1
    } else {
        width
    }
}

/// Fila (lógica o visual) bajo una celda de un marco de `height` filas que
/// empieza en `top` y muestra `total` filas. `offset` es la distancia desde el
/// borde superior del interior. El resultado se acota a las filas visibles que
/// existen: un clic fuera del texto cae en la primera o en la última.
pub(super) fn visible_index(top: usize, height: usize, total: usize, offset: usize) -> usize {
    let last_row = total.saturating_sub(1);
    let first = top.min(last_row);
    let last = top
        .saturating_add(height)
        .min(total)
        .saturating_sub(1)
        .max(first);
    top.saturating_add(offset).clamp(first, last)
}

/// Celdas que ocupa la mitad izquierda de un grafema de `width` celdas. Siempre
/// al menos una: un clic sobre la celda de un grafema estrecho, o de anchura
/// cero, deja el cursor delante de él.
pub(super) fn left_half(width: usize) -> usize {
    width.div_ceil(2).max(1)
}

/// Índice de byte de la frontera de grafema que corresponde a la columna `col`
/// (en celdas) del tramo `text[start..end]`. Un clic en la mitad izquierda de un
/// grafema deja el cursor antes de él; en la derecha, después. Más allá del
/// tramo devuelve `end`.
pub(super) fn boundary_in_span(text: &str, start: usize, end: usize, col: usize) -> usize {
    let mut cells = 0;
    for (offset, grapheme) in text[start..end].grapheme_indices(true) {
        let width = UnicodeWidthStr::width(grapheme);
        if col < cells + left_half(width) {
            return start + offset;
        }
        cells += width;
    }
    end
}

/// Número de grafemas que preceden al byte `byte` de `text`. Usa la misma cuenta
/// que `Composer::cursor()`, de modo que los índices son intercambiables.
pub(super) fn grapheme_count(text: &str, byte: usize) -> usize {
    text[..byte].graphemes(true).count()
}
