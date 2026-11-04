//! Fases, herramientas y todos del transcript.

use super::*;

impl App {
    pub(super) fn set_phase(&mut self, phase: &str) {
        if self.phase != phase {
            self.finish_phase();
            self.phase = phase.into();
            self.phase_elapsed = Duration::ZERO;
            self.phase_started_at = Some(Instant::now());
        }
    }

    pub(super) fn finish_phase(&mut self) {
        if let Some(started_at) = self.phase_started_at.take() {
            self.phase_elapsed = started_at.elapsed();
            self.record_phase(self.phase_elapsed);
        }
    }

    pub(super) fn finish_phase_as(&mut self, phase: &str) {
        let elapsed = self
            .phase_started_at
            .map_or(self.phase_elapsed, |started_at| started_at.elapsed());
        if self.phase_started_at.take().is_some() {
            self.record_phase(elapsed);
        }
        self.phase = phase.into();
        self.phase_elapsed = elapsed;
    }

    pub(super) fn record_phase(&mut self, elapsed: Duration) {
        let name = match self.phase.as_str() {
            "waiting for provider" => "Provider",
            "thinking" => "Thinking",
            "running tools" => "Tools",
            _ => return,
        };
        if let Some((_, total)) = self.phase_durations.iter_mut().find(|(key, _)| key == name) {
            *total += elapsed;
        } else {
            self.phase_durations.push((name.into(), elapsed));
        }
    }

