//! Registro de agentes: filtros, edición, tarjetas A2A y su vista.

use super::*;

impl App {
    pub fn set_provider_settings(&mut self, settings: ProviderSettings) {
        self.model = settings.model.clone();
        self.settings_baseline = Some(settings.clone());
        self.provider_settings = Some(settings);
    }

    /// Fija los niveles de esfuerzo declarados en la configuración (`[provider] efforts`).
    pub fn set_declared_efforts(&mut self, efforts: Vec<String>) {
        self.declared_efforts = efforts;
    }

    /// Sustituye el catálogo por otro recién llegado de la carga en segundo plano.
    pub fn set_catalog(&mut self, catalog: Arc<Catalog>) {
        self.catalog = catalog;
    }

    /// Entrega a la interfaz el registro de agentes que consulta `/agents`.
    /// Enlaza `/refresh-agents` con quien sabe descargar las tarjetas.
    pub fn set_refresh_agents_hook(&mut self, hook: Arc<dyn Fn() + Send + Sync>) {
        self.refresh_agents_hook = Some(hook);
    }

    /// Lanza el refresco de tarjetas remotas. Sin enlace, lo dice en lugar de callar.
    pub fn request_refresh_agents(&mut self) {
        self.slash_feedback = Some(match &self.refresh_agents_hook {
            Some(hook) => {
                hook();
                "Refreshing the remote agents' cards…".into()
            }
            None => "Remote agents are not enabled ([a2a] role)".into(),
        });
    }

    pub fn set_agent_registry(&mut self, registry: Arc<loomcoding_core::registry::AgentRegistry>) {
        self.agent_registry = Some(registry);
    }

    /// Establece el texto de búsqueda y vuelve a filtrar.
    pub fn set_agents_query(&mut self, query: String) {
        self.agents_query = query;
        self.refresh_agent_rows();
    }

    /// Establece el filtro por tag y vuelve a filtrar.
    pub fn set_agents_tag_filter(&mut self, tag: Option<Vec<String>>) {
        self.agents_tag_filter = tag;
        self.refresh_agent_rows();
    }

    /// Añade un tag al filtro multiselect.
    pub fn add_agent_tag_filter(&mut self, tag: String) {
        let tag = tag.trim().to_string();
        if tag.is_empty() {
            return;
        }
        let mut tags = self.agents_tag_filter.clone().unwrap_or_default();
        if !tags.contains(&tag) {
            tags.push(tag);
            self.agents_tag_filter = Some(tags);
            self.refresh_agent_rows();
        }
    }

    /// Quita un tag del filtro multiselect.
    pub fn remove_agent_tag_filter(&mut self, tag: &str) {
        if let Some(mut tags) = self.agents_tag_filter.clone() {
            tags.retain(|t| t != tag);
            self.agents_tag_filter = if tags.is_empty() { None } else { Some(tags) };
            self.refresh_agent_rows();
        }
    }

    /// Limpia ambos filtros y vuelve a filtrar.
    pub fn clear_agents_filters(&mut self) {
        self.agents_query.clear();
        self.agents_tag_filter = None;
        self.refresh_agent_rows();
    }

    /// Cicla el foco entre Lista → Query → Tag → Lista...
    pub fn cycle_agents_focus(&mut self, forward: bool) {
        self.agents_focus = match (self.agents_focus, forward) {
            (AgentsFocus::List, true) => AgentsFocus::Query,
            (AgentsFocus::Query, true) => AgentsFocus::Tag,
            (AgentsFocus::Tag, true) => AgentsFocus::List,
            (AgentsFocus::List, false) => AgentsFocus::Tag,
            (AgentsFocus::Query, false) => AgentsFocus::List,
            (AgentsFocus::Tag, false) => AgentsFocus::Query,
        };
        // Sincroniza agents_filter_edit para el modo edición
        self.agents_filter_edit = match self.agents_focus {
            AgentsFocus::Query => Some(AgentFilterEdit::Query),
            AgentsFocus::Tag => Some(AgentFilterEdit::Tag),
            AgentsFocus::List => None,
        };
    }

    pub fn begin_agent_edit(&mut self) {
        let Some(row) = self.agent_rows.get(self.agents_selected) else {
            return;
        };
        if !row.editable {
            self.agents_status = Some("Only local file agents can be edited".into());
            return;
        }
        self.agent_edit = Some(AgentEditorDraft {
            description: row.description.clone(),
            version: row.version.clone(),
            card_skills: card_skills_to_text(&row.card_skills),
            mcp: row.scope.mcp.join("\n"),
            local_skills: row.scope.skills.join("\n"),
            tools: row.scope.tools.join("\n"),
            field: 0,
            editing_value: false,
        });
        self.agents_status = None;
    }

    pub fn move_agent_edit_field(&mut self, delta: isize) {
        if let Some(edit) = &mut self.agent_edit {
            edit.field = edit.field.saturating_add_signed(delta).min(6);
        }
    }

    pub fn start_agent_edit_value(&mut self) {
        if self.agent_edit.as_ref().is_some_and(|edit| edit.field == 6) {
            self.save_agent_edit();
            return;
        }
        let Some(edit) = &mut self.agent_edit else {
            return;
        };
        self.agent_editor
            .set_text(&agent_edit_value(edit, edit.field));
        edit.editing_value = true;
    }

