//! Efectos del logo sobre el `Buffer`. Cada uno recibe el progreso ya acotado y solo escribe
//! en el área de la marca.

use std::f32::consts::PI;

use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::state::StyleToken;
use crate::widgets::Theme;

/// Radio horizontal del foco de la linterna, en columnas.
const FOCO_X: f32 = 5.0;
/// Radio vertical del foco de la linterna, en filas.
const FOCO_Y: f32 = 2.0;
/// Glifos del efecto tipográfico, mientras las letras no están resueltas.
const GLIFOS: [char; 6] = ['░', '▒', '▓', '#', '%', '&'];

/// Contexto de un dibujo: área ya recortada al buffer, progreso normalizado y tema.
pub(super) struct Ctx<'a> {
    pub(super) area: Rect,
    pub(super) p: f32,
    pub(super) seed: u64,
    pub(super) theme: &'a Theme,
}

impl Ctx<'_> {
    /// Celdas del área como `(x, y, columna relativa, fila relativa)`.
    pub(super) fn celdas(&self) -> impl Iterator<Item = (u16, u16, u16, u16)> + '_ {
        let a = self.area;
        (0..a.height).flat_map(move |ry| (0..a.width).map(move |rx| (a.x + rx, a.y + ry, rx, ry)))
    }

    pub(super) fn color_fg(&self, token: StyleToken) -> Option<Color> {
        self.theme.style(token).fg
    }

    /// Estilo de «aclarado»: negrita y el color más vivo del tema.
    pub(super) fn luz(&self) -> Style {
        let base = Style::default().add_modifier(Modifier::BOLD);
        match self.theme.styles.heading.fg {
            Some(fg) => base.fg(fg),
            None => base,
        }
    }

    pub(super) fn atenuado() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    pub(super) fn flashlight(&self, buf: &mut Buffer) {
        if self.p <= 0.0 || self.p >= 1.0 {
            return;
        }
        let w = f32::from(self.area.width);
        let h = f32::from(self.area.height);
        let eased = 0.5 - 0.5 * (PI * self.p).cos();
        let recorrido = (w - 1.0).max(0.0);
        let cx = if self.seed.is_multiple_of(2) {
            eased * recorrido
        } else {
            recorrido - eased * recorrido
        };
        let cy = (h - 1.0) / 2.0;
        for (x, y, rx, ry) in self.celdas() {
            let dx = (f32::from(rx) - cx) / FOCO_X;
            let dy = (f32::from(ry) - cy) / FOCO_Y;
            let d = (dx * dx + dy * dy).sqrt();
            let estilo = if d <= 1.0 {
                self.luz()
            } else if d > 1.6 {
                Self::atenuado()
            } else {
                continue;
            };
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(estilo);
            }
        }
    }

    pub(super) fn binary_rain(&self, buf: &mut Buffer) {
        let h = f32::from(self.area.height);
        let ciclo = h + 2.0;
        let tenue = self.theme.style(StyleToken::Muted);
        for (x, y, rx, ry) in self.celdas() {
            let carril = hash(self.seed, u64::from(rx), 0);
            if carril.is_multiple_of(3) {
                continue;
            }
            let fase = unit(carril) * ciclo;
            let cabeza = (self.p * ciclo * 2.0 + fase).rem_euclid(ciclo);
            let distancia = cabeza - f32::from(ry);
            if !(0.0..2.0).contains(&distancia) {
                continue;
            }
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            if cell.symbol() != " " {
                continue;
            }
            let digito = if hash(self.seed, u64::from(rx), u64::from(ry)) & 1 == 0 {
                '0'
            } else {
                '1'
            };
            cell.set_char(digito);
            cell.set_style(tenue);
            cell.set_style(Self::atenuado());
        }
    }

    pub(super) fn shimmer(&self, buf: &mut Buffer) {
        let range = f32::from(self.area.width.saturating_sub(1))
            + 2.0 * f32::from(self.area.height.saturating_sub(1));
        let pos = -2.0 + self.p * (range + 4.0);
        for (x, y, rx, ry) in self.celdas() {
            let diag = f32::from(rx) + 2.0 * f32::from(ry);
            if (diag - pos).abs() > 1.5 {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                if cell.symbol() != " " {
                    cell.set_style(self.luz());
                }
            }
        }
    }

    pub(super) fn scramble(&self, buf: &mut Buffer) {
        if self.p <= 0.0 || self.p >= 1.0 {
            return;
        }
        let paso = (self.p * 16.0) as u64;
        let ancho = u64::from(self.area.width);
        for (x, y, rx, ry) in self.celdas() {
            let celda = u64::from(rx) + u64::from(ry) * ancho;
            if unit(hash(self.seed, celda, 0)) <= self.p {
                continue;
            }
            let elegido = hash(self.seed, celda, paso) as usize % GLIFOS.len();
            if let Some(cell) = buf.cell_mut((x, y)) {
                if cell.symbol() != " " {
                    cell.set_char(GLIFOS[elegido]);
                }
            }
        }
    }

    pub(super) fn glitch(&self, buf: &mut Buffer) {
        if self.p <= 0.0 || self.p >= 1.0 {
            return;
        }
        let paso = (self.p * 12.0) as u64;
        let ancho = self.area.width;
        for ry in 0..self.area.height {
            if !hash(self.seed, u64::from(ry), paso).is_multiple_of(4) {
                continue;
            }
            let desplazamiento: i32 = if hash(self.seed, u64::from(ry), paso + 1).is_multiple_of(2)
            {
                1
            } else {
                -1
            };
            let y = self.area.y + ry;
            let fila: Vec<Cell> = (0..ancho)
                .map(|rx| buf[(self.area.x + rx, y)].clone())
                .collect();
            for rx in 0..ancho {
                let origen = i32::from(rx) - desplazamiento;
                let nueva = if (0..i32::from(ancho)).contains(&origen) {
                    fila[origen as usize].clone()
                } else {
                    Cell::default()
                };
                buf[(self.area.x + rx, y)] = nueva;
            }
        }
        let Some(tinta) = self.color_fg(StyleToken::Error) else {
            return;
        };
        let ancho_u = u64::from(ancho);
        for (x, y, rx, ry) in self.celdas() {
            let celda = u64::from(rx) + u64::from(ry) * ancho_u;
            if !hash(self.seed ^ 0x51, celda, paso).is_multiple_of(9) {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(Style::default().fg(tinta));
            }
        }
    }

    pub(super) fn dissolve(&self, buf: &mut Buffer) {
        if self.p <= 0.0 || self.p >= 1.0 {
            return;
        }
        let umbral = (PI * self.p).sin();
        let ancho = u64::from(self.area.width);
        for (x, y, rx, ry) in self.celdas() {
            let celda = u64::from(rx) + u64::from(ry) * ancho;
            if unit(hash(self.seed, celda, 1)) >= umbral {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                if cell.symbol() != " " {
                    cell.set_char(' ');
                }
            }
        }
    }

    pub(super) fn pulse(&self, buf: &mut Buffer) {
        let onda = (2.0 * PI * self.p).sin();
        let estilo = if onda > 0.3 {
            self.luz()
        } else if onda < -0.3 {
            Self::atenuado()
        } else {
            return;
        };
        for (x, y, _, _) in self.celdas() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(estilo);
            }
        }
    }
}

/// Mezcla determinista de tres valores en 64 bits (splitmix64).
fn hash(seed: u64, a: u64, b: u64) -> u64 {
    let mut z =
        seed ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Convierte un hash en un valor de `0.0..1.0`.
fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}
