//! Estado del showcase: tema activo, catálogo, selección y foco.

use crate::demo::Demo;
use crate::demos::{self, CatalogItem};
use crate::matrix::{self, Cell, Matrix};
use crate::theme_view::{self, ThemeEntry, ThemeExample, ThemeView};
use loomcoding_cloth::widgets::{Anim, ThemePreset};
use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

mod grid;
mod index;
mod nav;
mod route;
mod theme;

pub use route::route;
use route::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Index,
    Demo,
    Matrix,
    TextFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    None,
}

/// Fila del índice lateral. El texto no lleva el símbolo del componente: el dibujo lo pinta
/// aparte, con su estilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRow {
    /// Índice del catálogo del ejemplo; `None` en las cabeceras y en el aviso «Sin resultados».
    pub catalog_index: Option<usize>,
    /// Texto de la fila. Las filas hijas van ya sangradas con dos espacios.
    pub text: String,
    /// `true` si la fila es de un componente (cabecera o fila única) y lleva símbolo.
    pub marked: bool,
    /// Solo con `marked`: `true` si el símbolo es el del genérico (`◐`), `false` si es propio (`●`).
    pub generic: bool,
}

pub struct Showcase {
    demos: Vec<CatalogItem>,
    /// Índice del catálogo del demo seleccionado (el de un ejemplo, nunca el de una cabecera).
    selected: usize,
    focus: Focus,
    quit: bool,
    query: String,
    /// Matriz de componentes por tema, derivada una vez del catálogo.
    matrix: Matrix,
    /// Celda bajo el cursor de la matriz: (fila, columna), siempre dentro de los límites.
    matrix_cursor: (usize, usize),
    /// `true` si el demo activo se abrió desde la matriz; `Esc` vuelve entonces a ella.
    matrix_return: bool,
    /// Columna de `matrix::COLUMNS` del tema activo. El tema es estado propio del índice.
    theme_column: usize,
    /// Vista del tema activo: sus componentes y ejemplos. Se recalcula al cambiar de tema.
    view: ThemeView,
}

impl Showcase {
    pub fn new() -> Self {
        let demos = demos::catalog();
        let matrix = matrix::build(&demos);
        let view = theme_view::theme_view(&demos, 0);
        Self {
            demos,
            selected: 0,
            focus: Focus::Index,
            quit: false,
            query: String::new(),
            matrix,
            matrix_cursor: (0, 0),
            matrix_return: false,
            theme_column: 0,
            view,
        }
    }

    /// Vista del tema activo: componentes y ejemplos que el índice muestra.
    pub fn view(&self) -> &ThemeView {
        &self.view
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    #[cfg(test)]
    pub(crate) fn focus_demo_for_test(&mut self) {
        self.focus = Focus::Demo;
    }

    /// Construye un showcase de prueba con un demo por entrada.
    ///
    /// Solo para tests de navegación: el proveedor «Test» no existe en la matriz, así que la vista
    /// se construye a mano con una entrada de un único ejemplo por demo (todas bajo «Tests»).
    #[cfg(test)]
    pub(crate) fn with_demos(demos: Vec<Box<dyn Demo>>) -> Self {
        let demos: Vec<CatalogItem> = demos
            .into_iter()
            .map(|demo| CatalogItem {
                component: "Tests",
                variant: "Test",
                demo,
            })
            .collect();
        let matrix = matrix::build(&demos);
        let view = ThemeView {
            entries: (0..demos.len())
                .map(|catalog_index| ThemeEntry {
                    component: "Tests",
                    generic: false,
                    examples: vec![ThemeExample {
                        catalog_index,
                        label: "Test".to_string(),
                    }],
                })
                .collect(),
            missing: 0,
        };
        Self {
            demos,
            selected: 0,
            focus: Focus::Index,
            quit: false,
            query: String::new(),
            matrix,
            matrix_cursor: (0, 0),
            matrix_return: false,
            theme_column: 0,
            view,
        }
    }
}
