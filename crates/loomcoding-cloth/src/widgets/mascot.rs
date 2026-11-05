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
const WAIT: [[&str; 8]; 2] = [
    [
        ".....##..##.",
        ".....##....#",
        "....####..#.",
        "....#.##....",
        ".....##..#..",
        ".....##.....",
        "....#..#....",
        "....#..#....",
    ],
    [
        ".....##.....",
        ".....##.....",
        "....####....",
        "....#.##....",
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
fn braille(bitmap: &[&str; 8]) -> [String; 2] {
    // Bit de cada punto dentro de la celda: (columna, fila) → máscara Unicode braille.
    const BIT: [[u32; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];
    let mut rows = [String::new(), String::new()];
    for (cell_row, out) in rows.iter_mut().enumerate() {
        for cell_col in 0..6 {
            let mut mask = 0u32;
            for (dx, column_bits) in BIT.iter().enumerate() {
                for (dy, &bit) in column_bits.iter().enumerate() {
                    let line = bitmap[cell_row * 4 + dy].as_bytes();
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
        Pose::Wait => braille(&WAIT[(step / 3) % WAIT.len()]),
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
    fn fail_is_static() {
        assert_eq!(mascot_frame(Pose::Fail, 0), mascot_frame(Pose::Fail, 100));
    }
}
