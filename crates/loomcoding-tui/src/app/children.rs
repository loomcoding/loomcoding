//! Estado de los hijos: contadores, ficheros, nodos y eventos de hijo.

use super::*;

impl App {
    /// Recuento de subagentes de la sesión abierta por estado A2A exacto, en el orden de
    /// `TaskState::ALL`. Solo aparecen los estados con cuenta mayor que cero.
    pub fn subagent_state_counts(&self) -> Vec<(TaskState, usize)> {
        TaskState::ALL
            .into_iter()
            .filter_map(|estado| {
                let cuenta = self
                    .subagents
                    .values()
                    .filter(|s| Some(s.parent) == self.session_id && s.status.state == estado)
                    .count();
                (cuenta > 0).then_some((estado, cuenta))
            })
            .collect()
    }

    /// Resumen de los subagentes en curso. Sin ninguno, `0 subagents running` sin paréntesis
    /// (no hay nada que ocultar); con alguno, el desglose por estado A2A entre paréntesis.
    pub fn running_summary(&self) -> String {
        let total = self.active_agents();
        if total == 0 {
            "0 subagents running".to_owned()
        } else {
            format!("{total} subagents running ({})", self.running_breakdown())
        }
    }

    /// Desglose por estado A2A de los subagentes en curso, p. ej. `working 2, input-required 1`.
    /// Vacío si no hay ninguno en curso.
    pub fn running_breakdown(&self) -> String {
        let activos: Vec<(TaskState, usize)> = self
            .subagent_state_counts()
            .into_iter()
            .filter(|(estado, _)| !estado.is_terminal())
            .collect();
        formato_recuento(&activos)
    }

    /// Sufijo de los subagentes terminados, ` (subagents: completed 2, failed 1)`; vacío si
    /// no hay ninguno.
    pub fn finished_summary(&self) -> String {
        let terminados: Vec<(TaskState, usize)> = self
            .subagent_state_counts()
            .into_iter()
            .filter(|(estado, _)| estado.is_terminal())
            .collect();
        if terminados.is_empty() {
            String::new()
        } else {
            format!(" (subagents: {})", formato_recuento(&terminados))
        }
    }

    /// Número de subagentes de la sesión abierta que siguen en marcha, es decir, los que
    /// no están en un estado terminal de A2A.
    pub fn active_agents(&self) -> usize {
        self.subagents
            .values()
            .filter(|subagent| {
                Some(subagent.parent) == self.session_id && !subagent.status.state.is_terminal()
            })
            .count()
    }

    /// Número de ficheros distintos que ha editado `child` en la sesión abierta.
    pub fn child_file_count(&self, child: SessionId) -> usize {
        self.files_by_session.get(&child.0).map_or(0, BTreeMap::len)
    }

    /// Anota una edición en `files_by_session` para `session`. La última edición de una
    /// ruta sustituye a la anterior, igual que `changes.upsert`.
    pub(super) fn record_file(&mut self, session: i64, path: &str, added: usize, removed: usize) {
        self.files_by_session
            .entry(session)
            .or_default()
            .insert(path.to_string(), (added, removed));
    }

    /// Actualiza las métricas y el registro de un hijo de la sesión abierta. Solo toca
    /// `agent_stats` y `child_logs`: el turno, el transcript y los contadores del padre
    /// quedan intactos.
    /// Vuelve a dibujar el nodo del hijo, si existe, tras un cambio en su registro o en sus
    /// métricas. `sync_activity` no reemplaza la entrada si sus líneas no cambian, así que
    /// el coste de un evento que no altera el nodo es mínimo.
    pub(super) fn refresh_child_node(&mut self, child: i64) {
        let Some(subagent) = self.subagents.get(&child) else {
            return;
        };
        let lines = self.subagent_node_lines(subagent);
        let label = subagent.label.clone();
        self.sync_activity(&node_key(child), "subagent", &label, lines, false);
    }

