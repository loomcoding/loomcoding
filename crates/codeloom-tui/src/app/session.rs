//! Lista de sesiones, carga y borrador por sesión, navegación padre/hijo.

use super::*;

impl App {
    /// Set the sessions list and update the session picker state.
    ///
    /// When the sessions list is empty, automatically starts creating a new session:
    /// - Sets session_picker_open = false (new session starts immediately)
    /// - Sets session_loading = true (shows loading spinner)
    /// - Sets session_choice = Some(New) (queues new session creation)
    ///
    /// When sessions list is non-empty, shows the session picker for resuming:
    /// - Sets session_picker_open = true (user can select a session)
    /// - Sets session_loading = false (picker is available immediately)
    /// - Leaves session_choice = None (waiting for user selection)
    pub fn set_sessions(&mut self, sessions: Vec<SessionInfo>) {
        let is_empty = sessions.is_empty();
        self.sessions = sessions;
        self.session_query.clear();
        self.session_searching = false;
        self.session_choice = None;

        if is_empty {
            // No existing sessions - create new one
            self.session_picker_open = false;
            self.session_loading = true;
            self.session_choice = Some(SessionChoice::New);
        } else {
            // Existing sessions available - show picker
            self.session_picker_open = true;
            self.session_loading = false;
        }
    }

    pub fn matching_sessions(&self) -> impl Iterator<Item = &SessionInfo> {
        let query = self.session_query.to_lowercase();
        self.sessions
            .iter()
            .filter(move |session| session.preview.to_lowercase().contains(&query))
    }

    pub fn update_sessions(&mut self, sessions: Vec<SessionInfo>) {
        let selected = self
            .matching_sessions()
            .nth(self.session_selected)
            .map(|session| session.id);
        let selected_new = self.session_selected == self.matching_sessions().count();
        self.sessions = sessions;
        let matches = self.matching_sessions().collect::<Vec<_>>();
        self.session_selected = if selected_new {
            matches.len()
        } else {
            selected
                .and_then(|id| matches.iter().position(|session| session.id == id))
                .unwrap_or_else(|| self.session_selected.min(matches.len()))
        };
        self.session_picker_open = true;
        self.session_loading = false;
    }

    pub fn restore_messages(&mut self, messages: Vec<Message>) {
        self.transcript = new_transcript();
        self.assistant = None;
        self.history = batten::state::History::new();
        self.tools.clear();
        self.lsp_invoked = false;
        self.tool_entries.clear();
        self.activity_scroll = 0;
        // Las anotaciones de nodo van con sus entradas: si sobrevivieran, una llamada
        // restaurada se contaría dos veces o no se anotaría.
        self.node_calls.clear();
        self.node_actions.clear();
        self.activity_entries.clear();
        self.todos.clear();
        self.todo_entry = None;
        self.todo_tool_calls.clear();
        self.tools_open = false;
        self.a2a_open = false;
        self.tool_selected = 0;
        self.tool_scroll = (0, 0);
        self.changes = batten::state::FileChanges::new();
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.turn_input_tokens = 0;
        self.turn_output_tokens = 0;
        self.context_tokens = 0;
        self.context_memory = 0;
        self.background_summary = BackgroundSummary::None;
        for message in messages {
            if let Some(usage) = message.usage.as_ref() {
                self.input_tokens = self
                    .input_tokens
                    .saturating_add(u64::from(usage.input_tokens));
                self.output_tokens = self
                    .output_tokens
                    .saturating_add(u64::from(usage.output_tokens));
                // Como en vivo, la última petición con entrada conocida fija el tamaño del contexto.
                if message.role == Role::Assistant && usage.input_tokens > 0 {
                    self.context_tokens = u64::from(usage.input_tokens);
                }
            }
            match message.role {
                Role::User => {
                    let text = message.text();
                    let authored_by_human = message.origin == MessageOrigin::Human;
                    // El último resumen reconocido fija la memoria del medidor.
                    if let Some((_, cuerpo)) = parse_summary_text(&text) {
                        self.context_memory = estimate_tokens(cuerpo.len());
                    }
                    if authored_by_human {
                        self.history.push(text.clone());
                        self.push_user(&text);
                    } else {
                        self.internal_user_message(&text);
                    }
                }
                Role::Assistant => {
                    for part in message.parts {
                        match part {
                            ContentPart::Text { text } => self.push_assistant(&text),
                            ContentPart::ToolCall { id, name, input } => {
                                if name == "lsp" {
                                    self.lsp_invoked = true;
                                }
                                if name == TODO_TOOL {
                                    if let Ok(items) = codeloom_core::todo::parse_todos(&input) {
                                        self.todos = items;
                                        self.sync_todos();
                                    }
                                }
                                let facts = tool_facts(&name, &input);
                                let tool = self.tool_mut(&id, &name);
                                tool.set_summary(input.to_string());
                                tool.facts = facts;
                                self.sync_tool(&id);
                            }
                            _ => {}
                        }
                    }
                }
                Role::Tool => {
                    for part in message.parts {
                        if let ContentPart::ToolResult {
                            call_id,
                            content,
                            is_error,
                        } = part
                        {
                            if let Some(tool) = self.tools.iter_mut().find(|t| t.call_id == call_id)
                            {
                                tool.set_result(is_error);
                                tool.set_output(content);
                            }
                            self.sync_tool(&call_id);
                        }
                    }
                }
                Role::System => {}
            }
        }
        self.assistant = None;
        // Lo restaurado está completo: sus bloques largos se pliegan.
        self.transcript.settle();
        for tool in &mut self.tools {
            if tool.is_spinning() {
                tool.cancel();
            }
        }
        self.sync_all_tools();
        self.viewport.follow_bottom();
        self.status = "ready".into();
        self.phase = "ready".into();
        self.phase_started_at = None;
        self.phase_elapsed = Duration::ZERO;
        self.phase_durations.clear();
        self.current_step = 0;
        self.max_steps = 0;
        self.session_loading = false;
    }

