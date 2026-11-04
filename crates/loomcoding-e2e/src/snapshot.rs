//! Volcado de la pantalla de un emulador de terminal: texto y rejilla de celdas con estilo.

use serde::{Deserialize, Serialize};

/// Color de una celda tal como lo reporta `vt100`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSnap {
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

impl From<vt100::Color> for ColorSnap {
    fn from(color: vt100::Color) -> Self {
        match color {
            vt100::Color::Default => ColorSnap::Default,
            vt100::Color::Idx(i) => ColorSnap::Idx(i),
            vt100::Color::Rgb(r, g, b) => ColorSnap::Rgb(r, g, b),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CellSnap {
    pub ch: String,
    pub fg: ColorSnap,
    pub bg: ColorSnap,
    pub bold: bool,
    pub inverse: bool,
    pub underline: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: u16,
    pub cols: u16,
    /// (fila, columna) del cursor, empezando en 0.
    pub cursor: (u16, u16),
    /// Pantalla completa como texto, una línea por fila y sin espacios finales.
    pub text: String,
    pub cells: Vec<Vec<CellSnap>>,
}

impl Snapshot {
    pub fn from_screen(screen: &vt100::Screen) -> Snapshot {
        let (rows, cols) = screen.size();
        let cells = (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| match screen.cell(r, c) {
                        Some(cell) => CellSnap {
                            ch: cell.contents(),
                            fg: cell.fgcolor().into(),
                            bg: cell.bgcolor().into(),
                            bold: cell.bold(),
                            inverse: cell.inverse(),
                            underline: cell.underline(),
                        },
                        None => CellSnap {
                            ch: String::new(),
                            fg: ColorSnap::Default,
                            bg: ColorSnap::Default,
                            bold: false,
                            inverse: false,
                            underline: false,
                        },
                    })
                    .collect()
            })
            .collect::<Vec<Vec<CellSnap>>>();
        let text = cells
            .iter()
            .map(|row| row_to_text(row))
            .collect::<Vec<_>>()
            .join("\n");
        Snapshot {
            rows,
            cols,
            cursor: screen.cursor_position(),
            text,
            cells,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("un Snapshot siempre es serializable")
    }

    /// Texto de una fila sin los espacios finales; vacío si la fila no existe.
    pub fn row_text(&self, row: usize) -> String {
        self.cells
            .get(row)
            .map(|r| row_to_text(r))
            .unwrap_or_default()
    }
}

fn row_to_text(row: &[CellSnap]) -> String {
    let line: String = row
        .iter()
        .map(|c| if c.ch.is_empty() { " " } else { c.ch.as_str() })
        .collect();
    line.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser_with(bytes: &[u8]) -> vt100::Parser {
        let mut parser = vt100::Parser::new(5, 20, 0);
        parser.process(bytes);
        parser
    }

    #[test]
    fn captures_text_and_colors() {
        let parser = parser_with(b"\x1b[31mhola\x1b[0m x");
        let snap = Snapshot::from_screen(parser.screen());
        assert_eq!(snap.row_text(0), "hola x");
        assert_eq!(snap.cells[0][0].fg, ColorSnap::Idx(1));
        assert_eq!(snap.cells[0][5].fg, ColorSnap::Default);
    }

    #[test]
    fn json_round_trips() {
        let parser = parser_with(b"\x1b[1;4mhi\x1b[0m");
        let snap = Snapshot::from_screen(parser.screen());
        let back: Snapshot = serde_json::from_str(&snap.to_json()).unwrap();
        assert_eq!(back, snap);
        assert!(back.cells[0][0].bold && back.cells[0][0].underline);
    }

    #[test]
    fn records_cursor_and_size() {
        let parser = parser_with(b"ab");
        let snap = Snapshot::from_screen(parser.screen());
        assert_eq!(snap.cursor, (0, 2));
        assert_eq!((snap.rows, snap.cols), (5, 20));
    }
}
