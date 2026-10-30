//! Composer: buffer multilínea con cursor. Puro (sin ratatui).

use std::cell::Cell;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Invariante: `anchor` es `None` o distinto del cursor (una selección vacía
/// se normaliza a `None`), y se expresa en grafemas, igual que `cursor()`.
/// Toda operación que altere el texto la limpia, así que nunca apunta fuera
/// de rango.
///
/// `view_top` es estado de vista, no de edición: la fila superior visible que
/// dejó el último render (filas visuales en Crush, líneas lógicas en Classic).
/// Va en una `Cell` para que el render la actualice con `&Composer`.
///
/// `painted` guarda el cursor con el que se pintó por última vez. La traducción
/// de una celda a un índice (`hit_index`) debe usar esa misma geometría y no el
/// cursor vivo: durante un arrastre el cursor se mueve antes del siguiente repintado.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CollapsedPaste {
    start: usize,
    end: usize,
    text: String,
}

#[derive(Debug, Default)]
pub struct Composer {
    buf: String,
    collapsed_pastes: Vec<CollapsedPaste>,
    cursor: usize,
    preferred_col: Option<usize>,
    anchor: Option<usize>,
    view_top: Cell<usize>,
    painted: Cell<Option<usize>>,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fila superior visible que recordó el último render.
    pub fn view_top(&self) -> usize {
        self.view_top.get()
    }

    /// Fija la fila superior visible. El render la acota a la altura real, así
    /// que un valor fuera de rango no provoca un desplazamiento incorrecto.
    pub fn set_view_top(&self, top: usize) {
        self.view_top.set(top);
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
    fn grapheme_to_byte(&self, grapheme: usize) -> usize {
        self.buf
            .grapheme_indices(true)
            .nth(grapheme)
            .map_or(self.buf.len(), |(i, _)| i)
    }

    /// Descarta la selección y deja el cursor en el inicio del rango borrado.
    /// Solo se invoca con selección activa (el llamador ya lo comprobó).
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection_bytes() {
            let (start, end) = self.expand_selection_over_pastes(start, end);
            self.buf.replace_range(start..end, "");
            self.remove_pastes_in(start, end);
            self.shift_pastes_after(end, start as isize - end as isize);
            self.cursor = start;
        }
        self.anchor = None;
        self.preferred_col = None;
    }

    /// Rango ordenado `[inicio, fin)` en bytes, o `None` sin selección.
    pub fn selection_bytes(&self) -> Option<(usize, usize)> {
        let anchor = self.grapheme_to_byte(self.anchor?);
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Ancla de la selección en grafemas, o `None` si no hay selección activa.
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// Rango ordenado `[inicio, fin)` en grafemas, o `None` sin selección.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let head = self.cursor();
        if anchor == head {
            return None;
        }
        Some((anchor.min(head), anchor.max(head)))
    }

    /// Texto seleccionado, o `None` sin selección.
    pub fn selected_text(&self) -> Option<&str> {
        self.selection_bytes().map(|(s, e)| &self.buf[s..e])
    }

    /// Selección con los pegados colapsados expandidos a su contenido real.
    pub fn selected_text_expanded(&self) -> Option<String> {
        let (mut start, mut end) = self.selection_bytes()?;
        (start, end) = self.expand_selection_over_pastes(start, end);
        let mut result = String::new();
        let mut cursor = start;
        for paste in self
            .collapsed_pastes
            .iter()
            .filter(|p| p.start < end && p.end > start)
        {
            result.push_str(&self.buf[cursor..paste.start]);
            result.push_str(&paste.text);
            cursor = paste.end;
        }
        result.push_str(&self.buf[cursor..end]);
        Some(result)
    }

    fn expand_selection_over_pastes(&self, mut start: usize, mut end: usize) -> (usize, usize) {
        for paste in &self.collapsed_pastes {
            if paste.start < end && paste.end > start {
                start = start.min(paste.start);
                end = end.max(paste.end);
            }
        }
        (start, end)
    }

    fn shift_pastes_after(&mut self, from: usize, delta: isize) {
        for paste in &mut self.collapsed_pastes {
            if paste.start >= from {
                paste.start = paste.start.checked_add_signed(delta).unwrap();
                paste.end = paste.end.checked_add_signed(delta).unwrap();
            }
        }
    }

