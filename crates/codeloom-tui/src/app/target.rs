//! Destinatario del composer: la sesión abierta o un hijo directo, y el despacho de sus mensajes.

use super::*;

/// Nombre corto del estado A2A (`working`, `input-required`), el mismo que ve la persona usuaria.
fn state_label(state: TaskState) -> String {
    state
        .proto_name()
        .trim_start_matches("TASK_STATE_")
        .to_lowercase()
        .replace('_', "-")
}

impl App {
    /// Hijos directos de la sesión abierta, por orden de identificador.
    fn direct_children(&self) -> Vec<&codeloom_core::session::SubagentDescriptor> {
        let mut children: Vec<_> = self
            .subagents
            .values()
            .filter(|child| Some(child.parent) == self.session_id)
            .collect();
        children.sort_by_key(|child| child.child.0);
        children
    }

    fn child_state(&self, child: SessionId) -> Option<TaskState> {
        self.subagents.get(&child.0).map(|c| c.status.state)
    }

    /// Fija como destinatario a un hijo directo. Devuelve `false` y avisa si no lo es.
    pub fn focus_child(&mut self, child: SessionId) -> bool {
        if self.direct_children().iter().any(|c| c.child == child) {
            self.composer_target = ComposerTarget::Child(child);
            self.slash_feedback = None;
            true
        } else {
            self.slash_feedback = Some(format!("no direct subagent #{}", child.0));
            false
        }
    }

    /// Alterna el destinatario: sesión abierta → cada hijo vivo → sesión abierta.
    pub fn cycle_composer_target(&mut self) {
        let live: Vec<SessionId> = self
            .direct_children()
            .into_iter()
            .filter(|c| !c.status.state.is_terminal())
            .map(|c| c.child)
            .collect();
        let next = match self.composer_target {
            ComposerTarget::Open => live.first().copied(),
            ComposerTarget::Child(current) => live
                .iter()
                .position(|&c| c == current)
                .and_then(|i| live.get(i + 1).copied()),
        };
        match next {
            Some(child) => self.composer_target = ComposerTarget::Child(child),
            None if live.is_empty() => {
                self.composer_target = ComposerTarget::Open;
                self.slash_feedback = Some("no active subagents".into());
            }
            None => self.composer_target = ComposerTarget::Open,
        }
    }

    /// Texto que nombra el destinatario cuando no es la sesión abierta.
    pub fn composer_target_label(&self) -> Option<String> {
        let ComposerTarget::Child(child) = self.composer_target else {
            return None;
        };
        let descriptor = self.subagents.get(&child.0)?;
        Some(format!(
            "→ {} #{} ({})",
            descriptor.label,
            child.0,
            state_label(descriptor.status.state)
        ))
    }

    /// Mensajes en espera para un hijo.
    pub fn queued_for_child(&self, child: SessionId) -> usize {
        self.child_queue.get(&child).map_or(0, |q| q.len())
    }

    /// `/focus [id]`: analiza la orden al enviar. Sin id vuelve a la sesión abierta.
    /// Devuelve `true` si el texto era la orden (la consume o la deja con su error).
    pub fn try_focus_command(&mut self) -> bool {
        let text = self.composer.text().to_string();
        let Some(rest) = text.trim().strip_prefix("/focus") else {
            return false;
        };
        if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
            return false;
        }
        let argument = rest.trim().trim_start_matches('#');
        if argument.is_empty() {
            self.composer_target = ComposerTarget::Open;
            self.slash_feedback = None;
        } else {
            match argument.parse::<i64>() {
                Ok(id) => {
                    if !self.focus_child(SessionId(id)) {
                        return true;
                    }
                }
                Err(_) => {
                    self.slash_feedback =
                        Some(format!("/focus needs a subagent id, got `{argument}`"));
                    return true;
                }
            }
        }
        self.composer.submit();
        true
    }

    /// `/focus` elegido en el menú: deja la orden escrita y explica cómo se usa.
    pub fn begin_focus_command(&mut self) {
        self.composer.set_text("/focus ");
        self.slash_feedback =
            Some("/focus <subagent id> · Ctrl+G cycles the live subagents".into());
    }

    /// Despacha el borrador al hijo destinatario. `false`: el destino es la sesión abierta y
    /// no se hace nada; `true`: el composer ya se ocupó (envió, encoló o rechazó con aviso).
    pub fn submit_to_child(&mut self) -> bool {
        let ComposerTarget::Child(child) = self.composer_target else {
            return false;
        };
        let text = self.composer.text().to_string();
        if text.trim().is_empty() {
            return true;
        }
        let Some(state) = self.child_state(child) else {
            self.composer_target = ComposerTarget::Open;
            self.slash_feedback = Some(format!("subagent #{} is gone", child.0));
            return true;
        };
        if text.len() > MAX_CHILD_MESSAGE_BYTES {
            self.slash_feedback = Some(format!(
                "message too long for a subagent ({} of {} bytes)",
                text.len(),
                MAX_CHILD_MESSAGE_BYTES
            ));
            return true;
        }
        match child_dispatch(state) {
            ChildDispatch::Answer => {
                self.composer.submit();
                self.history.push(text.clone());
                self.answer_subagent_requested = Some((child, text));
            }
            ChildDispatch::Queue => {
                if self.queued_for_child(child) >= MAX_CHILD_QUEUE {
                    self.slash_feedback = Some(format!(
                        "subagent #{} already has {} messages waiting",
                        child.0, MAX_CHILD_QUEUE
                    ));
                } else {
                    self.composer.submit();
                    self.history.push(text.clone());
                    self.child_queue.entry(child).or_default().push_back(text);
                    self.slash_feedback = None;
                }
            }
            ChildDispatch::Reject => {
                self.slash_feedback = Some(match state {
                    TaskState::AuthRequired => {
                        format!("subagent #{} needs credentials, not text", child.0)
                    }
                    _ => format!(
                        "subagent #{} is {}: open it from the rail to continue there",
                        child.0,
                        state_label(state)
                    ),
                });
            }
        }
        true
    }

    /// Reacciona al cambio de estado de un hijo: entrega lo encolado cuando pide entrada y lo
    /// descarta, avisando, cuando termina sin haberlo pedido.
    pub(super) fn on_child_state_changed(&mut self, child: SessionId, state: TaskState) {
        match child_dispatch(state) {
            ChildDispatch::Answer => {
                if self.answer_subagent_requested.is_none() {
                    if let Some(text) = self
                        .child_queue
                        .get_mut(&child)
                        .and_then(|queue| queue.pop_front())
                    {
                        self.answer_subagent_requested = Some((child, text));
                    }
                }
            }
            ChildDispatch::Reject if state.is_terminal() => {
                if let Some(queue) = self.child_queue.remove(&child) {
                    if !queue.is_empty() {
                        self.slash_feedback = Some(format!(
                            "subagent #{} finished; {} queued message(s) dropped",
                            child.0,
                            queue.len()
                        ));
                    }
                }
            }
            _ => {}
        }
        if self.child_queue.get(&child).is_some_and(|q| q.is_empty()) {
            self.child_queue.remove(&child);
        }
    }

    /// El destinatario deja de valer si el hijo ya no es directo de la sesión abierta.
    pub(super) fn validate_composer_target(&mut self) {
        if let ComposerTarget::Child(child) = self.composer_target {
            if !self.direct_children().iter().any(|c| c.child == child) {
                self.composer_target = ComposerTarget::Open;
            }
        }
    }
}