    pub fn phase_summary(&self) -> String {
        self.phase_durations
            .iter()
            .map(|(name, duration)| format!("{name} {:.1}s", duration.as_secs_f64()))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn phase_duration(&self) -> Duration {
        self.phase_started_at
            .map_or(self.phase_elapsed, |started_at| started_at.elapsed())
    }

    pub(super) fn tool_mut(
        &mut self,
        call_id: &str,
        name: &str,
    ) -> &mut loomcoding_cloth::state::ToolItem {
        if let Some(index) = self.tools.iter().position(|tool| tool.call_id == call_id) {
            return self.tools.get_mut(index).unwrap();
        }
        if self.tools.len() == 8 {
            self.tools.pop_front();
            if self.tool_selected == 0 {
                self.tool_scroll = (0, 0);
            }
            self.tool_selected = self.tool_selected.saturating_sub(1);
        }
        self.tools
            .push_back(loomcoding_cloth::state::ToolItem::new(call_id, name));
        self.tools.back_mut().unwrap()
    }

    /// Visibilidad de las herramientas en el transcript (look `Inline`).
    pub(super) fn tool_inline(&self) -> bool {
        self.theme.look.tool == loomcoding_cloth::widgets::ToolStyle::Inline
    }

    /// Líneas que una herramienta o un elemento de actividad deja en el transcript. Con el
    /// look `Inline`, todas; con el compacto, solo la cabecera. El transcript es el diario de
    /// toda iteración: ningún look puede dejar una llamada sin rastro.
    pub(super) fn shown_lines(
        &self,
        lines: &[loomcoding_cloth::state::Line],
    ) -> Vec<loomcoding_cloth::state::Line> {
        if self.tool_inline() {
            lines.to_vec()
        } else {
            lines.iter().take(1).cloned().collect()
        }
    }

    /// Hijo al que apunta una llamada del padre: `job_output` o `job_kill` sobre el job de un
    /// hijo de la sesión abierta, o `subagent_control` sobre el id de un hijo. Cualquier otra
    /// llamada, o un id ajeno al árbol, no tiene hijo.
    pub(super) fn node_target(&self, tool: &loomcoding_cloth::state::ToolItem) -> Option<i64> {
        match tool.name.as_str() {
            "job_output" | "job_kill" => {
                let job = tool.facts.primary.as_deref()?;
                self.subagents
                    .values()
                    .find(|subagent| subagent.job_id == job)
                    .map(|subagent| subagent.child.0)
            }
            "subagent_control" => {
                let child = tool.facts.secondary.as_deref()?.parse::<i64>().ok()?;
                self.subagents.contains_key(&child).then_some(child)
            }
            _ => None,
        }
    }

    /// Líneas visibles de una herramienta en el transcript. Ninguna si es la lista de tareas o
    /// si su llamada cuelga de un nodo de subagente; si no, las que muestre el look actual.
    pub(super) fn tool_shown(
        &self,
        call_id: &str,
        lines: &[loomcoding_cloth::state::Line],
    ) -> Vec<loomcoding_cloth::state::Line> {
        if self.todo_tool_calls.contains(call_id) || self.node_calls.contains_key(call_id) {
            Vec::new()
        } else {
            self.shown_lines(lines)
        }
    }

    /// Vuelve a sincronizar las herramientas del padre. Sirve para reclasificar las llamadas que
    /// llegaron antes que su hijo; las demás no cambian, porque `sync_tool` las reconoce por caché.
    pub(super) fn resync_tools(&mut self) {
        let ids: Vec<String> = self.tools.iter().map(|tool| tool.call_id.clone()).collect();
        for id in ids {
            self.sync_tool(&id);
        }
    }

    /// Sincroniza las líneas de una herramienta con su entrada del transcript.
    /// Solo reemplaza la entrada si sus líneas cambiaron: no se llama desde el
    /// camino de los deltas, que se ejecuta por cada token. Una llamada que apunta a un
    /// hijo se anota una sola vez en su nodo y no deja línea propia.
    pub fn sync_tool(&mut self, call_id: &str) {
        let Some(item) = self.tools.iter().find(|tool| tool.call_id == call_id) else {
            return;
        };
        let lines = loomcoding_cloth::state::tool_lines_with(item, &item.facts);
        let todo = item.name == TODO_TOOL;
        let enlace = self.node_target(item);
        let nombre = item.name.clone();
        let recien_anotada = match enlace {
            Some(hijo) if !self.node_calls.contains_key(call_id) => {
                self.node_calls.insert(call_id.to_string(), hijo);
                self.node_actions.entry(hijo).or_default().push(nombre);
                self.refresh_child_node(hijo);
                true
            }
            _ => false,
        };
        // La lista de tareas tiene su propia entrada: la llamada que la escribe no se repite.
        let shown = if todo {
            Vec::new()
        } else {
            self.tool_shown(call_id, &lines)
        };
        match self.tool_entries.get_mut(call_id) {
            Some((id, cache)) => {
                // Una llamada recién anotada debe quitar su línea aunque sus líneas no cambien.
                if *cache == lines && !recien_anotada {
                    return;
                }
                *cache = lines;
                self.transcript.replace(*id, shown);
            }
            // La entrada se crea siempre, en su sitio cronológico. Si deja alguna línea visible,
            // corta el texto del asistente: lo que diga después va en una entrada nueva, debajo.
            None => {
                let visible = !shown.is_empty();
                let id = self.transcript.push_lines(shown);
                self.tool_entries.insert(call_id.to_string(), (id, lines));
                if todo {
                    self.todo_tool_calls.insert(call_id.to_string());
                }
                if visible {
                    self.assistant = None;
                }
            }
        }
    }

    /// Aplica la visibilidad del look actual a todas las herramientas, a los
    /// elementos de actividad y a la lista de tareas.
    pub fn sync_all_tools(&mut self) {
        let inline = self.tool_inline();
        for (call_id, (id, lines)) in &self.tool_entries {
            let shown = self.tool_shown(call_id, lines);
            self.transcript.replace(*id, shown);
        }
        for entry in &self.activity_entries {
            let shown = self.shown_activity_lines(entry.kind, entry.from_notice, &entry.lines);
            self.transcript.replace(entry.id, shown);
        }
        if let Some((id, lines)) = &self.todo_entry {
            let shown = if inline { lines.clone() } else { Vec::new() };
            self.transcript.replace(*id, shown);
        }
        self.resync_tools();
    }

    /// Sincroniza la lista de tareas con su entrada del transcript. Como en
    /// `sync_tool`, la entrada se crea la primera vez (aunque la lista esté
    /// vacía) y después solo se reemplaza si cambia la caché. No se llama desde
    /// el camino de los deltas.
    pub(super) fn sync_todos(&mut self) {
        use loomcoding_cloth::state::ToolStatus;
        use loomcoding_core::todo::TodoStatus;
        let items: Vec<(&str, ToolStatus)> = self
            .todos
            .iter()
            .map(|todo| {
                let status = match todo.status {
                    TodoStatus::Pending => ToolStatus::Pending,
                    TodoStatus::InProgress => ToolStatus::Running,
                    TodoStatus::Completed => ToolStatus::Success,
                };
                (todo.content.as_str(), status)
            })
            .collect();
        let lines = loomcoding_cloth::state::todo_lines(&items);
        let inline = self.tool_inline();
        match self.todo_entry.as_mut() {
            Some((id, cache)) => {
                if *cache == lines {
                    return;
                }
                let shown = if inline { lines.clone() } else { Vec::new() };
                *cache = lines;
                self.transcript.replace(*id, shown);
            }
            None => {
                let shown = if inline { lines.clone() } else { Vec::new() };
                let id = self.transcript.push_lines(shown);
                self.todo_entry = Some((id, lines));
                if inline {
                    self.assistant = None;
                }
            }
        }
    }
}