    /// Aparta el borrador de la sesión que se deja y pone el de `destino`
    /// (o uno vacío). El indicador de trabajo y su estilo son del agente, no
    /// del borrador, así que pasan al composer entrante.
    pub(super) fn swap_composer_draft(&mut self, destino: SessionId) {
        // Primera carga: lo ya escrito es de la sesión que se abre, no de otra.
        let Some(previa) = self.session_id else {
            return;
        };
        let entrante = self
            .composer_drafts
            .remove(&destino)
            .unwrap_or_else(|| ComposerDraft {
                composer: batten::state::Composer::new(),
                images: Vec::new(),
            });
        let working = self.composer.is_working();
        let style = self.composer.activity_style();
        let saliente = ComposerDraft {
            composer: std::mem::replace(&mut self.composer, entrante.composer),
            images: std::mem::replace(&mut self.draft_images, entrante.images),
        };
        self.composer.set_working(working);
        self.composer.set_activity_style(style);
        if !saliente.composer.is_empty() || !saliente.images.is_empty() {
            self.composer_drafts.insert(previa, saliente);
        }
    }

    pub fn load_session(&mut self, session: SessionId, messages: Vec<Message>) {
        // Un aviso de la sesión anterior no debe despertar a la nueva.
        if self.session_id != Some(session) {
            self.wake_pending = false;
            self.wake_causes.clear();
            self.swap_composer_draft(session);
            // El destinatario es un hijo de la sesión que se deja, no de la que se abre.
            self.composer_target = ComposerTarget::Open;
        }
        self.session_id = Some(session);
        // Una navegación con `open_child` o `go_parent` conserva la pila de padres;
        // cualquier otra carga (p. ej. el selector de sesiones) la vacía.
        let navegacion = std::mem::take(&mut self.nav_keep_stack);
        if !navegacion {
            self.parent_stack.clear();
            self.files_by_session.clear();
        }
        self.rail_selected = None;
        self.subagents.clear();
        self.agent_stats.clear();
        self.skill_statuses_by_child.clear();
        self.child_logs.clear();
        self.streaming_child_text.clear();
        self.expanded_nodes.clear();
        self.node_actions.clear();
        self.node_calls.clear();
        // Los apuntes son de la sesión anterior; la carga posterior repone los de esta.
        self.usage_records.clear();
        self.active_usage.clear();
        self.context_breakdown = None;
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.turn_input_tokens = 0;
        self.turn_output_tokens = 0;
        self.session_selected = 0;
        self.session_query.clear();
        self.session_searching = false;
        self.session_choice = None;
        self.session_loading = false;
        self.session_picker_open = false;
        self.session_picker_return_to_chat = false;
        // Una selección del transcript pertenece a la sesión anterior.
        self.text_sel = None;
        self.text_drag = false;
        // El bloque activo pertenece también a la sesión anterior.
        self.block = None;
        self.restore_messages(messages);
        // `restore_messages` vacía el panel de cambios; se repone con los ficheros de esta sesión.
        if let Some(files) = self.files_by_session.get(&session.0) {
            for (path, &(added, removed)) in files {
                self.changes.upsert(batten::state::FileChange {
                    path: path.clone(),
                    added,
                    removed,
                });
            }
        }
    }

