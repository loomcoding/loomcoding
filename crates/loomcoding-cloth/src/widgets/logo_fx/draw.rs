//! Dibujo de cada efecto sobre el `Buffer`, con colores del tema. Solo toca el `area` del logo.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::effects::Ctx;
use super::LogoEffect;
use crate::widgets::theme::ColorDepth;
use crate::widgets::Theme;

/// Aplica `effect` con `progress` en `0.0..=1.0` sobre las celdas de `area` que caigan en `buf`.
/// Con `ColorDepth::NoColor` no hace nada. A `progress` 0.0 y 1.0 los efectos que lo indican
/// dejan la marca tal cual.
pub fn apply(
    effect: LogoEffect,
    progress: f32,
    buf: &mut Buffer,
    area: Rect,
    theme: &Theme,
    seed: u64,
) {
    if theme.depth == ColorDepth::NoColor {
        return;
    }
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return;
    }
    let p = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let c = Ctx {
        area,
        p,
        seed,
        theme,
    };
    match effect {
        LogoEffect::Flashlight => c.flashlight(buf),
        LogoEffect::BinaryRain => c.binary_rain(buf),
        LogoEffect::Shimmer => c.shimmer(buf),
        LogoEffect::Scramble => c.scramble(buf),
        LogoEffect::Glitch => c.glitch(buf),
        LogoEffect::Dissolve => c.dissolve(buf),
        LogoEffect::Pulse => c.pulse(buf),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::loom_mark::{loom_mark, LOOM_MARK_ROWS, LOOM_MARK_WIDTH};
    use crate::widgets::theme::ColorDepth;
    use ratatui::style::{Modifier, Style};

    /// Área del logo dentro de un buffer de 40×8.
    const AREA: Rect = Rect {
        x: 2,
        y: 2,
        width: LOOM_MARK_WIDTH as u16,
        height: LOOM_MARK_ROWS as u16,
    };

    fn lienzo(theme: &Theme) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        for (i, linea) in loom_mark("Loomcoding", LOOM_MARK_WIDTH, theme)
            .iter()
            .enumerate()
        {
            for span in &linea.spans {
                buf.set_string(AREA.x, AREA.y + i as u16, &span.content, span.style);
            }
        }
        buf
    }

    fn dentro(x: u16, y: u16) -> bool {
        AREA.contains(ratatui::layout::Position::new(x, y))
    }

    fn estilo(buf: &Buffer, x: u16, y: u16) -> Style {
        buf[(x, y)].style()
    }

    fn mismos_fuera(antes: &Buffer, despues: &Buffer) -> bool {
        (0..40u16)
            .flat_map(|x| (0..8u16).map(move |y| (x, y)))
            .filter(|&(x, y)| !dentro(x, y))
            .all(|(x, y)| antes[(x, y)] == despues[(x, y)])
    }

    fn mismo_area(antes: &Buffer, despues: &Buffer) -> bool {
        (0..40u16)
            .flat_map(|x| (0..8u16).map(move |y| (x, y)))
            .filter(|&(x, y)| dentro(x, y))
            .all(|(x, y)| antes[(x, y)] == despues[(x, y)])
    }

    #[test]
    fn celdas_fuera_del_area_intactas_para_cada_efecto() {
        let theme = Theme::test();
        for efecto in LogoEffect::ALL {
            for seed in [6u64, 7] {
                for p in [0.0f32, 0.5, 1.0] {
                    let antes = lienzo(&theme);
                    let mut despues = antes.clone();
                    apply(efecto, p, &mut despues, AREA, &theme, seed);
                    assert!(
                        mismos_fuera(&antes, &despues),
                        "{efecto:?} p={p} seed={seed} tocó fuera del área"
                    );
                }
            }
        }
    }

    #[test]
    fn extremos_dejan_la_marca_igual() {
        let theme = Theme::test();
        for efecto in [
            LogoEffect::Flashlight,
            LogoEffect::Glitch,
            LogoEffect::Dissolve,
            LogoEffect::Scramble,
        ] {
            for p in [0.0f32, 1.0] {
                let antes = lienzo(&theme);
                let mut despues = antes.clone();
                apply(efecto, p, &mut despues, AREA, &theme, 11);
                assert!(mismo_area(&antes, &despues), "{efecto:?} p={p}");
            }
        }
    }

    #[test]
    fn linterna_aclara_el_centro_y_atenua_el_borde() {
        let theme = Theme::test();
        let antes = lienzo(&theme);
        let mut despues = antes.clone();
        apply(LogoEffect::Flashlight, 0.5, &mut despues, AREA, &theme, 4);
        let centro = (AREA.x + 9, AREA.y + 1);
        let borde = (AREA.x, AREA.y + 1);
        assert!(!estilo(&antes, centro.0, centro.1)
            .add_modifier
            .contains(Modifier::BOLD));
        assert!(estilo(&despues, centro.0, centro.1)
            .add_modifier
            .contains(Modifier::BOLD));
        assert!(estilo(&despues, borde.0, borde.1)
            .add_modifier
            .contains(Modifier::DIM));
    }

    #[test]
    fn semilla_par_va_a_la_izquierda_e_impar_a_la_derecha() {
        let theme = Theme::test();
        let centro_x = |seed: u64| -> f32 {
            let mut buf = lienzo(&theme);
            apply(LogoEffect::Flashlight, 0.2, &mut buf, AREA, &theme, seed);
            let xs: Vec<f32> = (AREA.x..AREA.x + AREA.width)
                .flat_map(|x| (AREA.y..AREA.y + AREA.height).map(move |y| (x, y)))
                .filter(|&(x, y)| estilo(&buf, x, y).add_modifier.contains(Modifier::BOLD))
                .map(|(x, _)| f32::from(x))
                .collect();
            assert!(!xs.is_empty(), "el foco no ilumina nada con seed {seed}");
            xs.iter().sum::<f32>() / xs.len() as f32
        };
        assert!(centro_x(2) < centro_x(3));
    }

    #[test]
    fn sin_color_no_toca_nada() {
        let mut theme = Theme::test();
        theme.depth = ColorDepth::NoColor;
        for efecto in LogoEffect::ALL {
            let antes = lienzo(&theme);
            let mut despues = antes.clone();
            apply(efecto, 0.5, &mut despues, AREA, &theme, 9);
            assert!(mismo_area(&antes, &despues), "{efecto:?}");
            assert!(mismos_fuera(&antes, &despues), "{efecto:?}");
        }
    }

    #[test]
    fn areas_degeneradas_no_entran_en_panico() {
        let theme = Theme::test();
        let areas = [
            Rect::new(0, 0, 0, 0),
            Rect::new(3, 3, 1, 1),
            Rect::new(35, 6, 19, 4),
            Rect::new(0, 0, 40, 8),
        ];
        for efecto in LogoEffect::ALL {
            for area in areas {
                for p in [0.0f32, 0.3, 1.0] {
                    let mut buf = lienzo(&theme);
                    apply(efecto, p, &mut buf, area, &theme, 1);
                    let mut pequeno = Buffer::empty(Rect::new(0, 0, 3, 2));
                    apply(efecto, p, &mut pequeno, AREA, &theme, 1);
                }
            }
        }
    }
}
