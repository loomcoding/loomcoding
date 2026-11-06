//! Render del sidebar: logo (o compacto) + secciones.

use crate::state::{Sidebar, StyleToken};
use crate::widgets::logo::{self, Wordmark, FONT_ROWS};
use crate::widgets::loom_mark::{self, LOOM_MARK_ROWS, LOOM_MARK_WIDTH};
use crate::widgets::text::truncate;
use crate::widgets::Theme;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
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
        // Cabe señalar que una sección sin líneas no se pinta: ni su título ni la fila en blanco.
        if section.lines.is_empty() {
            continue;
        }
        // Un título vacío es un bloque sin fila de título, como el primero del menú. Los títulos
        // van en negrita por encima del tema, que puede no declararla.
        if !section.title.is_empty() {
            lines.push(Line::from(Span::styled(
                section.title.clone(),
                theme
                    .style(StyleToken::Heading)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        for (indice, l) in section.lines.iter().enumerate() {
            let token = if section.error_lines.contains(&indice) {
                StyleToken::Error
            } else {
                StyleToken::Default
            };
            lines.push(Line::from(Span::styled(
                truncate(l, width),
                theme.style(token),
            )));
        }
        lines.push(Line::from(""));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Rectángulo exacto que ocupa la marca «LOOM» cuando `render` la pinta en `area`, o `None` si el
/// sidebar no lleva marca o no cabe (entonces `render` pinta el compacto, que no es marca).
///
/// Usa la misma geometría que `render`: la columna izquierda es el borde, y la marca, de 19×4
/// celdas, empieza en el interior. Como `render` solo la pinta si cabe, el rectángulo nunca sale
/// de `area`.
pub fn logo_rect(area: Rect, sidebar: &Sidebar) -> Option<Rect> {
    if !sidebar.logo.as_ref().is_some_and(|log| log.mark) {
        return None;
    }
    let inner = Block::default().borders(Borders::LEFT).inner(area);
    let cabe =
        inner.width as usize >= LOOM_MARK_WIDTH && inner.height as usize >= LOOM_MARK_MIN_HEIGHT;
    cabe.then(|| {
        Rect::new(
            inner.x,
            inner.y,
            LOOM_MARK_WIDTH as u16,
            LOOM_MARK_ROWS as u16,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Sidebar;
    use crate::widgets::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier, Style};
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

    /// Cabe señalar que el título va en negrita aunque el tema no la declare: un `[styles]`
    /// de TOML sustituye el estilo `heading` entero y pierde el `BOLD` por defecto. Se mira la
    /// celda de la primera letra del título.
    #[test]
    fn titles_are_bold() {
        let mut sb = Sidebar::new();
        sb.push_section("Actividad", vec!["x".into()]);
        let mut tema = Theme::test();
        tema.styles.heading = Style::default().fg(Color::Indexed(250));
        let mut term = Terminal::new(TestBackend::new(20, 4)).unwrap();
        term.draw(|f| render(f, f.area(), &sb, &tema)).unwrap();
        let buf = term.backend().buffer();
        // La columna 0 es el borde izquierdo; el título empieza en la 1.
        assert_eq!(buf[(1, 0)].symbol(), "A");
        assert!(
            buf[(1, 0)].modifier.contains(Modifier::BOLD),
            "el título debe ir en negrita: {:?}",
            buf[(1, 0)]
        );
    }

    /// Una sección sin líneas no pinta ni su título ni la fila en blanco que lo sigue.
    #[test]
    fn empty_sections_are_not_drawn() {
        let mut sb = Sidebar::new();
        sb.push_section("vacía", vec![]);
        sb.push_section("modelo", vec!["mock".into()]);
        let filas = interior(&sb, 20, 6);
        assert_eq!(filas[0], "modelo", "{filas:?}");
        assert_eq!(filas[1], "mock", "{filas:?}");
        assert_eq!(filas[2], "", "{filas:?}");
        assert!(!filas.iter().any(|f| f.contains("vacía")), "{filas:?}");
    }

    /// Un título vacío es el primer bloque del menú: sus líneas van sin fila de título.
    #[test]
    fn an_empty_title_draws_no_title_row() {
        let mut sb = Sidebar::new();
        sb.push_section("", vec!["x".into()]);
        let filas = interior(&sb, 20, 4);
        assert_eq!(filas[0], "x", "{filas:?}");
        assert_eq!(filas[1], "", "{filas:?}");
    }

    /// Pinta el sidebar en `area` de un terminal de 40×24 y devuelve el buffer completo.
    fn pintado(sb: &Sidebar, area: Rect) -> ratatui::buffer::Buffer {
        let mut term = Terminal::new(TestBackend::new(40, 24)).unwrap();
        term.draw(|f| render(f, area, sb, &Theme::test())).unwrap();
        term.backend().buffer().clone()
    }

    /// Cabe señalar que el rectángulo devuelto se comprueba contra lo que pinta `render`: el
    /// primer glifo `█` de la marca cae dentro, en la esquina superior izquierda.
    #[test]
    fn logo_rect_marca_el_sitio_real_de_la_marca() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        let area = Rect::new(0, 0, 23, 20);
        let rect = logo_rect(area, &sb).expect("con marca y sitio hay rectángulo");
        assert_eq!(rect, Rect::new(1, 0, 19, 4));
        let buf = pintado(&sb, area);
        assert_eq!(buf[(rect.x, rect.y)].symbol(), "█");
        assert!(rect.contains(ratatui::layout::Position::new(rect.x, rect.y)));
        let fila: String = (rect.x..rect.x + rect.width)
            .map(|x| buf[(x, rect.y)].symbol())
            .collect();
        assert!(fila.starts_with("█   ▄▀▀▄"), "fila 0 de la marca: {fila:?}");
    }

    #[test]
    fn logo_rect_sigue_el_desplazamiento_del_area() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        let rect = logo_rect(Rect::new(10, 5, 23, 20), &sb).unwrap();
        assert_eq!(rect, Rect::new(11, 5, 19, 4));
        let buf = pintado(&sb, Rect::new(10, 5, 23, 20));
        assert_eq!(buf[(rect.x, rect.y)].symbol(), "█");
    }

    #[test]
    fn logo_rect_es_none_sin_marca() {
        let area = Rect::new(0, 0, 23, 20);
        assert_eq!(logo_rect(area, &Sidebar::new()), None);
        assert_eq!(logo_rect(area, &Sidebar::with_logo("cloth")), None);
    }

    #[test]
    fn logo_rect_es_none_si_no_cabe_y_cae_al_compacto() {
        let sb = Sidebar::with_loom_mark("Loomcoding");
        assert_eq!(logo_rect(Rect::new(0, 0, 16, 40), &sb), None);
        assert_eq!(logo_rect(Rect::new(0, 0, 23, 4), &sb), None);
        // Con el compacto no hay marca: no se devuelve rectángulo.
        let out = interior(&sb, 16, 40).join("\n");
        assert!(!out.contains("▄▀▀▄"));
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
