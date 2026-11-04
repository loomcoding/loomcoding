//! Composer: buffer multilínea con cursor. Puro (sin ratatui).

use std::cell::Cell;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

mod editing;
mod motion;
mod pastes;
mod selection;
mod view;

#[cfg(test)]
mod tests;

/// Invariante: `anchor` es `None` o distinto del cursor (una selección vacía
/// se normaliza a `None`), y se expresa en grafemas, igual que `cursor()`.
/// Toda operación que altere el texto la limpia, así que nunca apunta fuera
/// de rango.
///
/// `view_top` es estado de vista, no de edición: la fila superior visible que
/// dejó el último render (filas visuales en Crush, líneas lógicas en Classic).
/// Va en una `Cell` para que el render la actualice con `&Composer`.
///
/// `painted` guarda el cursor con el que se pintó por última vez. La traducción
/// de una celda a un índice (`hit_index`) debe usar esa misma geometría y no el
/// cursor vivo: durante un arrastre el cursor se mueve antes del siguiente repintado.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CollapsedPaste {
    start: usize,
    end: usize,
    text: String,
}

#[derive(Debug)]
pub struct Composer {
    buf: String,
    collapsed_pastes: Vec<CollapsedPaste>,
    cursor: usize,
    preferred_col: Option<usize>,
    anchor: Option<usize>,
    view_top: Cell<usize>,
    painted: Cell<Option<usize>>,
    /// Estilo de actividad que se pinta en el prompt mientras el agente trabaja.
    /// Se fija al empezar cada petición, así que cambia de una a otra.
    activity_style: &'static str,
    /// Si el agente está trabajando: el prompt se convierte en el indicador braille.
    working: Cell<bool>,
    /// Fotograma con el que animar el indicador; lo fija el render en cada pasada.
    activity_frame: Cell<u64>,
}

impl Default for Composer {
    fn default() -> Self {
        Self {
            buf: String::new(),
            collapsed_pastes: Vec::new(),
            cursor: 0,
            preferred_col: None,
            anchor: None,
            view_top: Cell::new(0),
            painted: Cell::new(None),
            activity_style: super::super::widgets::activity::ACTIVITY_STYLES[0],
            working: Cell::new(false),
            activity_frame: Cell::new(0),
        }
    }
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }
}