    /// Despliega (`true`) o pliega (`false`) el nodo del subagente que ocupa el bloque activo.
    /// Devuelve `false`, sin tocar nada, si el bloque activo no es un nodo de subagente.
    pub fn set_node_expanded(&mut self, expanded: bool) -> bool {
        let Some(hijo) = self.active_node_child() else {
            return false;
        };
        if expanded {
            self.expanded_nodes.insert(hijo);
        } else {
            self.expanded_nodes.remove(&hijo);
        }
        self.refresh_child_node(hijo);
        true
    }

    /// Alterna la expansión del nodo de subagente bajo el bloque activo.
    pub fn toggle_node_expanded(&mut self) -> bool {
        let Some(hijo) = self.active_node_child() else {
            return false;
        };
        self.set_node_expanded(!self.expanded_nodes.contains(&hijo))
    }

    pub(super) fn active_node_child(&self) -> Option<i64> {
        let bloque = self.block_index()?;
        // Los avisos no son nodos: solo cuentan las entradas de subagente en vivo.
        self.activity_entries
            .iter()
            .filter(|e| !e.from_notice && e.kind == "subagent" && e.id.index() == bloque)
            .find_map(|e| e.key.strip_prefix("subagent:")?.parse::<i64>().ok())
    }

    /// Si `parent` es la sesión abierta o un descendiente suyo ya conocido (hijo, nieto...).
    pub(super) fn in_open_tree(&self, parent: SessionId) -> bool {
        Some(parent) == self.session_id
            || profundidad_y_camino(&self.subagents, self.session_id, parent.0).is_some()
    }

