//! Piezas comunes de los tests.

use super::*;

pub(super) fn draw(
    transcript: &Transcript,
    vp: &mut Viewport,
    layout: &mut Layout,
    w: u16,
    h: u16,
) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, f.area(), transcript, vp, layout, None, &Theme::test()))
        .unwrap();
    term.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

pub(super) fn buffer(transcript: &Transcript, w: u16, h: u16) -> String {
    draw(transcript, &mut Viewport::new(), &mut Layout::new(), w, h)
}

pub(super) fn long_transcript(n: usize) -> (Transcript, crate::state::EntryId) {
    let mut t = Transcript::new();
    for i in 0..n {
        t.push_entry(
            LineKind::Assistant,
            crate::state::Format::Plain,
            format!("l{i:05}"),
        );
    }
    let last = t.push_entry(LineKind::Assistant, crate::state::Format::Plain, "fin");
    (t, last)
}

/// Buffer del frame (no el del backend: `TestBackend` solo copia las
/// celdas que emite el diff, y las `Skip` no se emiten).
pub(super) fn link_buffer(text: &str, w: u16, h: u16, theme: &Theme) -> Buffer {
    let mut t = Transcript::new();
    t.push_entry(LineKind::Assistant, crate::state::Format::Markdown, text);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    let (mut vp, mut layout) = (Viewport::new(), Layout::new());
    let mut frame_buf = Buffer::empty(Rect::new(0, 0, w, h));
    term.draw(|f| {
        render(f, f.area(), &t, &mut vp, &mut layout, None, theme);
        frame_buf = f.buffer_mut().clone();
    })
    .unwrap();
    frame_buf
}

pub(super) fn banners(texts: &[&str]) -> Transcript {
    let mut t = Transcript::new();
    for s in texts {
        t.push(Line::new(LineKind::Banner, *s));
    }
    t
}

pub(super) fn select(from: CellPos, to: CellPos) -> TextSelection {
    let mut sel = TextSelection::new(from);
    sel.set_head(to);
    sel
}
