//! Comando `/ui`: el panel de apariencia desde la línea de comandos.
//!
//! `/ui` abre el panel. `/ui <fila> [valor]` cambia una fila sin abrirlo: sin valor, la fila
//! rota o alterna como en el panel; con valor, lo fija. Cabe señalar que todo cambio pasa por
//! los mismos setters que usa el panel, de modo que lo que se guarda en `ui.toml` es idéntico.

use super::ui_panel::UiRow;
use super::*;
use loomcoding_cloth::widgets::{LayoutStyle, StyleBundle, ThemePreset};

/// Nombre con el que se pide un preset en `/ui theme`: el tramo anterior a «·» de `name()`,
/// en minúsculas (`"Crush · Charmtone"` da `crush`), igual que la clave de `ui.toml`.
fn theme_key(preset: ThemePreset) -> String {
    preset
        .name()
        .split('·')
        .next()
        .unwrap_or_default()
        .trim()
        .to_lowercase()
}

/// Nombres de las filas, separados por comas, para el aviso de fila desconocida.
fn row_names() -> String {
    UiRow::ALL.map(UiRow::name).join(", ")
}

/// Aviso de valor desconocido, con las opciones válidas de la fila.
fn unknown_value(row: UiRow, value: &str, options: &str) -> String {
    format!("Unknown {}: {value}. Options: {options}", row.name())
}

/// Interpreta `on` u `off` (sin distinguir mayúsculas) para las filas de interruptor.
fn parse_on_off(row: UiRow, value: &str) -> Result<bool, String> {
    match value.to_lowercase().as_str() {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(unknown_value(row, value, "on, off")),
    }
}

/// Disposiciones que admite `/ui layout`, en el orden en que se listan.
const LAYOUT_CHOICES: [LayoutStyle; 4] = [
    LayoutStyle::Workspace,
    LayoutStyle::ContextRail,
    LayoutStyle::Minimal,
    LayoutStyle::Cards,
];

impl App {
    /// Ejecuta `/ui [fila [valor]]`. Sin argumentos abre el panel; con fila, la cambia.
    /// Un `Err` describe el rechazo: el llamador lo deja en `slash_feedback` y conserva el
    /// texto escrito. Un `Ok` deja su aviso `clave: valor` en `slash_feedback`, salvo que el
    /// guardado haya fallado, que prevalece.
    pub fn ui_command(&mut self, arg: &str) -> Result<(), String> {
        let mut partes = arg.split_whitespace();
        let Some(nombre) = partes.next() else {
            self.open_temas();
            return Ok(());
        };
        let valor = partes.collect::<Vec<_>>().join(" ");
        let row = UiRow::from_name(&nombre.to_lowercase())
            .ok_or_else(|| format!("Unknown row: {nombre}. Rows: {}", row_names()))?;

        if valor.is_empty() {
            // «Efectos» no cicla: sin valor abre su submenú, como `Enter` en la raíz del panel.
            if row == UiRow::Effects {
                self.open_temas();
                self.temas_level = TemasLevel::Effects;
                self.temas_selected = 0;
                return Ok(());
            }
            self.slash_feedback = None;
            let aviso = self.ui_row_cycle(row, true);
            if self.slash_feedback.is_none() {
                self.slash_feedback = aviso;
            }
            return Ok(());
        }

        self.slash_feedback = None;
        let aviso = self.ui_row_set(row, &valor)?;
        if self.slash_feedback.is_none() {
            self.slash_feedback = Some(aviso);
        }
        Ok(())
    }

    /// Fija el valor explícito de una fila (`/ui <fila> <valor>`) con el setter que ya usa el
    /// panel, y devuelve el aviso `clave: valor`. Un valor no válido da `Err` con las opciones
    /// reales de la fila y no cambia nada.
    pub fn ui_row_set(&mut self, row: UiRow, value: &str) -> Result<String, String> {
        let lower = value.to_lowercase();
        match row {
            UiRow::Theme => {
                let preset = ThemePreset::ALL
                    .into_iter()
                    .find(|preset| theme_key(*preset) == lower)
                    .ok_or_else(|| {
                        let opciones: Vec<String> =
                            ThemePreset::ALL.into_iter().map(theme_key).collect();
                        unknown_value(row, value, &opciones.join(", "))
                    })?;
                self.set_theme(preset);
            }
            UiRow::Style => {
                let bundle = StyleBundle::from_name(&lower).ok_or_else(|| {
                    let opciones: Vec<&str> = StyleBundle::ALL.iter().map(|b| b.name()).collect();
                    unknown_value(row, value, &opciones.join(", "))
                })?;
                self.set_style(bundle);
            }
            UiRow::Layout => {
                let layout = LayoutStyle::from_name(&lower).ok_or_else(|| {
                    let opciones: Vec<&str> = LAYOUT_CHOICES.iter().map(|l| l.name()).collect();
                    unknown_value(row, value, &opciones.join(", "))
                })?;
                self.set_layout(layout);
            }
            UiRow::Mascot => {
                let on = parse_on_off(row, value)?;
                self.set_mascot(on);
            }
            UiRow::Mouse => {
                let on = parse_on_off(row, value)?;
                self.set_mouse_capture(on);
            }
            UiRow::Sidebar => {
                // El menú lateral solo sabe alternar: se alterna únicamente si no está ya como se pide.
                let on = parse_on_off(row, value)?;
                if (self.ui_row_value(row) == "on") != on {
                    self.toggle_sidebar();
                }
            }
            UiRow::Effects | UiRow::Spinner => {
                return Err(format!("Use the panel: /ui {}", row.name()));
            }
        }
        Ok(format!("{}: {}", row.name(), self.ui_row_value(row)))
    }
}
