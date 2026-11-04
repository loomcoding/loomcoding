//! Vista del transcript: ventana visible + scroll + wraps CJK-aware.

use crate::state::{
    CellPos, DiffKind, Line, LineKind, Selection, StyleToken, TextSelection, ToolStatus,
    Transcript, Viewport,
};
use crate::widgets::Theme;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line as TLine, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::Frame;
use std::num::NonZeroU16;
use unicode_width::UnicodeWidthStr;

mod layout;
mod lines;
mod links;
mod pawan;
mod selection;

pub use layout::Layout;
use lines::*;
use links::*;
pub use pawan::{render_pawan, render_pawan_waiting, PawanMessage, PawanRole};
pub use selection::{hit_cell, paint_text_selection, selected_text};

#[cfg(test)]
mod tests;

/// Pinta el transcript: solo las líneas visibles. `layout` cachea alturas;
/// `viewport` recibe la geometría y el ancla; `selection` se resalta y su
/// extremo se mantiene visible.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    transcript: &Transcript,
    viewport: &mut Viewport,
    layout: &mut Layout,
    selection: Option<&Selection>,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rows = area.height as usize;
    let prev_width = layout.width();
    // Ancho de texto: si ya se medía sin la columna de la scrollbar y sigue
    // desbordando, se mantiene (así no se invalida la caché cada frame).
    let narrow = area.width.saturating_sub(1).max(1);
    let text_w = if area.width >= 2 && prev_width == narrow {
        layout.sync(transcript, narrow, theme);
        if layout.total_rows() > rows {
            narrow
        } else {
            layout.sync(transcript, area.width, theme);
            area.width
        }
    } else {
        layout.sync(transcript, area.width, theme);
        if area.width >= 2 && layout.total_rows() > rows {
            layout.sync(transcript, narrow, theme);
            narrow
        } else {
            area.width
        }
    };
    let overflow = text_w < area.width;

    if prev_width != 0 && prev_width != text_w && !viewport.is_following() {
        if let Some((line, row)) = viewport.anchor() {
            let h = layout.height_of(line).max(1);
            viewport.restore_top(layout.row_of(line) + row.min(h - 1));
        }
    }
    viewport.set_geometry(layout.total_rows(), rows);
    if let (Some(s), false) = (selection, transcript.is_empty()) {
        let head = s.head().min(transcript.len() - 1);
        let start = layout.row_of(head);
        viewport.ensure_visible(start, start + layout.height_of(head));
    }

    let text_area = Rect {
        width: text_w,
        ..area
    };
    if let Some((first, skip)) = layout.line_at_row(viewport.top()) {
        viewport.set_anchor(first, skip);
        let sel_style = theme.styles.selection;
        let mut lines = Vec::new();
        let mut filled = 0;
        let mut i = first;
        while i < transcript.len() && filled < rows + skip {
            if let Some(line) = transcript.line(i) {
                let entry_start = transcript.locate(i).is_some_and(|(_, line)| line == 0);
                let mut out = render_line(line, theme, entry_start);
                if selection.is_some_and(|s| s.contains(i)) {
                    for tl in &mut out {
                        tl.style = tl.style.patch(sel_style);
                    }
                }
                lines.extend(out);
            }
            filled += layout.height_of(i);
            i += 1;
        }
        let skip = u16::try_from(skip).unwrap_or(u16::MAX);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((skip, 0)),
            text_area,
        );
        apply_links(
            frame.buffer_mut(),
            text_area,
            transcript,
            layout,
            viewport.top(),
            theme,
        );
    }
    if overflow {
        let bar = Rect {
            x: area.x + area.width - 1,
            width: 1,
            ..area
        };
        crate::widgets::scrollbar::render(frame, bar, viewport.scroll(), theme);
    }
}
