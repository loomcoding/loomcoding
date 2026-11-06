//! Tema: tokens semánticos, glifos y carga desde `theme.toml` parcial.

use crate::state::{AgentState, StyleToken};
use crate::widgets::matrix_rain::{GlyphSet, MatrixConfig};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::BTreeMap;

mod loading;
mod look;
mod presets;
mod styles;

pub use loading::ThemeError;
pub use look::{ChromeStyle, ColorDepth, ComposerStyle, LayoutStyle, Look, StyleBundle, ToolStyle};
pub use presets::ThemePreset;
pub use styles::{Glyphs, Styles};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct Theme {
    pub styles: Styles,
    pub glyphs: Glyphs,
    pub extensions: BTreeMap<String, Style>,
    pub depth: ColorDepth,
    /// Pintar los enlaces como hipervínculos OSC 8.
    pub hyperlinks: bool,
    /// Presentación del composer y de las herramientas.
    pub look: Look,
    /// Configuración de la lluvia Matrix, desde `[effects.matrix]`.
    pub matrix: MatrixConfig,
    /// Caja redonda alrededor del mensaje del usuario (carril `box`). Cuando está activa, la
    /// última fila de la entrada del usuario es su borde inferior y se mide como tal. Lo fija la
    /// aplicación según el carril elegido; por defecto, apagada.
    pub rail_box: bool,
}
