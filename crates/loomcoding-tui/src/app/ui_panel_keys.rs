//! Teclas de la raíz del panel `/ui` (la ventana `/temas` con sus ocho filas).
//!
//! `←`/`→` cambian el valor de la fila seleccionada sin entrar en ella. `Enter` abre el
//! submenú de Tema, Efectos o Spinner y, en las demás filas, cicla el valor hacia delante.
//! Cada valor se cambia con `App::ui_row_cycle`, que ya guarda en `ui.toml`; aquí no se
//! repite esa lógica.

use super::ui_panel::UiRow;
use super::*;

impl UiRow {
    /// Posición de la fila en la raíz: el `temas_selected` que la señala.
    pub(super) fn root_index(self) -> usize {
        UiRow::ALL.iter().position(|row| *row == self).unwrap_or(0)
    }
}

impl App {
    /// Fila seleccionada en la raíz, si el índice cae dentro de `UiRow::ALL`.
    fn ui_root_selected(&self) -> Option<UiRow> {
        UiRow::ALL.get(self.temas_selected).copied()
    }

    /// `←`/`→` en la raíz: cambia el valor de la fila seleccionada sin entrar en ella.
    ///
    /// El aviso con el valor nuevo queda en `slash_feedback`. Si el guardado falla, prevalece
    /// el aviso que dejó el setter, que describe el error.
    pub(super) fn ui_root_cycle(&mut self, forward: bool) {
        let Some(row) = self.ui_root_selected() else {
            return;
        };
        self.slash_feedback = None;
        let aviso = self.ui_row_cycle(row, forward);
        if self.slash_feedback.is_none() {
            self.slash_feedback = aviso;
        }
    }

    /// `Enter` en la raíz: entra en el submenú de la fila o, si no tiene, la cicla.
    pub(super) fn ui_root_enter(&mut self) {
        let Some(row) = self.ui_root_selected() else {
            return;
        };
        let nivel = match row {
            UiRow::Theme => TemasLevel::Theme,
            UiRow::Effects => TemasLevel::Effects,
            UiRow::Spinner => TemasLevel::Spinner,
            _ => return self.ui_root_cycle(true),
        };
        self.temas_level = nivel;
        self.temas_selected = 0;
    }
}