    pub fn load_subagents(
        &mut self,
        session: SessionId,
        subagents: Vec<codeloom_core::session::SubagentDescriptor>,
    ) {
        if self.session_id == Some(session) {
            for child in subagents
                .iter()
                .filter(|child| !child.status.state.is_terminal())
            {
                self.expanded_nodes.insert(child.child.0);
            }
            self.subagents = subagents
                .into_iter()
                .map(|subagent| (subagent.child.0, subagent))
                .collect();
            // Las llamadas restauradas antes que los hijos se reclasifican ahora que constan.
            self.resync_tools();
            self.validate_composer_target();
            // Un hijo que ya no existe no puede seguir marcado en el rail.
            if self
                .rail_selected
                .is_some_and(|selected| !self.subagents.contains_key(&selected))
            {
                self.rail_selected = None;
            }
        }
    }

    /// Mueve la selección del rail entre los hijos de la sesión abierta, sin dar la
    /// vuelta. Sin hijos la quita; sin selección (o con una que ya no existe) elige el primero.
    pub fn rail_move(&mut self, delta: isize) {
        let children: Vec<i64> = self
            .subagents
            .values()
            .filter(|s| Some(s.parent) == self.session_id)
            .map(|s| s.child.0)
            .collect();
        let Some(&first) = children.first() else {
            self.rail_selected = None;
            return;
        };
        let current = self
            .rail_selected
            .and_then(|selected| children.iter().position(|&child| child == selected));
        self.rail_selected = Some(match current {
            None => first,
            Some(index) => {
                let last = children.len() - 1;
                children[index.saturating_add_signed(delta).min(last)]
            }
        });
    }

    /// Entra en el hijo `child` del rail: apila la sesión actual y pide cargar el hijo.
    /// No tiene efecto (y devuelve `false`) con un turno en curso, sin sesión abierta
    /// o si `child` no es hijo de la sesión abierta.
    pub fn open_child(&mut self, child: i64) -> bool {
        let Some(current) = self.session_id else {
            return false;
        };
        let directo = self
            .subagents
            .get(&child)
            .is_some_and(|s| Some(s.parent) == self.session_id);
        if self.turn_in_flight || !directo {
            return false;
        }
        self.parent_stack.push(current);
        self.nav_keep_stack = true;
        self.session_choice = Some(SessionChoice::Resume(SessionId(child)));
        true
    }

    /// Vuelve a la sesión desde la que se entró en el subagente actual. No tiene efecto
    /// (y devuelve `false`) con un turno en curso o con la pila de padres vacía.
    pub fn go_parent(&mut self) -> bool {
        if self.turn_in_flight {
            return false;
        }
        let Some(parent) = self.parent_stack.pop() else {
            return false;
        };
        self.nav_keep_stack = true;
        self.session_choice = Some(SessionChoice::Resume(parent));
        true
    }

    pub fn session_load_failed(&mut self) {
        self.session_loading = false;
        // Una navegación fallida no debe dejar la pila «encendida» para la siguiente carga.
        self.nav_keep_stack = false;
        self.session_picker_open = !self.sessions.is_empty();
        self.session_picker_return_to_chat = false;
    }
}