    pub fn finish_agent_edit_value(&mut self, save: bool) {
        let Some(edit) = &mut self.agent_edit else {
            return;
        };
        if save {
            set_agent_edit_value(edit, edit.field, self.agent_editor.text().to_string());
        }
        edit.editing_value = false;
    }

    pub fn cancel_agent_edit(&mut self) {
        self.agent_edit = None;
        self.agents_status = Some("Edit cancelled".into());
    }

    pub fn save_agent_edit(&mut self) {
        let (Some(edit), Some(row), Some(registry)) = (
            self.agent_edit.clone(),
            self.agent_rows.get(self.agents_selected).cloned(),
            self.agent_registry.clone(),
        ) else {
            return;
        };
        let (Some(path), Some(hash)) = (row.profile_path, row.content_hash) else {
            self.agents_status = Some("This profile has no editable source file".into());
            return;
        };
        let skills = match card_skills_from_text(&edit.card_skills) {
            Ok(skills) => skills,
            Err(error) => {
                self.agents_status = Some(error);
                return;
            }
        };
        let scope = loomcoding_core::registry::profile::AgentScope {
            mcp: scope_list(&edit.mcp),
            skills: scope_list(&edit.local_skills),
            tools: scope_list(&edit.tools),
        };
        let version = if edit.version.trim().is_empty() {
            None
        } else {
            Some(edit.version)
        };
        let profile = loomcoding_core::registry::profile::AgentProfileEdit {
            description: edit.description,
            version,
            card_skills: skills,
            scope,
        };
        match loomcoding_core::registry::profile::save_profile_edit(&path, &hash, &profile) {
            Ok(_) => {
                registry.reload();
                self.refresh_agent_rows();
                self.agents_status = Some(format!(
                    "Saved `{}`; project trust may need approval",
                    row.name
                ));
                self.agent_edit = None;
            }
            Err(error) => self.agents_status = Some(format!("Not saved: {error}")),
        }
    }

