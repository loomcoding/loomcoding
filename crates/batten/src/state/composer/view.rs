//! Vista del composer: ventana visible, actividad, cursor pintado y medidas de línea.

use super::*;

impl Composer {
    /// Fila superior visible que recordó el último render.
    pub fn view_top(&self) -> usize {
        self.view_top.get()
    }

    /// Fija la fila superior visible. El render la acota a la altura real, así
    /// que un valor fuera de rango no provoca un desplazamiento incorrecto.
    pub fn set_view_top(&self, top: usize) {
        self.view_top.set(top);
    }

    /// Estilo de actividad que el composer pinta en su prompt mientras trabaja
    /// (vacío: el composer usa el suyo por defecto).
    pub fn activity_style(&self) -> &'static str {
        self.activity_style
    }

    /// Fija el estilo de actividad; se llama al empezar cada petición.
    pub fn set_activity_style(&mut self, style: &'static str) {
        self.activity_style = style;
    }

    /// Si el composer debe pintar su prompt como indicador de actividad.
    pub fn is_working(&self) -> bool {
        self.working.get()
    }

    /// Marca el composer como «trabajando» o no.
    pub fn set_working(&self, working: bool) {
        self.working.set(working);
    }

    /// Fotograma actual del indicador, para que el render lo anime.
    pub fn activity_frame(&self) -> u64 {
        self.activity_frame.get()
    }

    /// Fija el fotograma del indicador (el render lo hace en cada pasada).
    pub fn set_activity_frame(&self, frame: u64) {
        self.activity_frame.set(frame);
    }

    /// Recuerda que el render acaba de pintar con el cursor actual.
    pub fn mark_painted(&self) {
        self.painted.set(Some(self.cursor()));
    }

    /// Cursor (en grafemas) con el que se pintó por última vez. Sin ningún
    /// pintado previo, equivale al cursor actual.
    pub fn painted_cursor(&self) -> usize {
        self.painted.get().unwrap_or_else(|| self.cursor())
    }

    /// Fila lógica que contiene el grafema `pos` (los saltos de línea cuentan
    /// para la fila en la que quedan, como en `row_of_cursor`).
    pub fn row_of_grapheme(&self, pos: usize) -> usize {
        self.buf[..self.grapheme_to_byte(pos)].matches('\n').count()
    }

    /// Convierte un índice de grafema en índice de byte. Posiciones fuera de
    /// rango caen al final del buffer.
    pub(super) fn grapheme_to_byte(&self, grapheme: usize) -> usize {
        self.buf
            .grapheme_indices(true)
            .nth(grapheme)
            .map_or(self.buf.len(), |(i, _)| i)
    }

    /// Rangos de byte por línea lógica, sin incluir el salto.
    pub(super) fn line_spans(&self) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        let mut start = 0;
        for (i, _) in self.buf.match_indices('\n') {
            spans.push((start, i));
            start = i + 1;
        }
        spans.push((start, self.buf.len()));
        spans
    }

    pub(super) fn byte_at_column(&self, start: usize, end: usize, column: usize) -> usize {
        let mut width = 0;
        let mut byte = start;
        for (offset, grapheme) in self.buf[start..end].grapheme_indices(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            let next_width = width + grapheme_width;
            if next_width > column {
                break;
            }
            width = next_width;
            byte = start + offset + grapheme.len();
            if width == column {
                break;
            }
        }
        byte
    }

    pub(super) fn row_of_cursor(&self) -> usize {
        self.line_spans()
            .iter()
            .position(|(s, e)| self.cursor >= *s && self.cursor <= *e)
            .unwrap_or(0)
    }

    pub fn cursor(&self) -> usize {
        self.buf[..self.cursor].graphemes(true).count()
    }

    pub fn lines(&self) -> usize {
        self.line_spans().len()
    }

    /// (fila, columna en anchura de display) del cursor.
    pub fn cursor_row_col(&self) -> (usize, usize) {
        let spans = self.line_spans();
        let row = self.row_of_cursor();
        let (s, _) = spans[row];
        let col = UnicodeWidthStr::width(&self.buf[s..self.cursor]);
        (row, col)
    }
}
