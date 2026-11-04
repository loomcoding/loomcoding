//! Matriz de componentes por tema: cursor, celdas y apertura de un ejemplo.

use super::*;

impl Showcase {
    /// Abre la matriz desde el índice.
    pub fn open_matrix(&mut self) {
        self.focus = Focus::Matrix;
    }

    /// Matriz de componentes por tema.
    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    /// Posición (fila, columna) del cursor de la matriz.
    pub fn matrix_cursor(&self) -> (usize, usize) {
        self.matrix_cursor
    }

    /// Celda bajo el cursor de la matriz, o `None` si la matriz no tiene filas.
    pub fn matrix_cell(&self) -> Option<Cell> {
        let (row, column) = self.matrix_cursor;
        self.matrix
            .rows
            .get(row)
            .and_then(|row| row.cells.get(column))
            .copied()
    }

    /// Desplaza el cursor de la matriz con tope en los bordes; no hace nada sin filas.
    pub fn matrix_move(&mut self, down: isize, right: isize) {
        let rows = self.matrix.rows.len();
        if rows == 0 {
            return;
        }
        let (row, column) = self.matrix_cursor;
        self.matrix_cursor = (
            step(row, down, rows),
            step(column, right, matrix::COLUMNS.len()),
        );
    }

    /// Coloca el cursor de la matriz en (fila, columna), con tope en los bordes; no hace nada sin
    /// filas. No abre nada: abrir es cosa de `Enter`.
    pub fn matrix_set_cursor(&mut self, row: usize, column: usize) {
        let rows = self.matrix.rows.len();
        if rows == 0 {
            return;
        }
        self.matrix_cursor = (row.min(rows - 1), column.min(matrix::COLUMNS.len() - 1));
    }

    /// Cambia al tema de la columna de la celda bajo el cursor y abre el demo de esa celda. Las
    /// celdas `Missing` no hacen nada.
    pub fn open_matrix_cell(&mut self) {
        let (row, column) = self.matrix_cursor;
        let Some(component) = self.matrix.rows.get(row).map(|row| row.component) else {
            return;
        };
        let Some(index) = matrix::target(&self.demos, component, column) else {
            return;
        };
        // Un filtro de texto activo podría ocultar el ejemplo que se abre: se limpia antes.
        self.query.clear();
        self.set_theme_column(column);
        self.select(index, true);
        self.matrix_return = true;
    }
}
