//! Edición del texto: inserción, borrado, texto completo y envío.

use super::*;

impl Composer {
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
}