    pub fn a2a_agents(&self) -> Vec<A2aAgentView> {
        let mut agents = self
            .agent_registry
            .as_ref()
            .map(|registry| {
                let snapshot = registry.snapshot();
                snapshot
                    .entries()
                    .into_iter()
                    .map(|entry| A2aAgentView {
                        name: entry.name.clone(),
                        card: entry.card.clone(),
                        origin: match &entry.origin {
                            loomcoding_core::registry::AgentOrigin::BuiltIn => "built-in".into(),
                            loomcoding_core::registry::AgentOrigin::File { level, .. } => {
                                match level {
                                    loomcoding_core::registry::ProfileLevel::Global => {
                                        "global profile".into()
                                    }
                                    loomcoding_core::registry::ProfileLevel::Catalog => {
                                        "catalog profile".into()
                                    }
                                    loomcoding_core::registry::ProfileLevel::Project => {
                                        "project profile".into()
                                    }
                                }
                            }
                            loomcoding_core::registry::AgentOrigin::Remote { .. } => {
                                "remote".into()
                            }
                        },
                        approval: snapshot.approval(&entry.name),
                        internal: false,
                        enabled: matches!(
                            snapshot.approval(&entry.name),
                            Some(
                                loomcoding_core::registry::Approval::NotRequired
                                    | loomcoding_core::registry::Approval::Approved
                            )
                        ),
                        execution: Some(entry.execution.clone()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        agents.push(A2aAgentView {
            name: "supervisor".into(),
            card: loomcoding_core::card::AgentCard::local(
                "Loomcoding Runtime Supervisor",
                "Local runtime directory and deterministic supervision; optional LLM assistance.",
                env!("CARGO_PKG_VERSION"),
            ),
            origin: "Loomcoding runtime".into(),
            approval: None,
            internal: true,
            enabled: true,
            execution: None,
        });
        agents
    }

    pub fn a2a_approval(&self, name: &str) -> Option<loomcoding_core::registry::Approval> {
        self.agent_registry
            .as_ref()
            .and_then(|registry| registry.snapshot().approval(name))
    }

    pub fn a2a_focus_next(&mut self) {
        self.a2a_focus = (self.a2a_focus + 1) % 3;
        self.a2a_detail_scroll = 0;
    }

    pub fn a2a_move(&mut self, delta: isize) {
        self.a2a_detail_scroll = 0;
        match self.a2a_focus {
            0 => {
                self.a2a_agent_selected = self
                    .a2a_agent_selected
                    .saturating_add_signed(delta)
                    .min(self.a2a_agents().len().saturating_sub(1));
            }
            1 => {
                self.a2a_task_selected = self
                    .a2a_task_selected
                    .saturating_add_signed(delta)
                    .min(self.subagents.len().saturating_sub(1));
            }
            _ => {
                self.a2a_event_selected = self
                    .a2a_event_selected
                    .saturating_add_signed(delta)
                    .min(self.a2a_observations.len().saturating_sub(1));
            }
        }
    }

    pub(super) fn record_a2a_observation(&mut self, observation: A2aObservation) {
        if self.a2a_observations.len() == MAX_A2A_OBSERVATIONS {
            self.a2a_observations.pop_front();
        }
        self.a2a_observations.push_back(observation);
    }

    /// Reconstruye las filas desde la instantánea vigente, conservando la selección acotada.
    /// El estado de aprobación se consulta en vivo, así que aprobar se refleja sin recargar.
    pub fn refresh_agent_rows(&mut self) {
        let filas = match &self.agent_registry {
            Some(registro) => {
                let instantanea = registro.snapshot();
                instantanea
                    .entries()
                    .into_iter()
                    .map(|entrada| AgentRow::from_entry(entrada, &instantanea))
                    .collect()
            }
            None => Vec::new(),
        };
        let query = self.agents_query.to_lowercase();
        let tags = self.agents_tag_filter.as_deref();
        self.agent_rows = filas
            .into_iter()
            .filter(|fila| {
                let coincide_texto = query.is_empty()
                    || fila.name.to_lowercase().contains(&query)
                    || fila.description.to_lowercase().contains(&query);
                let coincide_tag = match tags {
                    Some(tags) if !tags.is_empty() => tags.iter().any(|tag| {
                        let tag = tag.to_lowercase();
                        fila.card_skills.iter().any(|skill| {
                            skill
                                .tags
                                .iter()
                                .any(|etiqueta| etiqueta.to_lowercase().contains(&tag))
                        })
                    }),
                    _ => true,
                };
                coincide_texto && coincide_tag
            })
            .collect();
        self.agents_selected = self
            .agents_selected
            .min(self.agent_rows.len().saturating_sub(1));
    }

    /// Abre el popup de `/agents` con la instantánea vigente del registro.
    pub fn show_agents(&mut self) {
        self.agents_selected = 0;
        self.agents_status = None;
        self.refresh_agent_rows();
        self.agents_open = true;
    }

    /// Mueve la selección del popup de agentes (`delta` negativo sube), sin dar la vuelta.
    pub fn agents_move(&mut self, delta: isize) {
        let tope = self.agent_rows.len().saturating_sub(1);
        self.agents_selected = self.agents_selected.saturating_add_signed(delta).min(tope);
    }

    /// Aprueba (`aprobar`) o revoca el agente seleccionado y anota el resultado.
    pub fn agents_decide(&mut self, aprobar: bool) {
        let (Some(registro), Some(fila)) = (
            self.agent_registry.clone(),
            self.agent_rows.get(self.agents_selected),
        ) else {
            return;
        };
        let nombre = fila.name.clone();
        let resultado = if aprobar {
            registro.approve(&nombre)
        } else {
            registro.revoke(&nombre)
        };
        self.agents_status = Some(match resultado {
            Ok(()) if aprobar => format!("Approved `{nombre}`"),
            Ok(()) => format!("Revoked `{nombre}`"),
            Err(error) => format!("Not changed: {error}"),
        });
        self.refresh_agent_rows();
    }
}

pub(super) fn scope_list(text: &str) -> Vec<String> {
    text.lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn card_skills_to_text(skills: &[loomcoding_core::card::AgentSkill]) -> String {
    skills
        .iter()
        .map(|skill| {
            [
                skill.id.clone(),
                skill.name.clone(),
                skill.description.clone(),
                skill.tags.join(","),
                skill.examples.join(","),
                skill.input_modes.join(","),
                skill.output_modes.join(","),
            ]
            .join(" | ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn card_skills_from_text(
    text: &str,
) -> Result<Vec<loomcoding_core::card::AgentSkill>, String> {
    let mut skills = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        // shortcut: el editor no escapa «|» ni comas dentro de un valor; usar el formulario
        // por skill si aparecen en esos campos.
        let fields = line.split('|').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 7 || fields[0].is_empty() || fields[1].is_empty() {
            return Err(format!("A2A skill, línea {}: usa id | nombre | descripción | tags | ejemplos | input modes | output modes", index + 1));
        }
        let list = |value: &str| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        };
        let mut skill = loomcoding_core::card::AgentSkill::new(
            fields[0],
            fields[1],
            fields[2],
            list(fields[3]),
        );
        skill.examples = list(fields[4]);
        skill.input_modes = list(fields[5]);
        skill.output_modes = list(fields[6]);
        skills.push(skill);
    }
    Ok(skills)
}

pub(super) fn agent_edit_value(edit: &AgentEditorDraft, field: usize) -> String {
    match field {
        0 => edit.description.clone(),
        1 => edit.version.clone(),
        2 => edit.card_skills.clone(),
        3 => edit.mcp.clone(),
        4 => edit.local_skills.clone(),
        5 => edit.tools.clone(),
        _ => String::new(),
    }
}

pub(super) fn set_agent_edit_value(edit: &mut AgentEditorDraft, field: usize, value: String) {
    match field {
        0 => edit.description = value,
        1 => edit.version = value,
        2 => edit.card_skills = value,
        3 => edit.mcp = value,
        4 => edit.local_skills = value,
        5 => edit.tools = value,
        _ => {}
    }
}