    pub(super) fn apply_child_event(&mut self, child: SessionId, event: LiveEvent) {
        // Los eventos que no se reenvían no crean métricas.
        if !event.is_child_relevant() {
            return;
        }
        if let LiveEvent::SkillsLoaded { skills } = event {
            self.skill_statuses_by_child.insert(child.0, skills);
            return;
        }
        if matches!(&event, LiveEvent::UsageProgress { .. }) {
            self.apply_usage_progress(child, &event);
            return;
        }
        // Los mensajes del hijo alimentan su registro, pero no crean métricas: el registro
        // es independiente de `agent_stats`.
        if let LiveEvent::MessageAdded(message) = event {
            self.record_child_message(child.0, message);
            return;
        }
        if matches!(&event, LiveEvent::TurnStarted) {
            self.streaming_child_text.remove(&child.0);
        }
        if let LiveEvent::Delta { text } = event {
            if !text.is_empty() {
                let log = self.child_logs.entry(child.0).or_default();
                if !self.streaming_child_text.insert(child.0) {
                    if let Some(ChildItem::Text(current)) = log.items.back_mut() {
                        current.push_str(&text);
                        *current = recortar(current, MAX_CHILD_TEXT);
                    } else {
                        log.items
                            .push_back(ChildItem::Text(recortar(&text, MAX_CHILD_TEXT)));
                    }
                } else {
                    log.items
                        .push_back(ChildItem::Text(recortar(&text, MAX_CHILD_TEXT)));
                }
                while log.items.len() > MAX_CHILD_ITEMS {
                    log.items.pop_front();
                }
            }
            return;
        }
        // Un fichero editado por el hijo se anota en su sesión, sin tocar el panel del padre
        // ni crear métricas nuevas para el hijo.
        if let LiveEvent::FileDiff {
            path,
            added,
            removed,
        } = event
        {
            self.record_file(child.0, &path, added, removed);
            return;
        }
        // El fin de una herramienta del hijo cierra su elemento en el registro; las métricas
        // siguen su curso habitual más abajo.
        if let LiveEvent::ToolFinished { call_id, is_error } = &event {
            self.finish_child_tool(child.0, call_id, *is_error);
        }
        let now = Instant::now();
        let stats = self
            .agent_stats
            .entry(child.0)
            .or_insert_with(|| AgentStats {
                started_at: Some(now),
                ..AgentStats::default()
            });
        match event {
            LiveEvent::TurnStarted => {
                // Un turno nuevo reinicia el progreso, pero conserva los tokens acumulados.
                stats.step = 0;
                stats.max_steps = 0;
                stats.tool = None;
                stats.started_at = Some(now);
                stats.finished_at = None;
            }
            LiveEvent::StepStarted { step, max_steps } => {
                stats.step = step;
                stats.max_steps = max_steps;
            }
            LiveEvent::ToolStarted { name, .. } => stats.tool = Some(name),
            LiveEvent::ToolFinished { .. } => stats.tool = None,
            LiveEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            }
            | LiveEvent::CompactionUsage {
                input_tokens,
                output_tokens,
            } => {
                stats.input_tokens = stats.input_tokens.saturating_add(u64::from(input_tokens));
                stats.output_tokens = stats.output_tokens.saturating_add(u64::from(output_tokens));
            }
            LiveEvent::TurnEnded | LiveEvent::Error { .. } => {
                stats.finished_at = Some(now);
                stats.tool = None;
            }
            _ => {}
        }
    }

    /// Anota en el registro de `child` un mensaje completo. Del asistente se registran sus
    /// textos y sus llamadas a herramienta; los resultados de herramienta, de cualquier rol,
    /// cierran la llamada correspondiente. Los textos del usuario no se registran.
    pub(super) fn record_child_message(&mut self, child: i64, message: Message) {
        let asistente = message.role == Role::Assistant;
        let streaming = asistente && self.streaming_child_text.remove(&child);
        for part in message.parts {
            match part {
                ContentPart::Text { text } if asistente => {
                    let texto = text.trim();
                    if !texto.is_empty() {
                        let recortado = recortar(texto, MAX_CHILD_TEXT);
                        let log = self.child_logs.entry(child).or_default();
                        if streaming {
                            if let Some(ChildItem::Text(current)) = log.items.back_mut() {
                                *current = recortado;
                                continue;
                            }
                        }
                        log.items.push_back(ChildItem::Text(recortado));
                        while log.items.len() > MAX_CHILD_ITEMS {
                            log.items.pop_front();
                        }
                    }
                }
                ContentPart::ToolCall { id, name, input } if asistente => {
                    let summary = tool_facts(&name, &input)
                        .primary
                        .map(|primary| recortar(&primary, MAX_CHILD_SUMMARY));
                    self.push_child_item(
                        child,
                        ChildItem::Tool {
                            call_id: id,
                            name,
                            summary,
                            status: loomcoding_cloth::state::ToolStatus::Running,
                        },
                    );
                }
                ContentPart::ToolResult {
                    call_id, is_error, ..
                } => self.finish_child_tool(child, &call_id, is_error),
                _ => {}
            }
        }
    }

    /// Añade `item` al final del registro de `child` y descarta los más antiguos hasta
    /// respetar `MAX_CHILD_ITEMS`.
    pub(super) fn push_child_item(&mut self, child: i64, item: ChildItem) {
        let log = self.child_logs.entry(child).or_default();
        log.items.push_back(item);
        while log.items.len() > MAX_CHILD_ITEMS {
            log.items.pop_front();
        }
    }

    /// Fija el estado de la llamada `call_id` del registro de `child` según `is_error`. Si
    /// el registro no tiene esa llamada, no hace nada.
    pub(super) fn finish_child_tool(&mut self, child: i64, call_id: &str, is_error: bool) {
        let Some(log) = self.child_logs.get_mut(&child) else {
            return;
        };
        for item in &mut log.items {
            if let ChildItem::Tool {
                call_id: id,
                status,
                ..
            } = item
            {
                if id.as_str() == call_id {
                    *status = if is_error {
                        loomcoding_cloth::state::ToolStatus::Error
                    } else {
                        loomcoding_cloth::state::ToolStatus::Success
                    };
                    return;
                }
            }
        }
    }
}
