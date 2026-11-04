//! Disposición por entrada: alturas, filas y reindexado incremental.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct EntryLayout {
    pub(super) rev: u64,
    pub(super) heights: Vec<usize>,
}

/// Caché de alturas (filas tras el wrap) por entrada y ancho. La guarda quien
/// usa el widget; `sync` solo re-mide las entradas cuya revisión cambió.
#[derive(Debug, Default, Clone)]
pub struct Layout {
    pub(super) width: u16,
    pub(super) entries: Vec<EntryLayout>,
    /// Por línea global: fila donde empieza y altura.
    pub(super) row_starts: Vec<usize>,
    pub(super) heights: Vec<usize>,
    pub(super) total_rows: usize,
    pub(super) measured: usize,
}

impl Layout {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pone la caché al día para `width`.
    pub fn sync(&mut self, transcript: &Transcript, width: u16, theme: &Theme) {
        if width != self.width {
            self.entries.clear();
            self.width = width;
        }
        self.entries.truncate(transcript.entry_count());
        let mut first_changed = None;
        for i in 0..transcript.entry_count() {
            let rev = transcript.entry_rev(i);
            if self.entries.get(i).is_some_and(|e| e.rev == rev) {
                continue;
            }
            let heights = transcript
                .entry_lines(i)
                .iter()
                .enumerate()
                .map(|(line, l)| measure(l, width, theme, line == 0))
                .collect();
            self.measured += 1;
            let layout = EntryLayout { rev, heights };
            if i < self.entries.len() {
                self.entries[i] = layout;
            } else {
                self.entries.push(layout);
            }
            first_changed.get_or_insert(i);
        }
        if let Some(entry) = first_changed {
            self.reindex_from(entry, transcript.entry_start(entry));
        }
    }

    fn reindex_from(&mut self, entry: usize, line: usize) {
        self.row_starts.truncate(line);
        self.heights.truncate(line);
        let mut acc = match line {
            0 => 0,
            _ => self.row_starts[line - 1] + self.heights[line - 1],
        };
        for e in &self.entries[entry..] {
            for &h in &e.heights {
                self.row_starts.push(acc);
                self.heights.push(h);
                acc += h;
            }
        }
        self.total_rows = acc;
    }

    pub fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// Entradas medidas desde la creación (para tests de rendimiento).
    pub fn measured(&self) -> usize {
        self.measured
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    /// Fila global donde empieza la línea `line`.
    pub fn row_of(&self, line: usize) -> usize {
        self.row_starts
            .get(line)
            .copied()
            .unwrap_or(self.total_rows)
    }

    /// Filas que ocupa la línea `line`.
    pub fn height_of(&self, line: usize) -> usize {
        self.heights.get(line).copied().unwrap_or(0)
    }

    /// `(línea, fila dentro de la línea)` que ocupa la fila `row`.
    pub fn line_at_row(&self, row: usize) -> Option<(usize, usize)> {
        if self.heights.is_empty() {
            return None;
        }
        let row = row.min(self.total_rows.saturating_sub(1));
        let line = self.row_starts.partition_point(|&s| s <= row) - 1;
        Some((line, row - self.row_starts[line]))
    }
}
