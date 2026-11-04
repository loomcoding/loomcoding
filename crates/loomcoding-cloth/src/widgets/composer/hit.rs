//! Pruebas de impacto: de la celda pulsada al índice de grafema.

use super::*;

/// Traduce una celda a un índice en el composer Crush. Las filas visuales
/// visibles son `view_top..view_top + altura`, con `view_top` el valor que dejó
/// el último render (consulta pura: no lo recalcula ni lo escribe).
pub(super) fn hit_crush(composer: &Composer, area: Rect, x: u16, y: u16) -> usize {
    let text = composer.text();
    let layout = crush_layout_at(composer, composer.painted_cursor(), area.width);
    let index = visible_index(
        composer.view_top(),
        usize::from(area.height),
        layout.rows.len(),
        usize::from(y.saturating_sub(area.y)),
    );
    let row = &layout.rows[index];
    let gutter = UnicodeWidthStr::width(crush_gutter(row));
    let col = usize::from(x.saturating_sub(area.x)).saturating_sub(gutter);
    grapheme_count(text, boundary_in_span(text, row.start, row.end, col))
}

/// Traduce una celda a un índice en el composer Classic. Las líneas visibles son
/// `view_top..view_top + altura` en líneas lógicas; cada una muestra el tramo que
/// fijó `classic_windows`, tras un prefijo de `CLASSIC_PREFIX` columnas.
pub(super) fn hit_classic(composer: &Composer, inner: Rect, x: u16, y: u16) -> usize {
    let text = composer.text();
    let layout = classic_layout_at(
        composer,
        composer.painted_cursor(),
        inner.width.saturating_add(2),
    );
    let row = visible_index(
        composer.view_top(),
        usize::from(inner.height),
        layout.rows.len(),
        usize::from(y.saturating_sub(inner.y)),
    );
    let visual_row = &layout.rows[row];
    let col = usize::from(x.saturating_sub(inner.x)).saturating_sub(usize::from(CLASSIC_PREFIX));
    grapheme_count(
        text,
        boundary_in_span(text, visual_row.start, visual_row.end, col),
    )
}

/// Índice de grafema del composer bajo la celda `(x, y)` del terminal: la misma
/// unidad que `Composer::cursor()` y `select_range`. `area` es exactamente el
/// rectángulo que se pasó a `render_for_look`.
///
/// Geometría de cada look (debe coincidir con su renderer):
/// - **Crush**, sin marco. Fila visual `view_top + (y - area.y)`, acotada a las
///   filas que existen. Cada fila lleva su prefijo: `> ` o `::: ` en la primera
///   fila de la línea y sangría de igual anchura en las demás. Las celdas del
///   prefijo caen al inicio de la fila; las que quedan tras el texto, al final.
///   Al final de una fila partida por ajuste, el índice coincide con el inicio
///   de la siguiente, que es donde el render pinta el cursor. En una fila que
///   cierra un salto de línea lógico, el final queda antes del `\n`.
/// - **Classic**, marco redondeado de una celda por lado; el interior es
///   `classic_inner(area)`. Fila visual `view_top + (y - interior.y)`; las
///   líneas largas se ajustan al ancho disponible después del prefijo.
///
/// Un clic en la mitad derecha de un carácter ancho deja el cursor después de
/// él. Devuelve `None` solo si el área es nula o, en Classic, si no queda
/// interior; un composer vacío devuelve `Some(0)`.
pub fn hit_index(theme: &Theme, composer: &Composer, area: Rect, x: u16, y: u16) -> Option<usize> {
    let inner = match theme.look.composer {
        ComposerStyle::Classic => classic_inner(area),
        ComposerStyle::Crush => area,
    };
    if area.is_empty() || inner.is_empty() {
        return None;
    }
    if composer.is_empty() {
        return Some(0);
    }
    Some(match theme.look.composer {
        ComposerStyle::Classic => hit_classic(composer, inner, x, y),
        ComposerStyle::Crush => hit_crush(composer, area, x, y),
    })
}
