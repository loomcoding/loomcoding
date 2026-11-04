//! Navegación: siguiente y anterior, foco, salida de una demo y tick.

use super::*;

impl Showcase {
    /// Mueve la selección una posición abajo (con tope) y vuelve el foco al índice.
    pub fn next(&mut self) {
        let filtered = self.filtered_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            if position + 1 < filtered.len() {
                self.selected = filtered[position + 1];
            }
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
        self.focus = Focus::Index;
        self.matrix_return = false;
    }

    /// Mueve la selección una posición arriba (con tope) y vuelve el foco al índice.
    pub fn prev(&mut self) {
        let filtered = self.filtered_indices();
        if let Some(position) = filtered.iter().position(|index| *index == self.selected) {
            self.selected = filtered[position.saturating_sub(1)];
        } else if let Some(first) = filtered.first() {
            self.selected = *first;
        }
        self.focus = Focus::Index;
        self.matrix_return = false;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Index => Focus::Demo,
            _ => Focus::Index,
        };
        // Cualquier salida de Focus::Demo por Tab deja el demo sin procedencia de la matriz.
        self.matrix_return = false;
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        if focus != Focus::Demo {
            self.matrix_return = false;
        }
    }

    /// Sale del demo con `Esc`: vuelve a la matriz si el demo se abrió desde ella, si no al índice.
    pub fn leave_demo(&mut self) {
        self.focus = if self.matrix_return {
            Focus::Matrix
        } else {
            Focus::Index
        };
        self.matrix_return = false;
    }

    pub fn current(&mut self) -> Option<&mut Box<dyn Demo>> {
        self.demos
            .get_mut(self.selected)
            .map(|entry| &mut entry.demo)
    }

    /// Avanza la animación del demo activo.
    pub fn tick(&mut self, anim: &Anim) {
        if let Some(d) = self.current() {
            d.tick(anim);
        }
    }
}