    fn remove_pastes_in(&mut self, start: usize, end: usize) {
        self.collapsed_pastes
            .retain(|p| p.end <= start || p.start >= end);
    }

    fn delete_paste(&mut self, index: usize) {
        let paste = self.collapsed_pastes.remove(index);
        self.buf.replace_range(paste.start..paste.end, "");
        self.shift_pastes_after(paste.end, paste.start as isize - paste.end as isize);
        self.cursor = paste.start;
        self.anchor = None;
        self.preferred_col = None;
    }

    fn expand_paste(&mut self, index: usize) {
        let paste = self.collapsed_pastes.remove(index);
        let old_len = paste.end - paste.start;
        self.buf.replace_range(paste.start..paste.end, &paste.text);
        let new_len = paste.text.len();
        self.shift_pastes_after(paste.end, new_len as isize - old_len as isize);
        if self.cursor >= paste.end {
            self.cursor = self.cursor - old_len + new_len;
        } else if self.cursor > paste.start {
            self.cursor = paste.start + new_len;
        }
    }

    /// Fija ancla y cursor (ambos acotados al nº de grafemas). Si coinciden,
    /// no hay selección.
    pub fn select_range(&mut self, anchor: usize, head: usize) {
        let total = self.buf.graphemes(true).count();
        let anchor = anchor.min(total);
        let head = head.min(total);
        self.cursor = self.grapheme_to_byte(head);
        self.anchor = (anchor != head).then_some(anchor);
        self.preferred_col = None;
    }

