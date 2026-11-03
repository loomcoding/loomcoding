//! Permisos y preguntas pendientes; mensajes de usuario y asistente.

use super::*;

impl App {
    /// Encola una petición; si no hay diálogo abierto, la muestra ya.
    pub fn enqueue_permission(&mut self, req: PermissionRequest, reply: PermissionReply) {
        self.tools_open = false;
        self.queue_open = false;
        self.queue_editing = false;
        self.help_open = false;
        self.agents_open = false;
        self.a2a_open = false;
        self.settings_open = false;
        self.settings_editing = false;
        self.permission_queue.push_back((req, reply));
        self.open_next_permission();
    }

    /// Responde al diálogo abierto y pasa al siguiente de la cola.
    pub fn resolve_permission(&mut self, decision: Decision) {
        if let Some(p) = self.permission.take() {
            let _ = p.reply.send(decision);
        }
        self.open_next_permission();
    }

    pub(super) fn open_next_permission(&mut self) {
        if self.permission.is_some() {
            return;
        }
        while let Some((req, reply)) = self.permission_queue.pop_front() {
            if reply.is_closed() {
                continue;
            }
            self.permission = Some(PendingPermission {
                dialog: dialog_for(&req),
                reply,
            });
            self.fx_pending = true;
            break;
        }
    }

    /// Encola un lote de preguntas; si no hay diálogo abierto, lo muestra ya.
    /// Sin preguntas responde `None` de inmediato: el diálogo nunca se abre vacío.
    pub fn enqueue_questions(&mut self, questions: Vec<Question>, reply: QuestionReply) {
        if questions.is_empty() {
            let _ = reply.send(None);
            return;
        }
        self.tools_open = false;
        self.queue_open = false;
        self.queue_editing = false;
        self.help_open = false;
        self.agents_open = false;
        self.a2a_open = false;
        self.settings_open = false;
        self.settings_editing = false;
        self.question_queue.push_back((questions, reply));
        self.open_next_question();
    }

    /// Procesa una tecla en el diálogo de preguntas abierto.
    pub fn question_key(&mut self, key: batten::state::QuestionKey) {
        use batten::state::QuestionOutcome;
        let Some(pending) = self.question.as_mut() else {
            return;
        };
        match pending.dialog.handle(key) {
            QuestionOutcome::Pending => {}
            QuestionOutcome::Done(selected) => {
                let answers = pending
                    .questions
                    .iter()
                    .zip(selected)
                    .map(|(question, selected)| Answer {
                        question: question.question.clone(),
                        selected,
                    })
                    .collect();
                self.finish_question(Some(answers));
            }
            QuestionOutcome::Cancelled => self.finish_question(None),
        }
    }

    /// Cierra todas las preguntas abiertas o encoladas respondiendo `None`.
    pub fn cancel_question(&mut self) {
        if let Some(pending) = self.question.take() {
            let _ = pending.reply.send(None);
        }
        for (_, reply) in self.question_queue.drain(..) {
            let _ = reply.send(None);
        }
    }

    /// Responde al diálogo abierto con `answers` y pasa al siguiente de la cola.
    pub(super) fn finish_question(&mut self, answers: Option<Vec<Answer>>) {
        if let Some(pending) = self.question.take() {
            let _ = pending.reply.send(answers);
        }
        self.open_next_question();
    }

    pub(super) fn open_next_question(&mut self) {
        if self.question.is_some() {
            return;
        }
        while let Some((questions, reply)) = self.question_queue.pop_front() {
            if reply.is_closed() {
                continue;
            }
            let specs = questions.iter().map(spec_for).collect();
            self.question = Some(PendingQuestion {
                dialog: batten::state::QuestionDialog::new(specs),
                questions,
                reply,
            });
            break;
        }
    }

    /// Mensaje del usuario: entrada nueva; la siguiente respuesta empieza otra.
    pub fn push_user(&mut self, text: &str) {
        use batten::state::{Format, LineKind};
        self.transcript
            .push_entry(LineKind::User, Format::Plain, text);
        self.assistant = None;
    }

    /// Delta de la respuesta: se añade a la entrada en curso (markdown).
    pub fn push_assistant(&mut self, delta: &str) {
        use batten::state::{Format, LineKind};
        match self.assistant {
            Some(id) => self.transcript.append(id, delta),
            None => {
                // Sigue llegando texto: se ve entera hasta que acabe el turno.
                let id =
                    self.transcript
                        .push_streaming(LineKind::Assistant, Format::Markdown, delta);
                self.assistant = Some(id);
            }
        }
    }
}
