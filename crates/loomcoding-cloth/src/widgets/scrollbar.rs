//! Barra de scroll vertical para una vista con `Viewport`, al estilo de las de Internet Explorer.
//!
//! Cabe señalar que no se usa el `Scrollbar` de ratatui: su pulgar calcula la posición máxima sobre
//! `contenido − 1` y la longitud sobre `contenido + visible − 1`, de modo que, con un desplazamiento
//! que llega como mucho a `contenido − visible`, nunca alcanza la última fila. Aquí la geometría es
//! exacta: el pulgar toca siempre los dos extremos de la pista.

use crate::state::{Scroll, StyleToken};
use crate::widgets::{ChromeStyle, Theme};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

/// Flecha superior e inferior, como los botones de las barras clásicas.
const ARROW_UP: &str = "▲";
const ARROW_DOWN: &str = "▼";
/// Pista punteada y pulgar macizo, con unas rayitas de agarre en medio.
const TRACK: &str = "▒";
const THUMB: &str = "█";
const GRIP: &str = "≡";
/// Línea fina del aspecto sin cajas: pista tenue y pulgar más grueso.
const THIN_TRACK: &str = "│";
const THIN_THUMB: &str = "┃";
/// Altura mínima del área para dibujar las flechas (dos) y al menos dos filas de pista.
const MIN_HEIGHT_FOR_ARROWS: u16 = 4;
/// Longitud mínima del pulgar para dibujarle el agarre.
const MIN_THUMB_FOR_GRIP: usize = 3;

/// Posición y longitud del pulgar, en filas de pista, o `None` si no hay nada que desplazar.
///
/// La longitud es proporcional a lo visible (mínimo una fila) y la posición recorre toda la pista:
/// con el desplazamiento en cero el pulgar empieza en la fila 0 y con el máximo acaba en la última.
pub fn thumb(track: usize, scroll: Scroll) -> Option<(usize, usize)> {
    if track == 0 || scroll.content == 0 || scroll.content <= scroll.viewport {
        return None;
    }
    let content = scroll.content;
    let viewport = scroll.viewport.max(1);
    // Redondeo al entero más próximo, con aritmética entera.
    let len = ((track * viewport * 2 + content) / (content * 2)).clamp(1, track);
    let max_top = content - viewport;
    let offset = scroll.offset.min(max_top);
    let travel = track - len;
    let start = (travel * offset * 2 + max_top) / (max_top * 2);
    Some((start, len))
}

