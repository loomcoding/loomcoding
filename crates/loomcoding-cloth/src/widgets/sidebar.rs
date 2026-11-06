//! Render del sidebar: logo (o compacto) + secciones.

use crate::state::{Sidebar, StyleToken};
use crate::widgets::logo::{self, Wordmark, FONT_ROWS};
use crate::widgets::loom_mark::{self, LOOM_MARK_ROWS, LOOM_MARK_WIDTH};
use crate::widgets::text::truncate;
use crate::widgets::Theme;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Altura mínima para el wordmark de bloques (wordmark + una línea en blanco).
pub const LOGO_MIN_HEIGHT: usize = FONT_ROWS + 2;

/// Altura mínima para la marca «LOOM» (cuatro filas + una línea en blanco).
pub const LOOM_MARK_MIN_HEIGHT: usize = LOOM_MARK_ROWS + 1;

pub fn render(frame: &mut Frame, area: Rect, sidebar: &Sidebar, theme: &Theme) {
    let block = Block::default().borders(Borders::LEFT);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = inner.width as usize;
    let height = inner.height as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(log) = &sidebar.logo {
        if log.mark {
            // La marca pedida nunca cae al wordmark: si no cabe, va el compacto.
            if width >= LOOM_MARK_WIDTH && height >= LOOM_MARK_MIN_HEIGHT {
                lines.extend(loom_mark::loom_mark(&log.name, width, theme));
            } else {
                lines.extend(logo::compact(&log.name, width, theme));
            }
        } else {
            let wm = Wordmark::new(&log.name);
            if height >= LOGO_MIN_HEIGHT && wm.width() <= width {
                lines.extend(logo::render(&wm, theme));
            } else {
                lines.extend(logo::compact(&log.name, width, theme));
            }
        }
        lines.push(Line::from(""));
    }

    for section in &sidebar.sections {
        lines.push(Line::from(Span::styled(
            section.title.clone(),
            theme.style(StyleToken::Heading),
        )));
        for l in &section.lines {
            lines.push(Line::from(Span::styled(
                truncate(l, width),
                theme.style(StyleToken::Default),
            )));
        }
        lines.push(Line::from(""));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Sidebar;
    use crate::widgets::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer(sb: &Sidebar, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), sb, &Theme::test()))
            .unwrap();
        term.backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn shows_block_logo_and_sections() {
        let mut sb = Sidebar::with_logo("cloth");
        sb.push_section("estado", vec!["working".into()]);
        let out = buffer(&sb, 32, 40);
        assert!(out.contains('█'));
        assert!(out.contains("estado"));
        assert!(out.contains("working"));
    }

    #[test]
    fn compact_logo_when_the_wordmark_does_not_fit() {
        let sb = Sidebar::with_logo("cloth");
        let out = buffer(&sb, 32, LOGO_MIN_HEIGHT as u16 - 1);
        assert!(out.contains("cloth"));
    }

    #[test]
    fn compact_logo_when_too_narrow_for_the_wordmark() {
        let sb = Sidebar::with_logo("cloth");
        let out = buffer(&sb, 8, 40);
        assert!(out.contains("cloth"));
    }

    #[test]
    fn truncates_long_lines_with_an_ellipsis() {
        let mut sb = Sidebar::new();
        sb.push_section("s", vec!["x".repeat(100)]);
        let out = buffer(&sb, 20, 10);
        assert!(out.contains('…'));
    }

    #[test]
    fn truncates_on_display_width_not_bytes() {
        // CJK es 2 columnas por carácter: truncar por bytes desbordaría.
        let mut sb = Sidebar::new();
        sb.push_section("s", vec!["星".repeat(50)]);
        let _ = buffer(&sb, 20, 10);
    }

    #[test]
    fn zero_and_tiny_sizes_do_not_panic() {
        let sb = Sidebar::with_logo("cloth");
        let _ = buffer(&sb, 0, 0);
        let _ = buffer(&sb, 1, 1);
        let _ = buffer(&sb, 1, 40);
    }

    /// Filas del buffer sin el borde izquierdo ni el relleno final.
    fn interior(sb: &Sidebar, w: u16, h: u16) -> Vec<String> {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), sb, &Theme::test()))
            .unwrap();
        let buf = term.backend().buffer();
        (0..h)
            .map(|y| {
                let fila: String = (1..w).map(|x| buf[(x, y)].symbol()).collect();
                fila.trim_end().to_string()
            })
            .collect()
    }

    #[test]
    fn sidebar_with_the_loom_mark_starts_with_the_block_letters() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        let filas = interior(&sb, 23, 20);
        assert!(filas[0].contains("█   ▄▀▀▄ ▄▀▀▄ █▀▄▀█"), "{filas:?}");
        assert!(filas[2].contains('╱'), "{filas:?}");
        assert!(filas[3].contains("loomcoding"), "{filas:?}");
    }

    #[test]
    fn sidebar_with_the_loom_mark_falls_back_to_compact_when_narrow() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        let out = buffer(&sb, 16, 40);
        assert!(!out.contains("▄▀▀▄"));
        assert!(out.contains("Loomcoding"));
    }

    #[test]
    fn sidebar_with_the_loom_mark_falls_back_to_compact_when_short() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        let out = buffer(&sb, 23, 4);
        assert!(!out.contains("▄▀▀▄"));
        assert!(out.contains("Loomcoding"));
    }

    #[test]
    fn plain_with_logo_is_unchanged() {
        let sb = Sidebar::with_logo("cloth");
        let filas = interior(&sb, 32, 40);
        let wordmark = Wordmark::new("cloth");
        for (i, fila) in wordmark.rows().iter().enumerate() {
            assert_eq!(filas[i], fila.trim_end(), "fila {i} del wordmark");
        }
        assert_eq!(filas[FONT_ROWS], "", "línea en blanco tras el logo");
    }
}
