//! Movimiento del cursor por grafemas, líneas y límites.

use super::*;

impl Composer {
    pub(super) fn step_left(&mut self) {
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

    pub(super) fn step_right(&mut self) {
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

    pub(super) fn step_home(&mut self) {
        let spans = self.line_spans();
        self.cursor = spans[self.row_of_cursor()].0;
        self.preferred_col = None;
    }

    pub(super) fn step_end(&mut self) {
        let spans = self.line_spans();
        self.cursor = spans[self.row_of_cursor()].1;
        self.preferred_col = None;
    }

    pub(super) fn step_up(&mut self) {
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

    pub(super) fn step_down(&mut self) {
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
}
