//! Selección de texto: rango, extensión por teclado y texto seleccionado.

use super::*;

impl Composer {
    /// Descarta la selección y deja el cursor en el inicio del rango borrado.
    /// Solo se invoca con selección activa (el llamador ya lo comprobó).
    pub(super) fn delete_selection(&mut self) {
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

    pub(super) fn expand_selection_over_pastes(
        &self,
        mut start: usize,
        mut end: usize,
    ) -> (usize, usize) {
        for paste in &self.collapsed_pastes {
            if paste.start < end && paste.end > start {
                start = start.min(paste.start);
                end = end.max(paste.end);
            }
        }
        (start, end)
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

    pub(super) fn snap_cursor_to_grapheme_end(&mut self) {
        if let Some((start, grapheme)) = self
            .buf
            .grapheme_indices(true)
            .find(|(start, grapheme)| *start < self.cursor && self.cursor < *start + grapheme.len())
        {
            self.cursor = start + grapheme.len();
        }
    }

    /// Amplía la selección con un paso de cursor: fija el ancla en la posición
    /// actual si no la había y, tras moverse, normaliza una selección vacía.
    pub(super) fn extend_with(&mut self, step: fn(&mut Self)) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor());
        }
        step(self);
        if self.anchor == Some(self.cursor()) {
            self.anchor = None;
        }
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
}
