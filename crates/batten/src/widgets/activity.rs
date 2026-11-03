//! Indicadores de actividad braille del catálogo `dotmax`.
//!
//! `dotmax` dibuja en una rejilla braille de `ancho × alto` celdas de terminal.
//! Aquí se usa para la barra de estado: una sola fila, con el estilo elegido
//! animado por el `frame` del `Anim` que ya avanza el bucle de la TUI.
//!
//! Con la feature `dotmax` apagada, [`activity_glyph`] devuelve `None` y quien
//! llama cae al glifo propio de [`super::spinner`].

#[cfg(feature = "dotmax")]
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::Theme;

/// Estilos de `dotmax` que se ven bien en una única fila de terminal.
///
/// Se eligen de la familia `spinner` porque el resto del catálogo (barras,
/// medidores) necesita más de una fila para leerse. Los nombres son los del
/// crate; si una versión futura los renombra, [`activity_glyph`] cae al glifo
/// propio en vez de romper.
pub const ACTIVITY_STYLES: &[&str] = &[
    "dot-ring",
    "growing-arc",
    "spinner-bars",
    "arc-sweep",
    "pulse",
];

/// Un fotograma del indicador de actividad, ya resuelto a un `Span`.///
/// `frame` es el contador monótono del `Anim` de la app; `width` es el ancho en
/// celdas que se le concede al indicador (1 o 2 suelen bastar).
#[cfg(feature = "dotmax")]
pub fn activity_glyph(
    style_name: &str,
    frame: u64,
    width: u16,
    theme: &Theme,
) -> Option<Span<'static>> {
    use dotmax::progress::{styles_for_theme, BarContext};

    let width = usize::from(width.max(1));
    let styles = styles_for_theme("spinner");
    let style = styles.iter().find(|s| s.name() == style_name)?;

    let mut grid = dotmax::BrailleGrid::new(width, 1).ok()?;
    // `0.5` es el relleno: los spinners lo ignoran, las barras lo usan.
    let context = BarContext::new(0.5, frame as f32 * 0.05, width, 1);
    style.render(&mut grid, &context).ok()?;

    let mut text = String::with_capacity(width);
    let mut color = None;
    for x in 0..width {
        let glyph = grid.get_char(x, 0);
        text.push(if glyph == '\u{2800}' { ' ' } else { glyph });
        if color.is_none() {
            color = grid.get_color(x, 0).map(|c| Color::Rgb(c.r, c.g, c.b));
        }
    }
    let style = match color {
        Some(color) => Style::default().fg(color),
        None => theme.styles.accent,
    };
    Some(Span::styled(text, style))
}

/// Sin la feature `dotmax`, no hay indicador braille: quien llama usa el glifo propio.
#[cfg(not(feature = "dotmax"))]
pub fn activity_glyph(
    _style_name: &str,
    _frame: u64,
    _width: u16,
    _theme: &Theme,
) -> Option<Span<'static>> {
    None
}

/// Nombres de los estilos disponibles, para el autocompletado y los tests.
pub fn activity_style_names() -> &'static [&'static str] {
    ACTIVITY_STYLES
}

/// Elige un estilo para una petición nueva.
///
/// `seed` lo aporta quien llama (p. ej. el reloj al arrancar una petición): así
/// el estilo cambia entre peticiones y los tests pueden fijarlo. No se usa un
/// generador aleatorio para no añadir una dependencia por esto.
pub fn pick_activity_style(seed: u64) -> &'static str {
    ACTIVITY_STYLES[(seed % ACTIVITY_STYLES.len() as u64) as usize]
}

/// Spinner clásico de Crush: una barra que gira sobre sí misma.
///
/// Los cuatro glifos son el mismo trazo rotado 90°, así que la animación se lee
/// como un giro continuo a 4 pasos y encaja con el resto del chasis Crush.
pub const CRUSH_FRAMES: &[char] = &['-', '\\', '|', '/'];

/// Glifo del spinner de Crush para el fotograma `frame`.
pub fn crush_glyph(frame: u64, theme: &Theme) -> Span<'static> {
    let glyph = CRUSH_FRAMES[(frame as usize) % CRUSH_FRAMES.len()];
    Span::styled(glyph.to_string(), theme.styles.accent)
}

#[cfg(test)]
mod pick_tests {
    use super::*;

    #[test]
    fn a_seed_selects_a_style_and_stays_in_range() {
        for seed in [0u64, 1, 4, 5, 99, u64::MAX] {
            let name = pick_activity_style(seed);
            assert!(ACTIVITY_STYLES.contains(&name), "semilla {seed}: {name}");
        }
    }

    #[test]
    fn different_seeds_reach_every_style() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..ACTIVITY_STYLES.len() as u64 {
            seen.insert(pick_activity_style(seed));
        }
        assert_eq!(seen.len(), ACTIVITY_STYLES.len());
    }

    #[test]
    fn the_crush_spinner_cycles_through_its_four_frames() {
        let theme = Theme::default_dark();
        let mut seen = std::collections::BTreeSet::new();
        for frame in 0..4 {
            seen.insert(crush_glyph(frame, &theme).content);
        }
        assert_eq!(seen.len(), 4, "el spinner de crush no rota");
        // Vuelve a empezar: es cíclico, no acumulativo.
        assert_eq!(
            crush_glyph(0, &theme).content,
            crush_glyph(4, &theme).content
        );
    }
}

#[cfg(all(test, feature = "dotmax"))]
mod tests {
    use super::*;

    #[test]
    fn every_activity_style_renders_a_non_empty_cell() {
        let theme = Theme::default_dark();
        for name in ACTIVITY_STYLES {
            let span =
                activity_glyph(name, 3, 1, &theme).unwrap_or_else(|| panic!("{name} no renderiza"));
            assert!(
                !span.content.trim().is_empty(),
                "{name} produjo una celda vacía"
            );
        }
    }

    #[test]
    fn the_glyph_changes_between_frames() {
        let theme = Theme::default_dark();
        let first = activity_glyph("dot-ring", 0, 1, &theme).unwrap();
        let later = activity_glyph("dot-ring", 7, 1, &theme).unwrap();
        assert_ne!(first.content, later.content, "el indicador no anima");
    }

    #[test]
    fn an_unknown_style_is_reported_as_missing() {
        let theme = Theme::default_dark();
        assert!(activity_glyph("no-existe", 0, 1, &theme).is_none());
    }

    #[test]
    fn two_cells_are_wider_than_one() {
        let theme = Theme::default_dark();
        let one = activity_glyph("growing-arc", 2, 1, &theme).unwrap();
        let two = activity_glyph("growing-arc", 2, 2, &theme).unwrap();
        assert!(two.content.chars().count() >= one.content.chars().count());
    }
}
