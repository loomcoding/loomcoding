//! Buzón de preguntas de los subagentes (`input-required`).
//!
//! Cuando un hijo invoca `ask_user`, su petición queda aquí hasta que el padre
//! responde, la descarta el lanzador o el propio hijo deja de esperar. Es un
//! componente puro: no conoce la red ni la interfaz.

use crate::error::{CoreError, Result};
use crate::question::{Answer, Question};
use crate::session::SessionId;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::oneshot;

/// Petición de un hijo que espera respuesta de su padre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInput {
    pub parent: SessionId,
    pub child: SessionId,
    pub label: String,
    pub questions: Vec<Question>,
}

/// Petición viva en el buzón. La clave del mapa es el `i64` del hijo, porque
/// `SessionId` no implementa `Hash`.
struct Entry {
    /// Identifica esta petición concreta; evita que el guard de otra la borre.
    ticket: u64,
    parent: SessionId,
    label: String,
    questions: Vec<Question>,
    announced: bool,
    tx: oneshot::Sender<Option<Vec<Answer>>>,
}

impl Entry {
    fn view(&self, child: i64) -> PendingInput {
        PendingInput {
            parent: self.parent,
            child: SessionId(child),
            label: self.label.clone(),
            questions: self.questions.clone(),
        }
    }
}

#[derive(Default)]
struct Inner {
    entries: HashMap<i64, Entry>,
    next_ticket: u64,
}

/// Buzón de preguntas pendientes, indexado por hijo.
#[derive(Default)]
pub struct InputBroker {
    inner: Mutex<Inner>,
}

impl InputBroker {
    /// Registra una petición del hijo y espera la respuesta del padre.
    ///
    /// Devuelve `None` si la petición se descarta (`discard`, o sustitución por
    /// una pregunta nueva del mismo hijo). Si el futuro se cancela, la entrada
    /// se retira al soltarlo.
    pub async fn request(
        &self,
        parent: SessionId,
        child: SessionId,
        label: String,
        questions: Vec<Question>,
    ) -> Option<Vec<Answer>> {
        self.request_with(parent, child, label, questions, || {})
            .await
    }

    /// Como `request`, pero llama a `on_registered` justo después de registrar la
    /// petición y sin mantener el cerrojo. Así quien anuncia la petición (p. ej. por
    /// el bus) puede confiar en que ya es visible en `pending`.
    pub async fn request_with(
        &self,
        parent: SessionId,
        child: SessionId,
        label: String,
        questions: Vec<Question>,
        on_registered: impl FnOnce(),
    ) -> Option<Vec<Answer>> {
        let (tx, rx) = oneshot::channel();
        let (ticket, replaced) = {
            let mut inner = self.lock();
            inner.next_ticket += 1;
            let ticket = inner.next_ticket;
            let replaced = inner.entries.insert(
                child.0,
                Entry {
                    ticket,
                    parent,
                    label,
                    questions,
                    announced: false,
                    tx,
                },
            );
            (ticket, replaced)
        };
        on_registered();
        // La petición sustituida se avisa fuera del cerrojo.
        if let Some(previous) = replaced {
            let _ = previous.tx.send(None);
        }
        let _cleanup = Cleanup {
            broker: self,
            child: child.0,
            ticket,
        };
        rx.await.ok().flatten()
    }

    /// Peticiones pendientes de un padre, ordenadas por hijo.
    pub fn pending(&self, parent: SessionId) -> Vec<PendingInput> {
        let inner = self.lock();
        let mut out: Vec<PendingInput> = inner
            .entries
            .iter()
            .filter(|(_, entry)| entry.parent == parent)
            .map(|(child, entry)| entry.view(*child))
            .collect();
        out.sort_by_key(|input| input.child.0);
        out
    }

    /// Devuelve las peticiones de un padre que aún no se han anunciado y las
    /// marca como anunciadas: cada una sale una sola vez. No las responde.
    pub fn take_unannounced(&self, parent: SessionId) -> Vec<PendingInput> {
        let mut inner = self.lock();
        let mut out: Vec<PendingInput> = inner
            .entries
            .iter_mut()
            .filter(|(_, entry)| entry.parent == parent && !entry.announced)
            .map(|(child, entry)| {
                entry.announced = true;
                entry.view(*child)
            })
            .collect();
        out.sort_by_key(|input| input.child.0);
        out
    }

    /// Responde la petición pendiente del hijo: una respuesta por pregunta, en orden.
    ///
    /// Si no hay petición del padre para ese hijo, o el número de respuestas no
    /// coincide, devuelve error y la petición sigue esperando.
    pub fn answer(&self, parent: SessionId, child: SessionId, answers: Vec<String>) -> Result<()> {
        let (tx, built) = {
            let mut inner = self.lock();
            let entry = match inner.entries.get(&child.0) {
                Some(entry) if entry.parent == parent => entry,
                _ => {
                    return Err(CoreError::Tool(format!(
                        "subagent {} has no pending question",
                        child.0
                    )))
                }
            };
            if answers.len() != entry.questions.len() {
                return Err(CoreError::Tool(format!(
                    "subagent {} asked {} question(s) but {} answer(s) were given",
                    child.0,
                    entry.questions.len(),
                    answers.len()
                )));
            }
            let built: Vec<Answer> = entry
                .questions
                .iter()
                .zip(answers)
                .map(|(question, selected)| Answer {
                    question: question.question.clone(),
                    selected: vec![selected],
                })
                .collect();
            let removed = inner.entries.remove(&child.0).ok_or_else(|| {
                CoreError::Tool(format!("subagent {} has no pending question", child.0))
            })?;
            (removed.tx, built)
        };
        // Un receptor ya cancelado no es un error: su guard ya limpió la entrada.
        let _ = tx.send(Some(built));
        Ok(())
    }

    /// Descarta la petición del hijo sin responder; su `request` devuelve `None`.
    pub fn discard(&self, child: SessionId) {
        let removed = self.lock().entries.remove(&child.0);
        if let Some(entry) = removed {
            let _ = entry.tx.send(None);
        }
    }

    /// Bloqueo del buzón. Un cerrojo envenenado no debe tumbar al resto de hijos.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Retira la entrada de `request` si el futuro se suelta antes de tiempo. Solo
/// borra la entrada si sigue siendo la suya (mismo `ticket`).
struct Cleanup<'a> {
    broker: &'a InputBroker,
    child: i64,
    ticket: u64,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let mut inner = self.broker.lock();
        if inner
            .entries
            .get(&self.child)
            .is_some_and(|entry| entry.ticket == self.ticket)
        {
            inner.entries.remove(&self.child);
        }
    }
}

mod responder;
pub use responder::ChildResponder;

#[cfg(test)]
mod tests;