    /// Mueve el cursor (acotado) y limpia la selección.
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = self.grapheme_to_byte(pos);
        self.anchor = None;
        self.preferred_col = None;
    }

    /// Selecciona todo el buffer, con el cursor al final.
    pub fn select_all(&mut self) {
        let total = self.buf.graphemes(true).count();
        self.select_range(0, total);
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Rangos de byte por línea lógica, sin incluir el salto.
    fn line_spans(&self) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        let mut start = 0;
        for (i, _) in self.buf.match_indices('\n') {
            spans.push((start, i));
            start = i + 1;
        }
        spans.push((start, self.buf.len()));
        spans
    }

    fn snap_cursor_to_grapheme_end(&mut self) {
        if let Some((start, grapheme)) = self
            .buf
            .grapheme_indices(true)
            .find(|(start, grapheme)| *start < self.cursor && self.cursor < *start + grapheme.len())
        {
            self.cursor = start + grapheme.len();
        }
    }

    fn byte_at_column(&self, start: usize, end: usize, column: usize) -> usize {
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

    fn row_of_cursor(&self) -> usize {
        self.line_spans()
            .iter()
            .position(|(s, e)| self.cursor >= *s && self.cursor <= *e)
            .unwrap_or(0)
    }

    pub fn insert_char(&mut self, c: char) {
        if c == '\r' {
            self.newline();
            return;
        }
        if self.anchor.is_some() {
            self.delete_selection();
        }
        self.expand_paste_at_cursor();
        self.shift_pastes_after(self.cursor, c.len_utf8() as isize);
        self.buf.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.snap_cursor_to_grapheme_end();
        self.preferred_col = None;
    }

    pub fn newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn insert_str(&mut self, s: &str) {
        if self.anchor.is_some() {
            self.delete_selection();
        }
        let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
        self.expand_paste_at_cursor();
        self.shift_pastes_after(self.cursor, normalized.len() as isize);
        self.buf.insert_str(self.cursor, &normalized);
        self.cursor += normalized.len();
        self.snap_cursor_to_grapheme_end();
        self.preferred_col = None;
    }

    /// Inserta un pegado largo como una unidad visible y conserva su payload.
    pub fn insert_paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if text.chars().count() <= 1000 {
            self.insert_str(&text);
            return;
        }
        if self.anchor.is_some() {
            self.delete_selection();
        }
        self.expand_paste_at_cursor();
        let label = format!("[Pasted Content {} chars]", text.chars().count());
        let start = self.cursor;
        self.shift_pastes_after(start, label.len() as isize);
        self.buf.insert_str(start, &label);
        self.cursor += label.len();
        self.collapsed_pastes.push(CollapsedPaste {
            start,
            end: start + label.len(),
            text,
        });
        self.collapsed_pastes.sort_by_key(|paste| paste.start);
        self.preferred_col = None;
    }

    fn expand_paste_at_cursor(&mut self) {
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|p| p.start < self.cursor && self.cursor < p.end)
        {
            self.expand_paste(index);
        }
    }

    pub fn backspace(&mut self) {
        if self.anchor.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|p| p.end == self.cursor)
        {
            self.delete_paste(index);
            return;
        }
        self.expand_paste_at_cursor();
        let (start, grapheme) = self
            .buf
            .grapheme_indices(true)
            .take_while(|(start, grapheme)| *start + grapheme.len() <= self.cursor)
            .last()
            .unwrap();
        let end = start + grapheme.len();
        self.buf.replace_range(start..end, "");
        self.shift_pastes_after(end, start as isize - end as isize);
        self.cursor = start;
        self.preferred_col = None;
    }

    pub fn delete(&mut self) {
        if self.anchor.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor >= self.buf.len() {
            return;
        }
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|p| p.start == self.cursor)
        {
            self.delete_paste(index);
            return;
        }
        self.expand_paste_at_cursor();
        let start = self.cursor;
        let end = start + self.buf[start..].graphemes(true).next().unwrap().len();
        self.buf.replace_range(start..end, "");
        self.shift_pastes_after(end, start as isize - end as isize);
        self.preferred_col = None;
    }

    // Pasos de cursor compartidos por los movimientos normales (que limpian la
    // selección) y los `extend_*` (que la amplían). Así ambos se comportan igual.

    fn step_left(&mut self) {
        if let Some(paste) = self
            .collapsed_pastes
            .iter()
            .find(|p| p.start < self.cursor && self.cursor <= p.end)
        {
            self.cursor = paste.start;
            self.preferred_col = None;
            return;
        }
        self.cursor = self.buf[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(start, _)| start);
        self.preferred_col = None;
    }

    fn step_right(&mut self) {
        if let Some(paste) = self
            .collapsed_pastes
            .iter()
            .find(|p| p.start <= self.cursor && self.cursor < p.end)
        {
            self.cursor = paste.end;
            self.preferred_col = None;
            return;
        }
        if self.cursor < self.buf.len() {
            self.cursor += self.buf[self.cursor..]
                .graphemes(true)
                .next()
                .unwrap()
                .len();
        }
        self.preferred_col = None;
    }

    fn step_home(&mut self) {
        let spans = self.line_spans();
        self.cursor = spans[self.row_of_cursor()].0;
        self.preferred_col = None;
    }

    fn step_end(&mut self) {
        let spans = self.line_spans();
        self.cursor = spans[self.row_of_cursor()].1;
        self.preferred_col = None;
    }

    fn step_up(&mut self) {
        let spans = self.line_spans();
        let row = self.row_of_cursor();
        if row == 0 {
            return;
        }
        let (start, _) = spans[row];
        let col = self
            .preferred_col
            .unwrap_or(UnicodeWidthStr::width(&self.buf[start..self.cursor]));
        let (s, e) = spans[row - 1];
        self.cursor = self.byte_at_column(s, e, col);
        self.preferred_col = Some(col);
    }

    fn step_down(&mut self) {
        let spans = self.line_spans();
        let row = self.row_of_cursor();
        if row + 1 >= spans.len() {
            return;
        }
        let (start, _) = spans[row];
        let col = self
            .preferred_col
            .unwrap_or(UnicodeWidthStr::width(&self.buf[start..self.cursor]));
        let (s, e) = spans[row + 1];
        self.cursor = self.byte_at_column(s, e, col);
        self.preferred_col = Some(col);
    }

    /// Amplía la selección con un paso de cursor: fija el ancla en la posición
    /// actual si no la había y, tras moverse, normaliza una selección vacía.
    fn extend_with(&mut self, step: fn(&mut Self)) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor());
        }
        step(self);
        if self.anchor == Some(self.cursor()) {
            self.anchor = None;
        }
    }

    pub fn move_left(&mut self) {
        self.anchor = None;
        self.step_left();
    }

    pub fn move_right(&mut self) {
        self.anchor = None;
        self.step_right();
    }

    pub fn move_home(&mut self) {
        self.anchor = None;
        self.step_home();
    }

    pub fn move_end(&mut self) {
        self.anchor = None;
        self.step_end();
    }

    pub fn move_up(&mut self) {
        self.anchor = None;
        self.step_up();
    }

    pub fn move_down(&mut self) {
        self.anchor = None;
        self.step_down();
    }

    pub fn extend_left(&mut self) {
        self.extend_with(Self::step_left);
    }

    pub fn extend_right(&mut self) {
        self.extend_with(Self::step_right);
    }

    pub fn extend_home(&mut self) {
        self.extend_with(Self::step_home);
    }

    pub fn extend_end(&mut self) {
        self.extend_with(Self::step_end);
    }

    pub fn extend_up(&mut self) {
        self.extend_with(Self::step_up);
    }

    pub fn extend_down(&mut self) {
        self.extend_with(Self::step_down);
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

    pub fn set_text(&mut self, s: &str) {
        self.buf = s.replace("\r\n", "\n").replace('\r', "\n");
        self.collapsed_pastes.clear();
        self.cursor = self.buf.len();
        self.preferred_col = None;
        self.anchor = None;
    }

    pub fn push_char(&mut self, c: char) {
        self.insert_char(c);
    }

    pub fn text(&self) -> &str {
        &self.buf
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Devuelve el texto y limpia el buffer. Vacío o solo-espacios → `None`
    /// (y también limpia, para no dejar basura).
    pub fn submit(&mut self) -> Option<String> {
        self.cursor = 0;
        self.preferred_col = None;
        self.anchor = None;
        if self.expanded_text().trim().is_empty() {
            self.buf.clear();
            self.collapsed_pastes.clear();
            return None;
        }
        let text = self.expanded_text();
        self.buf.clear();
        self.collapsed_pastes.clear();
        Some(text)
    }

    fn expanded_text(&self) -> String {
        let mut result = String::new();
        let mut cursor = 0;
        for paste in &self.collapsed_pastes {
            result.push_str(&self.buf[cursor..paste.start]);
            result.push_str(&paste.text);
            cursor = paste.end;
        }
        result.push_str(&self.buf[cursor..]);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_and_submits() {
        let mut c = Composer::new();
        c.push_char('h');
        c.push_char('i');
        assert_eq!(c.text(), "hi");
        assert_eq!(c.submit(), Some("hi".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.submit(), None);
    }

    #[test]
    fn whitespace_only_is_not_submitted() {
        let mut c = Composer::new();
        c.push_char(' ');
        c.newline();
        assert_eq!(c.submit(), None);
        assert_eq!(c.text(), "");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut c = Composer::new();
        c.push_char('a');
        c.push_char('b');
        c.backspace();
        assert_eq!(c.text(), "a");
        c.backspace();
        c.backspace();
        assert!(c.is_empty());
    }

    #[test]
    fn newline_inserts_newline() {
        let mut c = Composer::new();
        c.push_char('a');
        c.newline();
        c.push_char('b');
        assert_eq!(c.text(), "a\nb");
    }

    #[test]
    fn inserts_in_the_middle() {
        let mut c = Composer::new();
        c.insert_str("hl");
        c.move_left();
        c.insert_char('e');
        assert_eq!(c.text(), "hel");
        assert_eq!(c.cursor(), 2);
    }

    #[test]
    fn backspace_and_delete_around_the_cursor() {
        let mut c = Composer::new();
        c.insert_str("abc");
        c.move_left();
        c.move_left(); // cursor en 1
        c.backspace();
        assert_eq!(c.text(), "bc");
        c.delete();
        assert_eq!(c.text(), "c");
    }

    #[test]
    fn home_and_end_go_to_line_bounds() {
        let mut c = Composer::new();
        c.insert_str("uno\ndos");
        c.move_home();
        assert_eq!(c.cursor(), 4);
        c.move_end();
        assert_eq!(c.cursor(), 7);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let mut c = Composer::new();
        c.insert_str("aaaa\nb");
        c.move_end(); // final de "b" (indice 6)
        c.move_up();
        assert_eq!(c.cursor_row_col(), (0, 1));
        c.move_down();
        assert_eq!(c.cursor_row_col(), (1, 1));
    }

    #[test]
    fn cursor_row_col_counts_display_width() {
        let mut c = Composer::new();
        c.insert_str("星x");
        assert_eq!(c.cursor_row_col(), (0, 3));
    }

    #[test]
    fn handles_accented_input_without_corrupting() {
        let mut c = Composer::new();
        c.insert_str("áé");
        c.move_left();
        c.insert_char('x');
        assert_eq!(c.text(), "áxé");
    }

    #[test]
    fn lines_counts_logical_lines() {
        let mut c = Composer::new();
        assert_eq!(c.lines(), 1);
        c.insert_str("a\nb\nc");
        assert_eq!(c.lines(), 3);
    }

    #[test]
    fn set_text_puts_the_cursor_at_the_end() {
        let mut c = Composer::new();
        c.insert_str("old");
        c.set_text("new\nline");
        assert_eq!(c.text(), "new\nline");
        assert_eq!(c.cursor(), 8);
    }

    #[test]
    fn cursor_and_deletion_keep_combining_and_emoji_graphemes_intact() {
        let mut c = Composer::new();
        c.insert_str("e\u{301}👩‍💻");
        assert_eq!(c.cursor(), 2);
        c.move_left();
        c.backspace();
        assert_eq!(c.text(), "👩‍💻");
        assert_eq!(c.cursor_row_col(), (0, 0));
    }

    #[test]
    fn vertical_cursor_keeps_display_column_with_wide_graphemes() {
        let mut c = Composer::new();
        c.set_text("星x\n1234");
        c.move_up();
        assert_eq!(c.cursor_row_col(), (0, 3));
    }

    #[test]
    fn normalizes_windows_line_endings_before_editing() {
        let mut c = Composer::new();
        c.insert_str("first\r\nsecond");
        assert_eq!(c.text(), "first\nsecond");
        c.set_text("first\r\nsecond");
        assert_eq!(c.text(), "first\nsecond");
        c.set_text("\r\n");
        assert_eq!(c.text(), "\n");
        c.move_end();
        c.move_up();
        c.move_down();
        c.backspace();
        assert!(c.is_empty());
    }

    /// Familia ZWJ: un único grafema formado por cinco codepoints y 18 bytes.
    const FAMILIA: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";

    #[test]
    fn extend_right_selects_graphemes() {
        let mut c = Composer::new();
        c.insert_str(&format!("ae\u{301}{FAMILIA}b"));
        c.set_cursor(0);
        c.extend_right();
        c.extend_right();
        c.extend_right();
        assert_eq!(c.anchor(), Some(0));
        assert_eq!(c.cursor(), 3);
        assert_eq!(c.selection(), Some((0, 3)));
        let esperado = format!("ae\u{301}{FAMILIA}");
        assert_eq!(c.selected_text(), Some(esperado.as_str()));
    }

    #[test]
    fn extend_then_move_clears_selection() {
        let mut c = Composer::new();
        c.insert_str("hola");
        c.set_cursor(0);
        c.extend_right();
        c.extend_right();
        assert_eq!(c.selection(), Some((0, 2)));
        c.move_right();
        assert_eq!(c.anchor(), None);
        assert_eq!(c.selection(), None);
        assert_eq!(c.cursor(), 3);
    }

    #[test]
    fn selection_is_ordered_regardless_of_direction() {
        let mut c = Composer::new();
        c.insert_str("abcd");
        c.extend_left();
        c.extend_left();
        assert_eq!(c.anchor(), Some(4));
        assert_eq!(c.selection(), Some((2, 4)));
        assert_eq!(c.selected_text(), Some("cd"));
        c.select_range(1, 3);
        assert_eq!(c.selection(), Some((1, 3)));
        c.select_range(3, 1);
        assert_eq!(c.selection(), Some((1, 3)));
        assert_eq!(c.selected_text(), Some("bc"));
        c.set_cursor(2);
        c.extend_home();
        assert_eq!(c.selection(), Some((0, 2)));
        c.set_cursor(2);
        c.extend_end();
        assert_eq!(c.selection(), Some((2, 4)));
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut c = Composer::new();
        c.insert_str("hola mundo");
        c.select_range(0, 4);
        c.insert_char('X');
        assert_eq!(c.text(), "X mundo");
        assert_eq!(c.cursor(), 1);
        assert_eq!(c.selection(), None);
    }

    #[test]
    fn backspace_and_delete_remove_only_the_selection() {
        let mut c = Composer::new();
        c.set_text("abcdef");
        c.select_range(1, 3);
        c.backspace();
        assert_eq!(c.text(), "adef");
        assert_eq!(c.cursor(), 1);
        assert_eq!(c.selection(), None);

        c.set_text("abcdef");
        c.select_range(2, 4);
        c.delete();
        assert_eq!(c.text(), "abef");
        assert_eq!(c.cursor(), 2);

        c.set_text("abcdef");
        c.select_range(4, 1);
        c.backspace();
        assert_eq!(c.text(), "aef");
        assert_eq!(c.cursor(), 1);
    }

    #[test]
    fn paste_insert_str_replaces_the_selection() {
        let mut c = Composer::new();
        c.insert_str("hola mundo");
        c.select_range(5, 10);
        c.insert_str("tierra\r\nnueva");
        assert_eq!(c.text(), "hola tierra\nnueva");
        assert_eq!(c.cursor(), 17);
        assert_eq!(c.selection(), None);
    }

    #[test]
    fn newline_replaces_the_selection() {
        let mut c = Composer::new();
        c.insert_str("ab cd");
        c.select_range(2, 3);
        c.newline();
        assert_eq!(c.text(), "ab\ncd");
        assert_eq!(c.cursor(), 3);
    }

    #[test]
    fn set_text_and_submit_clear_the_selection() {
        let mut c = Composer::new();
        c.insert_str("abc");
        c.select_range(0, 2);
        c.set_text("xyz");
        assert_eq!(c.anchor(), None);
        assert_eq!(c.selection(), None);
        assert_eq!(c.text(), "xyz");
        c.select_range(0, 2);
        assert_eq!(c.submit(), Some("xyz".to_string()));
        assert_eq!(c.anchor(), None);
        assert_eq!(c.selection(), None);
    }

    #[test]
    fn select_range_clamps_out_of_range_positions() {
        let mut c = Composer::new();
        c.insert_str("abc");
        c.select_range(1, 99);
        assert_eq!(c.selection(), Some((1, 3)));
        assert_eq!(c.cursor(), 3);
        c.select_range(99, 99);
        assert_eq!(c.selection(), None);
        assert_eq!(c.cursor(), 3);
        c.select_range(50, 1);
        assert_eq!(c.anchor(), Some(3));
        assert_eq!(c.selection(), Some((1, 3)));
    }

    #[test]
    fn selection_never_splits_a_grapheme() {
        let mut c = Composer::new();
        let texto = format!("e\u{301}{FAMILIA}\u{661F}x");
        c.set_text(&texto);
        c.set_cursor(0);
        c.extend_right();
        assert_eq!(c.selection(), Some((0, 1)));
        assert_eq!(c.selected_text(), Some("e\u{301}"));
        c.extend_right();
        c.extend_right();
        assert_eq!(c.selection(), Some((0, 3)));
        let esperado = format!("e\u{301}{FAMILIA}\u{661F}");
        assert_eq!(c.selected_text(), Some(esperado.as_str()));
        c.select_range(2, 3);
        assert_eq!(c.selected_text(), Some("\u{661F}"));
    }

    #[test]
    fn selected_text_spans_multiple_lines() {
        let mut c = Composer::new();
        c.set_text("uno\ndos\ntres");
        c.select_range(2, 9);
        assert_eq!(c.selected_text(), Some("o\ndos\nt"));
        c.set_cursor(2);
        c.extend_down();
        assert_eq!(c.selection(), Some((2, 6)));
        assert_eq!(c.selected_text(), Some("o\ndo"));
    }

    #[test]
    fn selection_bytes_matches_selection() {
        let mut c = Composer::new();
        let texto = format!("e\u{301}{FAMILIA}\u{661F}");
        c.set_text(&texto);
        c.select_range(1, 3);
        assert_eq!(c.selection(), Some((1, 3)));
        assert_eq!(c.selection_bytes(), Some((3, texto.len())));
        let esperado = format!("{FAMILIA}\u{661F}");
        assert_eq!(c.selected_text(), Some(esperado.as_str()));
    }

    #[test]
    fn extend_up_down_follow_preferred_column() {
        let mut c = Composer::new();
        c.set_text("abcdef\nxy\nabcdef");
        c.set_cursor(3);
        c.extend_down();
        assert_eq!(c.selection(), Some((3, 9)));
        assert_eq!(c.selected_text(), Some("def\nxy"));
        // La columna preferida (3) se conserva al pasar por una línea corta.
        c.extend_down();
        assert_eq!(c.selection(), Some((3, 13)));
        assert_eq!(c.selected_text(), Some("def\nxy\nabc"));
    }

    #[test]
    fn select_all_selects_everything() {
        let mut c = Composer::new();
        c.select_all();
        assert_eq!(c.anchor(), None);
        assert_eq!(c.selection(), None);
        c.set_text("hola\nmundo");
        c.select_all();
        assert_eq!(c.anchor(), Some(0));
        assert_eq!(c.selection(), Some((0, 10)));
        assert_eq!(c.selected_text(), Some("hola\nmundo"));
        c.clear_selection();
        assert_eq!(c.selection(), None);
        assert_eq!(c.cursor(), 10);
    }

    #[test]
    fn no_selection_leaves_existing_behaviour_untouched() {
        let mut c = Composer::new();
        c.push_char('a');
        c.insert_str("bc");
        c.move_left();
        c.insert_char('X');
        c.newline();
        assert_eq!(c.text(), "abX\nc");
        c.move_up();
        c.move_end();
        c.move_down();
        c.move_left();
        c.move_right();
        c.move_home();
        c.move_end();
        c.backspace();
        c.backspace();
        c.delete();
        c.move_left();
        c.delete();
        assert_eq!(c.text(), "ab");
        assert_eq!(c.cursor(), 2);
        c.insert_str("\r\nz");
        c.move_home();
        c.move_up();
        c.move_down();
        assert_eq!(c.text(), "ab\nz");
        assert_eq!(c.cursor(), 3);
        assert_eq!(c.cursor_row_col(), (1, 0));
        assert_eq!(c.selection(), None);
        assert_eq!(c.submit(), Some("ab\nz".to_string()));
    }

    #[test]
    fn long_pastes_collapse_and_expand_on_submit() {
        let mut c = Composer::new();
        let pasted = "é".repeat(1001);
        c.insert_paste(&pasted);
        assert_eq!(c.text(), "[Pasted Content 1001 chars]");
        assert_eq!(c.submit(), Some(pasted));
    }

    #[test]
    fn pasted_marker_text_is_literal_and_deletion_is_atomic() {
        let mut c = Composer::new();
        let pasted = "x".repeat(1001);
        c.insert_paste(&pasted);
        c.backspace();
        assert_eq!(c.text(), "");
        c.insert_str("[Pasted Content 1001 chars]");
        assert_eq!(c.submit(), Some("[Pasted Content 1001 chars]".to_string()));

        let mut c = Composer::new();
        c.insert_str("before");
        c.insert_paste(&pasted);
        c.insert_str("after");
        c.set_cursor(6);
        c.delete();
        assert_eq!(c.submit(), Some("beforeafter".to_string()));
    }

    #[test]
    fn multiple_pastes_keep_order_and_copy_expanded_content() {
        let mut c = Composer::new();
        let first = "a".repeat(1001);
        let second = "b".repeat(1002);
        c.insert_paste(&first);
        c.insert_str("+");
        c.insert_paste(&second);
        c.select_all();
        let copied = c.selected_text_expanded().unwrap();
        assert_eq!(copied, format!("{first}+{second}"));
        assert_eq!(c.submit(), Some(copied));
    }
}
