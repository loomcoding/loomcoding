//! Índice del showcase: filas visibles, filtro de texto y selección.

use super::*;

impl Showcase {
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Entradas de la vista que pasan el filtro de texto, con sus ejemplos visibles.
    ///
    /// Un componente entra entero si su nombre contiene el texto; si no, solo entran sus ejemplos
    /// cuya etiqueta lo contiene, y si ninguno lo hace el componente se omite. Sin texto, todo.
    pub(super) fn visible(&self) -> Vec<(&ThemeEntry, Vec<&ThemeExample>)> {
        let query = self.query.to_lowercase();
        self.view
            .entries
            .iter()
            .filter_map(|entry| {
                if query.is_empty() || entry.component.to_lowercase().contains(&query) {
                    return Some((entry, entry.examples.iter().collect()));
                }
                let examples: Vec<&ThemeExample> = entry
                    .examples
                    .iter()
                    .filter(|example| example.label.to_lowercase().contains(&query))
                    .collect();
                (!examples.is_empty()).then_some((entry, examples))
            })
            .collect()
    }

    /// Índices del catálogo seleccionables, en orden de la vista. Son los ejemplos visibles: la
    /// cabecera de un componente con varios ejemplos no es seleccionable.
    pub fn filtered_indices(&self) -> Vec<usize> {
        self.visible()
            .iter()
            .flat_map(|(_, examples)| examples.iter().map(|example| example.catalog_index))
            .collect()
    }

    /// Filas del índice.
    ///
    /// Un componente con un único ejemplo visible es una sola fila seleccionable. Con varios, una
    /// cabecera sin índice (no seleccionable, con `─`) y una fila hija por ejemplo, sangrada con su
    /// etiqueta. Las cabeceras y las filas únicas van marcadas (`marked`) con el símbolo de su tipo.
    pub fn index_rows(&self) -> Vec<IndexRow> {
        let mut rows = Vec::new();
        for (entry, examples) in self.visible() {
            if let [example] = examples.as_slice() {
                rows.push(IndexRow {
                    catalog_index: Some(example.catalog_index),
                    text: entry.component.to_string(),
                    marked: true,
                    generic: entry.generic,
                });
            } else {
                rows.push(IndexRow {
                    catalog_index: None,
                    text: format!("{} ─", entry.component),
                    marked: true,
                    generic: entry.generic,
                });
                for example in examples {
                    rows.push(IndexRow {
                        catalog_index: Some(example.catalog_index),
                        text: format!("  {}", example.label),
                        marked: false,
                        generic: false,
                    });
                }
            }
        }
        if rows.is_empty() {
            rows.push(IndexRow {
                catalog_index: None,
                text: "Sin resultados".to_string(),
                marked: false,
                generic: false,
            });
        }
        rows
    }

    pub fn visible_index_rows(&self, height: u16) -> (usize, Vec<IndexRow>) {
        let rows = self.index_rows();
        let selected_row = rows
            .iter()
            .position(|row| row.catalog_index == Some(self.selected))
            .unwrap_or(0);
        let height = usize::from(height);
        let start = selected_row
            .saturating_sub(height / 2)
            .min(rows.len().saturating_sub(height));
        (start, rows.into_iter().skip(start).take(height).collect())
    }

    pub fn select(&mut self, index: usize, open: bool) {
        if index < self.demos.len() {
            self.selected = index;
            if open {
                self.focus = Focus::Demo;
            }
        }
    }

    pub(super) fn keep_selection_visible(&mut self) {
        let filtered = self.filtered_indices();
        if !filtered.contains(&self.selected) {
            self.selected = filtered.first().copied().unwrap_or(0);
        }
    }

    pub fn push_query(&mut self, character: char) {
        self.query.push(character);
        self.keep_selection_visible();
    }

    pub fn pop_query(&mut self) {
        self.query.pop();
        self.keep_selection_visible();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.keep_selection_visible();
    }

    pub fn component_at(&self, i: usize) -> Option<&str> {
        self.demos.get(i).map(|d| d.component)
    }

    pub fn variant_at(&self, i: usize) -> Option<&str> {
        self.demos.get(i).map(|d| d.variant)
    }

    /// Pista de teclas del demo activo.
    pub fn current_hint(&self) -> &str {
        self.demos
            .get(self.selected)
            .map(|d| d.demo.hint())
            .unwrap_or("")
    }

    pub fn selected(&self) -> usize {
        self.selected
    }
}