/// Pinta una barra vertical en `area` según la posición de `scroll`.
///
/// Cabe señalar que el aspecto sigue al marco del tema: con paneles (`Panels`) se dibuja la barra
/// clásica con flechas; sin cajas (`Flat`), una línea fina, sin flechas ni agarre.
pub fn render(frame: &mut Frame, area: Rect, scroll: Scroll, theme: &Theme) {
    if area.is_empty() {
        return;
    }
    let accent = theme.style(StyleToken::Accent);
    let muted = theme.style(StyleToken::Muted);
    if theme.look.chrome == ChromeStyle::Flat {
        let thumb = thumb(usize::from(area.height), scroll);
        let buf = frame.buffer_mut();
        for row in 0..usize::from(area.height) {
            let on = thumb.is_some_and(|(start, len)| (start..start + len).contains(&row));
            let (symbol, style) = if on {
                (THIN_THUMB, accent)
            } else {
                (THIN_TRACK, muted)
            };
            buf[(area.x, area.y + row as u16)]
                .set_symbol(symbol)
                .set_style(style);
        }
        return;
    }
    let arrows = area.height >= MIN_HEIGHT_FOR_ARROWS;
    let track_len = usize::from(area.height) - if arrows { 2 } else { 0 };
    let first_track_row = area.y + u16::from(arrows);
    let thumb = thumb(track_len, scroll);
    let buf = frame.buffer_mut();
    let mut put = |y: u16, symbol: &str, style: Style| {
        buf[(area.x, y)].set_symbol(symbol).set_style(style);
    };

    if arrows {
        // Una flecha apagada indica que ya no se puede ir más en esa dirección.
        let at_top = scroll.offset == 0 || thumb.is_none();
        let at_bottom =
            thumb.is_none_or(|_| scroll.offset >= scroll.content.saturating_sub(scroll.viewport));
        put(area.y, ARROW_UP, if at_top { muted } else { accent });
        put(
            area.y + area.height - 1,
            ARROW_DOWN,
            if at_bottom { muted } else { accent },
        );
    }

    let grip_style = Style {
        fg: muted.fg,
        bg: accent.fg,
        ..accent
    };
    for row in 0..track_len {
        let y = first_track_row + row as u16;
        match thumb {
            Some((start, len)) if (start..start + len).contains(&row) => {
                let middle = start + len / 2;
                if len >= MIN_THUMB_FOR_GRIP && row == middle {
                    put(y, GRIP, grip_style);
                } else {
                    put(y, THUMB, accent);
                }
            }
            _ => put(y, TRACK, muted),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Scroll;
    use crate::widgets::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Símbolos de la columna de la barra, de arriba abajo.
    fn column(h: u16, scroll: Scroll) -> Vec<String> {
        let backend = TestBackend::new(1, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), scroll, &Theme::test()))
            .unwrap();
        let buf = term.backend().buffer();
        (0..h).map(|y| buf[(0, y)].symbol().to_string()).collect()
    }

    fn scroll(offset: usize, content: usize, viewport: usize) -> Scroll {
        Scroll {
            offset,
            content,
            viewport,
        }
    }

    #[test]
    fn the_thumb_reaches_both_ends_of_the_track() {
        for (content, viewport, track) in [(100, 10, 10), (51, 5, 3), (1000, 40, 38), (11, 10, 8)] {
            let max = content - viewport;
            let (top, len) = thumb(track, scroll(0, content, viewport)).unwrap();
            assert_eq!(top, 0, "arriba con {content}/{viewport}/{track}");
            assert!(len >= 1 && len <= track);
            let (start, len) = thumb(track, scroll(max, content, viewport)).unwrap();
            assert_eq!(start + len, track, "abajo con {content}/{viewport}/{track}");
        }
    }

    #[test]
    fn the_thumb_only_moves_down_as_the_offset_grows() {
        let mut last = 0;
        for offset in 0..=90 {
            let (start, _) = thumb(10, scroll(offset, 100, 10)).unwrap();
            assert!(start >= last, "retrocede en {offset}");
            last = start;
        }
    }

    #[test]
    fn the_thumb_length_follows_how_much_is_visible() {
        let (_, small) = thumb(20, scroll(0, 1000, 10)).unwrap();
        let (_, large) = thumb(20, scroll(0, 40, 10)).unwrap();
        assert_eq!(small, 1, "mínimo una fila");
        assert!(large > small);
        assert_eq!(
            thumb(20, scroll(0, 10, 10)),
            None,
            "si cabe todo, sin pulgar"
        );
    }

    #[test]
    fn it_draws_arrows_a_dotted_track_and_a_thumb_with_a_grip() {
        // 12 filas: flechas arriba y abajo, 10 de pista y un pulgar de 5 filas con el agarre en medio.
        let top = column(12, scroll(0, 20, 10));
        assert_eq!(top.first().map(String::as_str), Some("▲"));
        assert_eq!(top.last().map(String::as_str), Some("▼"));
        assert!(top.iter().any(|s| s == "▒"), "pista punteada: {top:?}");
        assert!(top.iter().any(|s| s == "█"), "pulgar: {top:?}");
        assert!(top.iter().any(|s| s == "≡"), "agarre: {top:?}");
    }

    #[test]
    fn at_the_bottom_the_thumb_touches_the_last_track_row() {
        let rows = column(12, scroll(10, 20, 10));
        // La fila 10 es la última de pista (la 11 es la flecha inferior).
        assert_ne!(rows[10], "▒", "el pulgar llega abajo: {rows:?}");
        assert_eq!(rows[1], "▒", "y arriba queda pista");
        let rows = column(12, scroll(0, 20, 10));
        assert_ne!(rows[1], "▒", "al principio el pulgar toca arriba: {rows:?}");
    }

    #[test]
    fn the_arrows_dim_at_the_limits() {
        let theme = Theme::test();
        let (accent, muted) = (
            theme.style(StyleToken::Accent).fg,
            theme.style(StyleToken::Muted).fg,
        );
        let fg_at = |offset: usize, y: u16| {
            let backend = TestBackend::new(1, 12);
            let mut term = Terminal::new(backend).unwrap();
            term.draw(|f| render(f, f.area(), scroll(offset, 20, 10), &theme))
                .unwrap();
            term.backend().buffer()[(0, y)].fg
        };
        assert_eq!(
            fg_at(0, 0),
            muted.unwrap_or_default(),
            "arriba apagada al principio"
        );
        assert_eq!(fg_at(0, 11), accent.unwrap_or_default(), "abajo encendida");
        assert_eq!(
            fg_at(10, 11),
            muted.unwrap_or_default(),
            "abajo apagada al final"
        );
        assert_eq!(fg_at(10, 0), accent.unwrap_or_default(), "arriba encendida");
    }

    #[test]
    fn a_short_area_draws_no_arrows() {
        let rows = column(3, scroll(0, 100, 10));
        assert!(!rows.iter().any(|s| s == "▲" || s == "▼"), "{rows:?}");
        assert!(rows.iter().any(|s| s == "█"));
    }

    /// Columna pintada con un tema de aspecto plano.
    fn flat_column(h: u16, scroll: Scroll) -> Vec<String> {
        let mut theme = Theme::test();
        theme.look.chrome = ChromeStyle::Flat;
        let mut term = Terminal::new(TestBackend::new(1, h)).unwrap();
        term.draw(|f| render(f, f.area(), scroll, &theme)).unwrap();
        let buf = term.backend().buffer();
        (0..h).map(|y| buf[(0, y)].symbol().to_string()).collect()
    }

    #[test]
    fn the_flat_look_draws_a_thin_line_without_arrows() {
        let rows = flat_column(10, scroll(0, 100, 10));
        assert!(rows.iter().all(|s| s == "│" || s == "┃"), "{rows:?}");
        assert_eq!(rows[0], "┃", "el pulgar toca arriba");
        assert_eq!(rows[9], "│");
    }

    #[test]
    fn the_flat_thumb_reaches_the_last_row() {
        let rows = flat_column(10, scroll(90, 100, 10));
        assert_eq!(rows[9], "┃", "el pulgar toca abajo: {rows:?}");
        assert_eq!(rows[0], "│");
    }

    #[test]
    fn zero_and_tiny_sizes_do_not_panic() {
        let s = scroll(0, 0, 0);
        let _ = column(1, s);
        let _ = column(1, scroll(5, 3, 9));
        let _ = column(2, scroll(0, 100, 10));
        let backend = TestBackend::new(0, 0);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), s, &Theme::test()))
            .unwrap();
    }
}
