//! Filas del panel `/ui`: nombre estable, etiqueta visible, valor vigente y ciclo de cada una.
//!
//! Cada fila reutiliza el método de `App` que ya guarda su valor (`set_theme`, `set_style`,
//! `set_layout`, `set_mascot`, `set_mouse_capture`, `persist_spinner`...). Aquí no se duplica
//! ninguna lógica de persistencia.

use super::*;
use loomcoding_cloth::widgets::{LayoutStyle, StyleBundle, ThemePreset};

/// Orden de rotación de la fila «Distribución». No es `LayoutStyle::next`, que no incluye
/// `Workspace` y pertenece a `/layout`.
const LAYOUT_ROW_ORDER: [LayoutStyle; 4] = [
    LayoutStyle::ContextRail,
    LayoutStyle::Workspace,
    LayoutStyle::Minimal,
    LayoutStyle::Cards,
];

/// Filas del panel de apariencia, en el orden en que se muestran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRow {
    Theme,
    Style,
    Layout,
    Effects,
    Spinner,
    Mascot,
    Sidebar,
    Mouse,
}

impl UiRow {
    /// Todas las filas, en el orden del panel.
    pub const ALL: [UiRow; 8] = [
        UiRow::Theme,
        UiRow::Style,
        UiRow::Layout,
        UiRow::Effects,
        UiRow::Spinner,
        UiRow::Mascot,
        UiRow::Sidebar,
        UiRow::Mouse,
    ];

    /// Clave estable de la fila (la que usa `/ui`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Style => "style",
            Self::Layout => "layout",
            Self::Effects => "effects",
            Self::Spinner => "spinner",
            Self::Mascot => "mascot",
            Self::Sidebar => "sidebar",
            Self::Mouse => "mouse",
        }
    }

    /// Resuelve una clave producida por `name`; cualquier otro texto da `None`.
    pub fn from_name(name: &str) -> Option<UiRow> {
        Self::ALL.into_iter().find(|row| row.name() == name)
    }

    /// Etiqueta visible en español.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "Tema",
            Self::Style => "Estilo",
            Self::Layout => "Distribución",
            Self::Effects => "Efectos",
            Self::Spinner => "Spinner",
            Self::Mascot => "Muñeco",
            Self::Sidebar => "Menú lateral",
            Self::Mouse => "Ratón",
        }
    }

    /// Indica si la fila abre un submenú con `Enter`.
    pub const fn has_submenu(self) -> bool {
        matches!(self, Self::Theme | Self::Effects | Self::Spinner)
    }

    /// Indica si la fila es un interruptor on/off.
    pub const fn is_toggle(self) -> bool {
        matches!(self, Self::Mascot | Self::Sidebar | Self::Mouse)
    }
}

/// Texto on/off para las filas de interruptor y para el resumen de Efectos.
const fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

/// Índice siguiente en un ciclo de `len` posiciones. Si el valor actual no está en el ciclo
/// (un look mixto, por ejemplo), avanza al primero y retrocede al último.
fn step(actual: Option<usize>, len: usize, forward: bool) -> usize {
    match (actual, forward) {
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    }
}

impl App {
    /// Valor vigente de la fila, listo para pintar a la derecha.
    pub fn ui_row_value(&self, row: UiRow) -> String {
        match row {
            UiRow::Theme => self.theme_preset.name().to_string(),
            UiRow::Style => self
                .theme
                .look
                .bundle()
                .map_or("custom", StyleBundle::name)
                .to_string(),
            UiRow::Layout => self.theme.look.layout.name().to_string(),
            UiRow::Effects => format!(
                "matrix {} · logo {}",
                on_off(self.matrix_rain.is_emitting()),
                on_off(self.logo_fx_on)
            ),
            UiRow::Spinner => self.spinner_choice.label(),
            UiRow::Mascot => on_off(self.mascot).to_string(),
            UiRow::Sidebar => on_off(self.sidebar_visible()).to_string(),
            UiRow::Mouse => on_off(self.mouse_capture).to_string(),
        }
    }

    /// Cambia el valor de la fila sin abrir nada y devuelve el aviso con el valor nuevo
    /// (`"layout: rail"`). Devuelve `None` si la fila no se cicla («Efectos» se abre con `Enter`).
    pub fn ui_row_cycle(&mut self, row: UiRow, forward: bool) -> Option<String> {
        match row {
            UiRow::Theme => {
                let presets = ThemePreset::ALL;
                let actual = presets.iter().position(|p| *p == self.theme_preset);
                self.set_theme(presets[step(actual, presets.len(), forward)]);
            }
            UiRow::Style => {
                let actual = self.theme.look.bundle();
                let bundles = StyleBundle::ALL;
                let indice = bundles.iter().position(|b| Some(*b) == actual);
                self.set_style(bundles[step(indice, bundles.len(), forward)]);
            }
            UiRow::Layout => {
                let actual = LAYOUT_ROW_ORDER
                    .iter()
                    .position(|l| *l == self.theme.look.layout);
                self.set_layout(LAYOUT_ROW_ORDER[step(actual, LAYOUT_ROW_ORDER.len(), forward)]);
            }
            UiRow::Effects => return None,
            UiRow::Spinner => {
                let actual = SPINNER_CHOICES
                    .iter()
                    .position(|c| *c == self.spinner_choice);
                let next = SPINNER_CHOICES[step(actual, SPINNER_CHOICES.len(), forward)];
                self.spinner_choice = next;
                // El composer refleja la elección sin esperar a la próxima petición.
                self.composer.set_activity_style(next.style_for(0));
                self.persist_spinner();
            }
            UiRow::Mascot => self.set_mascot(!self.mascot),
            UiRow::Sidebar => self.toggle_sidebar(),
            UiRow::Mouse => self.toggle_mouse_capture(),
        }
        Some(format!("{}: {}", row.name(), self.ui_row_value(row)))
    }

    /// Si el menú lateral se ve, según la disposición: el rail con `ContextRail`, la tarjeta de
    /// agentes con `Cards` y el sidebar izquierdo con el resto (los mismos campos que
    /// `toggle_sidebar` alterna, para que el valor y el atajo `Ctrl+B` coincidan).
    fn sidebar_visible(&self) -> bool {
        match self.theme.look.layout {
            LayoutStyle::ContextRail => !self.rail_hidden,
            LayoutStyle::Cards => !self.agents_card_folded,
            _ => self.sidebar_expanded,
        }
    }
}
