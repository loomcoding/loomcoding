//! Marca «LOOM» de cuatro filas al estilo Crush, para la cabecera del menú izquierdo.

use crate::state::StyleToken;
use crate::widgets::text::truncate;
use crate::widgets::Theme;
use ratatui::text::{Line, Span};

/// Ancho de la marca: las dos filas de letras y la franja miden 19 columnas.
pub const LOOM_MARK_WIDTH: usize = 19;

/// Filas de la marca: dos de letras, la franja y el nombre.
pub const LOOM_MARK_ROWS: usize = 4;

/// Letras «LOOM» en bloques, dos filas de 19 columnas cada una.
const LETRAS: [&str; 2] = ["█   ▄▀▀▄ ▄▀▀▄ █▀▄▀█", "█▄▄ ▀▄▄▀ ▀▄▄▀ █ ▀ █"];

/// Devuelve las cuatro filas de la marca, o un vector vacío si `width` no alcanza
/// `LOOM_MARK_WIDTH`; en ese caso el llamante cae al logo compacto.
///
/// Cabe señalar que las letras y la franja usan el acento del tema y el nombre,
/// en minúsculas, el estilo tenue.
pub fn loom_mark(name: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    if width < LOOM_MARK_WIDTH {
        return Vec::new();
    }
    let acento = theme.style(StyleToken::Accent);
    let tenue = theme.style(StyleToken::Muted);
    vec![
        Line::from(Span::styled(LETRAS[0], acento)),
        Line::from(Span::styled(LETRAS[1], acento)),
        Line::from(Span::styled("╱".repeat(LOOM_MARK_WIDTH), acento)),
        Line::from(Span::styled(truncate(&name.to_lowercase(), width), tenue)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn texts(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn loom_mark_has_four_rows_and_fits_nineteen_columns() {
        let theme = Theme::test();
        let lineas = loom_mark("Loomcoding", LOOM_MARK_WIDTH, &theme);
        let filas = texts(&lineas);
        assert_eq!(filas.len(), 4, "{filas:?}");
        assert_eq!(filas[0].width(), 19, "letras, fila 1: {:?}", filas[0]);
        assert_eq!(filas[1].width(), 19, "letras, fila 2: {:?}", filas[1]);
        assert_eq!(filas[2], "╱".repeat(19), "franja");
        assert_eq!(filas[3], "loomcoding", "nombre en minúsculas");
        let acento = theme.style(StyleToken::Accent).fg;
        assert_eq!(lineas[0].spans[0].style.fg, acento);
    }

    #[test]
    fn loom_mark_is_empty_when_too_narrow() {
        let theme = Theme::test();
        assert!(loom_mark("Loomcoding", 18, &theme).is_empty());
    }
}
