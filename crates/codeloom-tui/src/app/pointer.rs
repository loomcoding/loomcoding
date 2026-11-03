//! Foco, ratón, selección de texto y bloques plegables del transcript.

use super::*;

impl App {
    /// Alterna el foco entre composer y transcript. Desde un panel lateral, Tab
    /// devuelve el foco al composer.
    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Composer => Focus::Transcript,
            Focus::Transcript => Focus::Composer,
            _ => Focus::Composer,
        };
        if self.focus == Focus::Transcript {
            self.ensure_block();
        }
    }

    /// Con el chrome `Flat`, si el transcript no tiene bloque activo y hay entradas,
    /// activa el último. Así siempre hay un bloque marcado al entrar en él. Con
    /// Classic, o sin entradas, no hace nada.
    pub(super) fn ensure_block(&mut self) {
        if self.theme.look.chrome != batten::widgets::ChromeStyle::Flat || self.block.is_some() {
            return;
        }
        if let Some(last) = self.transcript.entry_count().checked_sub(1) {
            self.block = Some(last);
        }
    }

    /// Ventana bajo la celda (x, y) del último fotograma. El composer tiene
    /// prioridad sobre el transcript, cuya área lo contiene.
    pub fn pane_at(&self, x: u16, y: u16) -> Option<Focus> {
        if self
            .composer_area
            .is_some_and(|area| rect_contains(area, x, y))
        {
            return Some(Focus::Composer);
        }
        self.pane_rects
            .iter()
            .find(|(_, area)| rect_contains(*area, x, y))
            .map(|(pane, _)| *pane)
    }

    /// Copia la selección del composer al portapapeles (lo vuelca el bucle con
    /// OSC 52) y avisa del número de caracteres. Devuelve `false` sin selección.
    pub fn copy_composer_selection(&mut self) -> bool {
        let Some(text) = self.composer.selected_text_expanded() else {
            return false;
        };
        let chars = text.chars().count();
        self.clipboard = Some(text.to_owned());
        self.slash_feedback = Some(format!("Copied {chars} chars"));
        true
    }

    /// Pulsación del botón izquierdo. Dirige el foco a la ventana pulsada; en el
    /// composer coloca el cursor y queda listo para arrastrar. Fuera de cualquier
    /// ventana no hace nada, y en otra ventana anula el arrastre en curso.
    pub fn mouse_down(&mut self, x: u16, y: u16) {
        self.text_drag = false;
        let Some(pane) = self.pane_at(x, y) else {
            self.mouse_drag = None;
            return;
        };
        self.focus = pane;
        if pane == Focus::Transcript {
            self.mouse_drag = None;
            self.transcript_mouse_down(x, y);
            self.ensure_block();
            return;
        }
        let area = self.composer_area.filter(|_| pane == Focus::Composer);
        let Some(area) = area else {
            self.mouse_drag = None;
            return;
        };
        // El composer y el transcript no comparten selección.
        self.text_sel = None;
        match hit_index(&self.theme, &self.composer, composer_text_area(area), x, y) {
            Some(index) => {
                self.composer.set_cursor(index);
                self.mouse_drag = Some(index);
            }
            None => self.mouse_drag = None,
        }
    }

    /// Arrastre con el botón izquierdo: selecciona desde el ancla hasta la celda,
    /// acotada al área del composer. Sin arrastre en curso, no hace nada.
    pub fn mouse_drag_to(&mut self, x: u16, y: u16) {
        if self.text_drag {
            self.transcript_mouse_drag_to(x, y);
            return;
        }
        let (Some(anchor), Some(area)) = (self.mouse_drag, self.composer_area) else {
            return;
        };
        if area.is_empty() {
            return;
        }
        let text_area = composer_text_area(area);
        let x = x.clamp(text_area.x, text_area.right().saturating_sub(1));
        let y = y.clamp(area.y, area.bottom() - 1);
        if let Some(head) = hit_index(&self.theme, &self.composer, text_area, x, y) {
            self.composer.select_range(anchor, head);
        }
    }

    /// Soltar el botón. Solo si hubo un arrastre que empezó en el composer se
    /// copia la selección: un clic simple no copia nada.
    pub fn mouse_up(&mut self) {
        if self.text_drag {
            self.transcript_mouse_up();
            return;
        }
        if self.mouse_drag.take().is_some() {
            self.copy_composer_selection();
        }
    }

    /// Pulsación dentro del área de texto del transcript: enfoca el transcript y
    /// empieza una selección de texto en la celda pulsada. Fuera de esa área solo
    /// enfoca. Anula la selección de líneas y la del composer.
    pub fn transcript_mouse_down(&mut self, x: u16, y: u16) {
        self.text_sel = None;
        self.text_drag = false;
        self.selection = None;
        self.composer.clear_selection();
        let Some(area) = self
            .transcript_area
            .filter(|area| rect_contains(*area, x, y))
        else {
            return;
        };
        let Some(at) =
            batten::widgets::transcript::hit_cell(&self.layout, &self.viewport, area, x, y)
        else {
            return;
        };
        self.text_sel = Some(batten::state::TextSelection::new(at));
        self.text_sel_width = self.layout.width();
        self.text_drag = true;
        // El clic activa el bloque pulsado; la selección de texto no cambia.
        self.block = self.entry_at_row(at.row);
        // Pulsar el aviso de líneas ocultas despliega (o pliega) el bloque.
        let sobre_aviso = self
            .layout
            .line_at_row(at.row)
            .and_then(|(line, _)| self.transcript.line(line))
            .is_some_and(|line| line.kind == batten::state::LineKind::Fold);
        if sobre_aviso {
            self.toggle_block_fold();
        }
    }

    /// Entrada del transcript que ocupa la fila de documento `row`, si la hay.
    pub(super) fn entry_at_row(&self, row: usize) -> Option<usize> {
        let (line, _) = self.layout.line_at_row(row)?;
        self.transcript.locate(line).map(|(entry, _)| entry)
    }

    /// Bloque activo acotado a las entradas existentes: `None` sin transcript.
    pub fn block_index(&self) -> Option<usize> {
        let count = self.transcript.entry_count();
        self.block.filter(|_| count > 0).map(|b| b.min(count - 1))
    }

    /// Bloque anterior (`[`). Sin bloque activo, activa el último.
    pub fn block_prev(&mut self) {
        let Some(last) = self.transcript.entry_count().checked_sub(1) else {
            self.block = None;
            return;
        };
        let target = self
            .block_index()
            .map_or(last, |current| current.saturating_sub(1));
        self.focus_block(target, true);
    }

    /// Bloque siguiente (`]`). Sin bloque activo, activa el último.
    pub fn block_next(&mut self) {
        let Some(last) = self.transcript.entry_count().checked_sub(1) else {
            self.block = None;
            return;
        };
        let target = self
            .block_index()
            .map_or(last, |current| (current + 1).min(last));
        self.focus_block(target, false);
    }

    /// Activa la entrada `entry` y desplaza lo mínimo para verla entera; si no cabe,
    /// deja visible su comienzo. Al retroceder suelta el seguimiento del final.
    pub(super) fn focus_block(&mut self, entry: usize, backwards: bool) {
        self.block = Some(entry);
        if backwards {
            // `scroll_up(0)` solo suelta el seguimiento del final sin mover la vista.
            self.viewport.scroll_up(0);
        }
        let first = self.transcript.entry_start(entry);
        let start = self.layout.row_of(first);
        let height = self
            .layout
            .row_of(first + self.transcript.entry_lines(entry).len())
            .saturating_sub(start);
        let view = self
            .transcript_area
            .map_or(1, |area| usize::from(area.height).max(1));
        self.viewport
            .ensure_visible(start, start + height.min(view));
    }

    /// Texto plano del bloque activo: las líneas de la entrada unidas por `\n`.
    pub fn block_text(&self) -> Option<String> {
        let entry = self.block_index()?;
        // El texto completo, también el que un pliegue deja fuera de la vista.
        self.transcript.entry_text(entry)
    }

    /// Pliega o despliega el bloque activo si es largo. `false` si no hay nada que plegar.
    pub fn toggle_block_fold(&mut self) -> bool {
        match self.block_index() {
            Some(entry) => self.transcript.toggle_fold(entry),
            None => false,
        }
    }

    /// Copia el bloque activo al portapapeles y avisa del número de caracteres.
    /// Devuelve `false` si no hay bloque activo.
    pub fn copy_block(&mut self) -> bool {
        let Some(text) = self.block_text() else {
            return false;
        };
        let chars = text.chars().count();
        self.clipboard = Some(text);
        self.slash_feedback = Some(format!("Copied {chars} chars"));
        true
    }

    /// Arrastre sobre el transcript: mueve la cabeza de la selección. Si el puntero
    /// sale por arriba o por abajo del área, el transcript se desplaza una fila por
    /// evento hacia ese lado y la cabeza queda acotada a la fila visible.
    pub fn transcript_mouse_drag_to(&mut self, x: u16, y: u16) {
        if !self.text_drag {
            return;
        }
        let (Some(area), Some(mut sel)) = (
            self.transcript_area.filter(|area| !area.is_empty()),
            self.text_sel,
        ) else {
            return;
        };
        if y <= area.y {
            self.viewport.scroll_up(1);
        } else if y.saturating_add(1) >= area.bottom() {
            self.viewport.scroll_down(1);
        }
        if let Some(head) =
            batten::widgets::transcript::hit_cell(&self.layout, &self.viewport, area, x, y)
        {
            sel.set_head(head);
            self.text_sel = Some(sel);
        }
    }

    /// Soltar tras un arrastre sobre el transcript: copia si la selección no está
    /// vacía. Un clic simple (selección vacía) la descarta. La selección queda
    /// resaltada tras copiar.
    pub fn transcript_mouse_up(&mut self) {
        self.text_drag = false;
        if self.text_sel.is_some_and(|sel| sel.is_empty()) {
            self.text_sel = None;
            return;
        }
        self.copy_transcript_selection();
    }

    /// Copia la selección de texto del transcript al portapapeles y avisa del número
    /// de caracteres. Devuelve `false` si no hay selección no vacía.
    pub fn copy_transcript_selection(&mut self) -> bool {
        let Some(sel) = self.text_sel.filter(|sel| !sel.is_empty()) else {
            return false;
        };
        let text = batten::widgets::transcript::selected_text(
            &self.transcript,
            &self.layout,
            &self.theme,
            &sel,
        );
        let chars = text.chars().count();
        self.clipboard = Some(text);
        self.slash_feedback = Some(format!("Copied {chars} chars"));
        true
    }
}
