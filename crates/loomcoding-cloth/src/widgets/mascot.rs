//! Muñeco braille: una figura de 6×2 celdas cuya postura dice qué hace el agente.
//!
//! Cada postura es una secuencia de mapas de puntos de 12×8 (`#` = punto encendido) que se
//! convierte en dos líneas de caracteres braille. Cabe señalar que el reloj del llamante se
//! mide en ticks de 50 ms.

/// Lo que hace el agente, y por tanto la postura del muñeco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    Think,
    Run,
    Hunt,
    Wait,
    Done,
    Fail,
}

/// Muñecos de 12×8 puntos (6×2 celdas braille). `#` es punto encendido.
const RUN: [[&str; 8]; 4] = [
    [
        ".....##.....",
        ".....##.....",
        "....####....",
        "...#.##.#...",
        ".....##.....",
        "....#..#....",
        "...#....#...",
        "..#......#..",
    ],
    [
        ".....##.....",
        ".....##.....",
        "....####....",
        "....#.##.#..",
        ".....##.....",
        ".....##.....",
        "....#..#....",
        "...#....#...",
    ],
    [
        ".....##.....",
        ".....##.....",
        "....####....",
        "...#.##.#...",
        ".....##.....",
        ".....##.....",
        ".....##.....",
        "....#..#....",
    ],
    [
        ".....##.....",
        ".....##.....",
        "....####....",
        "..#..##..#..",
        ".....##.....",
        "....#..#....",
        "....#..#....",
        "...#....#...",
    ],
];
const THINK: [[&str; 8]; 2] = [
    [
        ".....##...#.",
        ".....##..#.#",
        "....####..#.",
        "...#.##.#...",
        ".....##.....",
        ".....##.....",
        "....#..#....",
        "....#..#....",
    ],
    [
        ".....##.....",
        ".....##..#.#",
        "....####.#.#",
        "...#.##.#.#.",
        ".....##.....",
        ".....##.....",
        "....#..#....",
        "....#..#....",
    ],
];
const DONE: [[&str; 8]; 2] = [
    [
        "..#..##..#..",
        "...#.##.#...",
        "....####....",
        ".....##.....",
        ".....##.....",
        "....#..#....",
        "....#..#....",
        "............",
    ],
    [
        "............",
        "..#..##..#..",
        "...#.##.#...",
        "....####....",
        ".....##.....",
        ".....##.....",
        "....#..#....",
        "...#....#...",
    ],
];
/// Espera con garbo: el moonwalk. Con sombrero, un pie plano que apunta a la derecha y el otro
/// de puntillas; las piernas se alternan y `moonwalk` desliza toda la figura hacia la izquierda.
const MOONWALK: [[&str; 8]; 4] = [
    [
        ".....####...",
        "......##....",
        ".....####...",
        "....#.##.#..",
        "......##....",
        ".....#..#...",
        ".....#..#...",
        ".....##.....",
    ],
    [
        ".....####...",
        "......##....",
        ".....####...",
        "....#.##.#..",
        "......##....",
        "......##....",
        "......##....",
        "......##....",
    ],
    [
        ".....####...",
        "......##....",
        ".....####...",
        "....#.##.#..",
        "......##....",
        ".....#..#...",
        ".....#..#...",
        "........##..",
    ],
    [
        ".....####...",
        "......##....",
        ".....####...",
        "....#.##.#..",
        "......##....",
        "......##....",
        "......##....",
        "......##....",
    ],
];

/// Puntos que recorre el moonwalk en cada sentido antes de darse la vuelta.
const MOONWALK_TRAVEL: usize = 4;

/// Desplaza una fila de 12 puntos `delta` puntos (negativo, a la izquierda), rellenando de vacío.
fn shifted(row: &str, delta: isize) -> String {
    let mut cells = vec!['.'; 12];
    for (i, c) in row.chars().enumerate() {
        let target = i as isize + delta;
        if (0..12).contains(&target) {
            cells[target as usize] = c;
        }
    }
    cells.into_iter().collect()
}

/// Mapa del moonwalk para un paso del reloj. Las piernas cambian en cada paso y la figura avanza
/// un punto cada dos. Cabe señalar que primero se desliza hacia la izquierda y, al tocar la pared,
/// se da la vuelta (el dibujo en espejo, de modo que los pies sigan apuntando al revés de la
/// marcha) y regresa hacia la derecha; en el extremo derecho vuelve a girar y sigue. El giro cae
/// en el mismo lugar por ambos lados, así que el paseo no da saltos.
fn moonwalk(step: usize) -> [String; 8] {
    let glide = (step / 2) % (2 * MOONWALK_TRAVEL);
    MOONWALK[step % MOONWALK.len()].map(|row| {
        if glide < MOONWALK_TRAVEL {
            shifted(row, -(glide as isize))
        } else {
            let mirrored: String = row.chars().rev().collect();
            shifted(&mirrored, glide as isize - MOONWALK_TRAVEL as isize - 2)
        }
    })
}
const FAIL: [[&str; 8]; 1] = [[
    "............",
    "............",
    "............",
    "............",
    "............",
    "..##........",
    ".####.#####.",
    "..##..#...#.",
]];

