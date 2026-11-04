//! Piezas comunes de los tests.

use super::*;

/// Demo vacía para tests de navegación, independientes del catálogo real.
pub(super) struct Dummy(&'static str);

pub(super) const NAMES: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];

impl Demo for Dummy {
    fn title(&self) -> &str {
        self.0
    }
    fn hint(&self) -> &str {
        ""
    }
    fn render(
        &mut self,
        _f: &mut ratatui::Frame,
        _a: ratatui::layout::Rect,
        _t: &batten::widgets::Theme,
    ) {
    }
}

pub(super) fn dummy_show(n: usize) -> Showcase {
    let demos: Vec<Box<dyn Demo>> = (0..n)
        .map(|i| Box::new(Dummy(NAMES[i])) as Box<dyn Demo>)
        .collect();
    Showcase::with_demos(demos)
}

pub(super) struct Rec(std::rc::Rc<std::cell::Cell<Option<KeyModifiers>>>);

impl Demo for Rec {
    fn title(&self) -> &str {
        "rec"
    }
    fn hint(&self) -> &str {
        ""
    }
    fn render(
        &mut self,
        _f: &mut ratatui::Frame,
        _a: ratatui::layout::Rect,
        _t: &batten::widgets::Theme,
    ) {
    }
    fn handle_key_event(&mut self, _code: KeyCode, mods: KeyModifiers) -> bool {
        self.0.set(Some(mods));
        true
    }
}

pub(super) fn press(show: &mut Showcase, code: KeyCode) {
    route(show, KeyEventKind::Press, code, KeyModifiers::NONE);
}

/// Índice de columna de la matriz por nombre.
pub(super) fn column_index(name: &str) -> usize {
    matrix::COLUMNS
        .iter()
        .position(|(column, _)| *column == name)
        .unwrap()
}

/// Coloca el cursor de la matriz en la celda (componente, columna), buscando por nombre.
pub(super) fn place(show: &mut Showcase, component: &str, column: &str) {
    let row = show
        .matrix()
        .rows
        .iter()
        .position(|row| row.component == component)
        .unwrap_or_else(|| panic!("sin fila para {component}"));
    show.matrix_cursor = (row, column_index(column));
}

/// Primer demo del catálogo con ese componente y esa variante (por prefijo de proveedor).
pub(super) fn first_demo(show: &Showcase, component: &str, provider_prefix: &str) -> usize {
    show.demos
        .iter()
        .position(|item| item.component == component && item.variant.starts_with(provider_prefix))
        .unwrap()
}

/// Componente de la primera fila cuya celda en esa columna cumple `pred`.
pub(super) fn component_where(
    show: &Showcase,
    column: usize,
    pred: impl Fn(Cell) -> bool,
) -> &'static str {
    show.matrix()
        .rows
        .iter()
        .find(|row| pred(row.cells[column]))
        .map(|row| row.component)
        .expect("ninguna fila cumple la condición en el catálogo real")
}
