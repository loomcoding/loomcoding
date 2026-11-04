//! Columna de tema: preset activo y ciclo entre columnas.

use super::*;

impl Showcase {
    /// Columna de `matrix::COLUMNS` del tema activo.
    pub fn theme_column(&self) -> usize {
        self.theme_column
    }

    /// Preset del tema activo; se deriva de su columna, no se guarda aparte.
    pub fn theme_preset(&self) -> ThemePreset {
        matrix::COLUMNS
            .get(self.theme_column)
            .and_then(|(_, preset)| *preset)
            .unwrap_or(ThemePreset::Cloth)
    }

    /// Cambia el tema del índice por su columna en `matrix::COLUMNS`; los índices fuera de ella
    /// se ignoran. Recalcula la vista.
    ///
    /// Si el componente seleccionado existe en el tema nuevo, se selecciona su primer ejemplo
    /// visible; si no, el primero de la lista. Con la lista vacía la selección no cambia. No
    /// cambia el foco ni `matrix_return`.
    pub fn set_theme_column(&mut self, column: usize) {
        if column >= matrix::COLUMNS.len() {
            return;
        }
        let previous = self.demos.get(self.selected).map(|item| item.component);
        self.theme_column = column;
        self.view = theme_view::theme_view(&self.demos, column);
        let kept = previous.and_then(|component| self.first_visible_of(component));
        if let Some(index) = kept.or_else(|| self.filtered_indices().first().copied()) {
            self.selected = index;
        }
    }

    /// Primer ejemplo visible del componente en la vista actual, si lo hay.
    pub(super) fn first_visible_of(&self, component: &str) -> Option<usize> {
        self.visible()
            .into_iter()
            .find(|(entry, _)| entry.component == component)
            .and_then(|(_, examples)| examples.first().map(|example| example.catalog_index))
    }

    /// Pasa al tema siguiente (o, con `backwards`, al anterior), con vuelta.
    pub fn cycle_theme_column(&mut self, backwards: bool) {
        let columns = matrix::COLUMNS.len();
        let next = if backwards {
            (self.theme_column + columns - 1) % columns
        } else {
            (self.theme_column + 1) % columns
        };
        self.set_theme_column(next);
    }
}