/// Convierte un mapa de puntos de 12×8 en dos líneas de seis celdas braille.
fn braille<S: AsRef<str>>(bitmap: &[S; 8]) -> [String; 2] {
    // Bit de cada punto dentro de la celda: (columna, fila) → máscara Unicode braille.
    const BIT: [[u32; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
    let mut rows = [String::new(), String::new()];
    for (cell_row, out) in rows.iter_mut().enumerate() {
        for cell_col in 0..6 {
            let mut mask = 0u32;
            for (dx, column_bits) in BIT.iter().enumerate() {
                for (dy, &bit) in column_bits.iter().enumerate() {
                    let line = bitmap[cell_row * 4 + dy].as_ref().as_bytes();
                    if line.get(cell_col * 2 + dx) == Some(&b'#') {
                        mask |= bit;
                    }
                }
            }
            out.push(char::from_u32(0x2800 + mask).unwrap_or(' '));
        }
    }
    rows
}

/// Fotograma del muñeco para una postura y un reloj de 50 ms.
///
/// Cabe señalar que `Run` y `Hunt` comparten fotogramas; `Think`, `Wait` y `Done` avanzan más
/// despacio que `Run`, y `Fail` es estático.
pub fn mascot_frame(pose: Pose, tick: u64) -> [String; 2] {
    let step = (tick / 3) as usize;
    match pose {
        Pose::Run | Pose::Hunt => braille(&RUN[step % RUN.len()]),
        Pose::Think => braille(&THINK[(step / 2) % THINK.len()]),
        Pose::Wait => braille(&moonwalk(step)),
        Pose::Done => braille(&DONE[(step / 2) % DONE.len()]),
        Pose::Fail => braille(&FAIL[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_maps_dots_to_the_right_bits() {
        let mut bitmap = ["............"; 8];
        bitmap[0] = "#...........";
        assert_eq!(braille(&bitmap)[0].chars().next(), Some('\u{2801}'));
        bitmap[0] = ".#..........";
        assert_eq!(braille(&bitmap)[0].chars().next(), Some('\u{2808}'));
        bitmap[0] = "............";
        bitmap[3] = "#...........";
        assert_eq!(braille(&bitmap)[0].chars().next(), Some('\u{2840}'));
    }

    #[test]
    fn every_pose_draws_two_lines_of_six_cells() {
        let poses = [
            Pose::Think,
            Pose::Run,
            Pose::Hunt,
            Pose::Wait,
            Pose::Done,
            Pose::Fail,
        ];
        for pose in poses {
            for tick in [0, 3, 6, 9, 12] {
                let [a, b] = mascot_frame(pose, tick);
                assert_eq!((a.chars().count(), b.chars().count()), (6, 6));
            }
        }
    }

    #[test]
    fn running_changes_between_frames() {
        assert_ne!(mascot_frame(Pose::Run, 0), mascot_frame(Pose::Run, 3));
    }

    #[test]
    fn wait_moonwalks_gliding_to_the_left() {
        // Cabe señalar que la figura avanza un punto a la izquierda cada dos pasos de 150 ms: tras
        // cuatro pasos (tick 12) el ala del sombrero ha cruzado a la celda anterior.
        let hueco = |tick: u64| {
            let [alto, _] = mascot_frame(Pose::Wait, tick);
            alto.chars().position(|c| c != '\u{2800}').unwrap()
        };
        assert!(
            hueco(12) < hueco(0),
            "el paseo debe deslizarse a la izquierda"
        );
        // Las piernas cambian entre pasos consecutivos y el reloj da la vuelta: no hay salto raro.
        assert_ne!(mascot_frame(Pose::Wait, 0), mascot_frame(Pose::Wait, 3));
        // El ciclo completo (ida, vuelta y piernas) dura 16 pasos de 150 ms: 48 ticks.
        assert_eq!(
            mascot_frame(Pose::Wait, 0),
            mascot_frame(Pose::Wait, 3 * 16 * 3)
        );
        // Y `Done` recupera su celebración con los brazos en alto, que no se desliza.
        let [alto, _] = mascot_frame(Pose::Done, 0);
        assert_ne!(alto, mascot_frame(Pose::Wait, 0)[0]);
    }

    #[test]
    fn wait_turns_around_at_the_wall_and_glides_back() {
        // Cabe señalar que, al llegar a la pared izquierda (paso 8, tick 24), el muñeco se da la
        // vuelta y regresa hacia la derecha: el ala del sombrero vuelve a cruzar de celda.
        let hueco = |tick: u64| {
            let [alto, _] = mascot_frame(Pose::Wait, tick);
            alto.chars().position(|c| c != '\u{2800}').unwrap()
        };
        assert!(
            hueco(42) > hueco(24),
            "tras la vuelta debe deslizarse a la derecha"
        );
        // Sin saltos: de un punto al siguiente la figura solo se mueve un punto como mucho,
        // y la vuelta no deja el sombrero más lejos que eso de donde estaba.
        let ala = |tick: u64| {
            let [alto, _] = mascot_frame(Pose::Wait, tick);
            alto.chars().position(|c| c != '\u{2800}').unwrap()
        };
        for tick in (0..96u64).step_by(6) {
            let delta = ala(tick + 6).abs_diff(ala(tick));
            assert!(delta <= 1, "salto de {delta} celdas en el tick {tick}");
        }
        // El espejo cambia el dibujo: la vuelta no repite el mismo fotograma de la ida.
        assert_ne!(mascot_frame(Pose::Wait, 21), mascot_frame(Pose::Wait, 27));
    }

    #[test]
    fn fail_is_static() {
        assert_eq!(mascot_frame(Pose::Fail, 0), mascot_frame(Pose::Fail, 100));
    }
}
