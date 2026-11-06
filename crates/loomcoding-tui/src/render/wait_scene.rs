//! Escena de la espera: el muñeco se acerca de espaldas, empuja las letras hasta el centro de la
//! pantalla, se da la vuelta y regresa; las letras vuelven solas a su sitio y el ciclo se repite.
//!
//! Cabe señalar que es una función pura del reloj y del ancho: no guarda estado alguno.

/// Margen izquierdo de la franja, en columnas.
const MARGIN: u16 = 1;
/// Columnas del bloque braille del muñeco.
const MASCOT_WIDTH: u16 = 6;
/// Columna de las letras en reposo: tras el margen, el muñeco y dos de hueco.
pub(super) const TEXT_HOME: u16 = MARGIN + MASCOT_WIDTH + 2;
/// Distancia entre el muñeco y las letras cuando las empuja (una celda de aire tras su espalda).
const CONTACT_GAP: u16 = 5;

/// Dónde está cada cosa en un instante de la escena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WaitScene {
    /// Columna, dentro de la franja, donde empieza el bloque del muñeco.
    pub mascot_x: u16,
    /// El muñeco camina de espaldas hacia la derecha (dibujo en espejo).
    pub mirrored: bool,
    /// Columna donde empiezan las letras.
    pub text_x: u16,
    /// Paso de 150 ms, para alternar las piernas.
    pub step: u64,
}

/// Escena para el reloj `tick` (50 ms), una franja de `width` columnas y unas letras de
/// `text_len`. `None` si la franja es demasiado estrecha para recorrerla: entonces el muñeco se
/// queda con su paseo corto de siempre.
pub(super) fn wait_scene(tick: u64, width: u16, text_len: u16) -> Option<WaitScene> {
    // Las letras acaban centradas; el recorrido son las columnas entre su sitio y el centro.
    let center = width.saturating_sub(text_len) / 2;
    if center < TEXT_HOME + 2 {
        return None;
    }
    let travel = u64::from(center - TEXT_HOME);
    let contact = TEXT_HOME - CONTACT_GAP;
    // Pasos de la aproximación: del margen al punto en que el muñeco toca las letras.
    let approach = u64::from(contact - MARGIN);
    // Aproximación, empuje, vuelta (de la pared del centro a la izquierda) y regreso de las letras.
    let total = 2 * approach + 3 * travel;
    let step = tick / 3;
    let s = step % total;
    let (mascot_x, mirrored, text_x) = if s < approach {
        (MARGIN + s as u16 + 1, true, TEXT_HOME)
    } else if s < approach + travel {
        let k = (s - approach) as u16;
        (contact + k + 1, true, TEXT_HOME + k + 1)
    } else if s < 2 * approach + 2 * travel {
        // Cabe señalar que el primer paso de la vuelta gira en el sitio: misma columna, de cara.
        let k = (s - approach - travel) as u16;
        (contact + travel as u16 - k, false, center)
    } else {
        let k = (s - 2 * approach - 2 * travel) as u16;
        (MARGIN, false, center - k - 1)
    };
    Some(WaitScene {
        mascot_x,
        mirrored,
        text_x,
        step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: u16 = 100;
    const LEN: u16 = 16;
    /// Columna donde quedan las letras, centradas.
    const CENTER: u16 = (WIDTH - LEN) / 2;

    fn at(step: u64) -> WaitScene {
        wait_scene(step * 3, WIDTH, LEN).expect("hay sitio para recorrer la franja")
    }

    /// Pasos de cada fase con el ancho de prueba.
    fn phases() -> (u64, u64, u64) {
        let l = u64::from(CENTER - TEXT_HOME);
        let p1 = u64::from(TEXT_HOME - CONTACT_GAP - MARGIN);
        (p1, l, 2 * p1 + 3 * l)
    }

    #[test]
    fn it_starts_at_home_walking_backwards() {
        let scene = at(0);
        assert_eq!(scene.text_x, TEXT_HOME);
        assert!(scene.mirrored);
        assert!(scene.mascot_x > MARGIN);
    }

    #[test]
    fn the_letters_are_pushed_to_the_middle_of_the_screen() {
        let (p1, l, _) = phases();
        let pushing_ends = at(p1 + l - 1);
        assert_eq!(pushing_ends.text_x, CENTER);
        assert!(pushing_ends.mirrored, "empuja de espaldas");
        // Mientras empuja, las letras van pegadas al muñeco.
        for step in p1..p1 + l {
            let scene = at(step);
            assert_eq!(scene.text_x, scene.mascot_x + CONTACT_GAP, "paso {step}");
        }
    }

    #[test]
    fn it_turns_around_and_walks_back_leaving_the_letters_in_the_middle() {
        let (p1, l, _) = phases();
        let turn = at(p1 + l);
        assert!(!turn.mirrored, "tras la pared, de cara");
        assert_eq!(turn.text_x, CENTER);
        let back = at(2 * p1 + 2 * l - 1);
        assert_eq!(back.text_x, CENTER, "las letras se quedan en el centro");
        assert!(
            back.mascot_x <= MARGIN + 1,
            "el muñeco ha vuelto a la pared"
        );
    }

    #[test]
    fn the_letters_slide_home_and_the_cycle_repeats() {
        let (p1, l, total) = phases();
        assert_eq!(at(total - 1).text_x, TEXT_HOME);
        // Se repite la posición; el paso absoluto sigue contando para alternar las piernas.
        let place = |s: WaitScene| (s.mascot_x, s.mirrored, s.text_x);
        assert_eq!(place(at(total)), place(at(0)));
        assert_eq!(place(at(7 * total + 5)), place(at(5)));
        // Las letras regresan sin que el muñeco se mueva.
        assert_eq!(at(2 * p1 + 2 * l).mascot_x, MARGIN);
    }

    #[test]
    fn nothing_jumps_overlaps_or_leaves_the_strip() {
        let (_, _, total) = phases();
        let mut previous = at(total - 1);
        for step in 0..total * 2 {
            let scene = at(step);
            assert!(
                scene.mascot_x.abs_diff(previous.mascot_x) <= 1,
                "salta el muñeco en {step}"
            );
            assert!(
                scene.text_x.abs_diff(previous.text_x) <= 1,
                "saltan las letras en {step}"
            );
            let right_edge = scene.mascot_x + if scene.mirrored { 4 } else { 5 };
            assert!(
                right_edge <= scene.text_x,
                "el muñeco pisa las letras en {step}"
            );
            assert!(scene.text_x + LEN <= WIDTH, "las letras se salen en {step}");
            previous = scene;
        }
    }

    #[test]
    fn a_narrow_strip_keeps_the_short_walk() {
        assert_eq!(wait_scene(0, 30, LEN), None);
        assert_eq!(wait_scene(0, 0, LEN), None);
    }
}
