//! Estado del diff de la sesión dentro del menú izquierdo: qué fichero está elegido, cuál está
//! desplegado y el resultado de la última llamada a `git`.
//!
//! Cabe señalar que la llamada a `git` nunca se hace en el hilo de teclas ni bloquea el bucle:
//! `refresh_session_diff` lanza un hilo y guarda el receptor; `poll_session_diff` lo recoge sin
//! esperar.

use super::App;
use crate::session_diff::{fetch_session_diff, DiffOutcome};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Estado del bloque «Ficheros» del menú izquierdo. Vive solo en memoria de la interfaz.
#[derive(Default)]
pub struct SessionDiffView {
    /// Último resultado de `git`; `None` mientras no llega el primero.
    pub outcome: Option<DiffOutcome>,
    /// Índice del fichero resaltado en `App::changes`.
    pub selected: usize,
    /// Índice del fichero cuyo diff está desplegado, si hay. Solo uno a la vez.
    pub expanded: Option<usize>,
    /// Número de ficheros con que se pidió el último diff.
    pub fetched_for: usize,
    /// Suma de `(añadidas, quitadas)` con que se pidió el último diff.
    pub fetched_totals: (usize, usize),
    /// Petición a `git` en vuelo, si hay.
    pub pending: Option<Receiver<DiffOutcome>>,
}

impl App {
    /// Lanza un nuevo diff si la lista de ficheros cambió desde la última petición y no hay
    /// ninguna en vuelo. Nunca bloquea: la llamada a `git` corre en un hilo aparte.
    pub fn refresh_session_diff(&mut self) {
        self.clamp_session_diff();
        if self.changes.is_empty() {
            self.session_diff.outcome = None;
            self.session_diff.fetched_for = 0;
            self.session_diff.fetched_totals = (0, 0);
            return;
        }
        if self.session_diff.pending.is_some() {
            return;
        }
        let count = self.changes.len();
        let totals = self.changes.totals();
        if count == self.session_diff.fetched_for && totals == self.session_diff.fetched_totals {
            return;
        }
        let rutas: Vec<String> = self
            .changes
            .iter()
            .map(|change| change.path.clone())
            .collect();
        let cwd = PathBuf::from(&self.cwd);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            // Si nadie espera ya el resultado, el envío falla y no importa.
            let _ = tx.send(fetch_session_diff(&cwd, &rutas));
        });
        self.session_diff.pending = Some(rx);
        self.session_diff.fetched_for = count;
        self.session_diff.fetched_totals = totals;
    }

    /// Recoge el resultado de la petición en vuelo, si ya llegó. No espera.
    pub fn poll_session_diff(&mut self) {
        let resultado = match self.session_diff.pending.as_ref() {
            Some(rx) => rx.try_recv(),
            None => return,
        };
        match resultado {
            Ok(outcome) => {
                self.session_diff.outcome = Some(outcome);
                self.session_diff.pending = None;
            }
            Err(TryRecvError::Empty) => {}
            // El hilo terminó sin enviar nada: se olvida la petición para volver a pedirla.
            Err(TryRecvError::Disconnected) => {
                self.session_diff.pending = None;
                self.session_diff.fetched_for = 0;
                self.session_diff.fetched_totals = (0, 0);
            }
        }
    }

    /// Mueve la selección `delta` ficheros, acotada a la lista. No da la vuelta.
    pub fn diff_select(&mut self, delta: isize) {
        self.clamp_session_diff();
        let len = self.changes.len();
        if len == 0 {
            return;
        }
        let nuevo = self.session_diff.selected as isize + delta;
        self.session_diff.selected = nuevo.clamp(0, len as isize - 1) as usize;
    }

    /// Despliega el diff del fichero seleccionado, o lo pliega si ya estaba desplegado.
    pub fn diff_toggle(&mut self) {
        self.clamp_session_diff();
        if self.changes.is_empty() {
            return;
        }
        let seleccionado = self.session_diff.selected;
        self.session_diff.expanded = match self.session_diff.expanded {
            Some(abierto) if abierto == seleccionado => None,
            _ => Some(seleccionado),
        };
    }

    /// Mantiene `selected` y `expanded` dentro de `changes`, por si la lista se ha acortado.
    fn clamp_session_diff(&mut self) {
        let len = self.changes.len();
        let view = &mut self.session_diff;
        if len == 0 {
            view.selected = 0;
            view.expanded = None;
            return;
        }
        view.selected = view.selected.min(len - 1);
        if view.expanded.is_some_and(|indice| indice >= len) {
            view.expanded = None;
        }
    }
}
