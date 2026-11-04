//! Pegados largos colapsados: inserción, expansión y borrado atómico.

use super::*;

impl Composer {
    pub(super) fn shift_pastes_after(&mut self, from: usize, delta: isize) {
        for paste in &mut self.collapsed_pastes {
            if paste.start >= from {
                paste.start = paste.start.checked_add_signed(delta).unwrap();
                paste.end = paste.end.checked_add_signed(delta).unwrap();
            }
        }
    }

    pub(super) fn remove_pastes_in(&mut self, start: usize, end: usize) {
        self.collapsed_pastes
            .retain(|p| p.end <= start || p.start >= end);
    }

    pub(super) fn delete_paste(&mut self, index: usize) {
        let paste = self.collapsed_pastes.remove(index);
        self.buf.replace_range(paste.start..paste.end, "");
        self.shift_pastes_after(paste.end, paste.start as isize - paste.end as isize);
        self.cursor = paste.start;
        self.anchor = None;
        self.preferred_col = None;
    }

    pub(super) fn expand_paste(&mut self, index: usize) {
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

    pub(super) fn expand_paste_at_cursor(&mut self) {
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|p| p.start < self.cursor && self.cursor < p.end)
        {
            self.expand_paste(index);
        }
    }

    pub(super) fn expanded_text(&self) -> String {
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
