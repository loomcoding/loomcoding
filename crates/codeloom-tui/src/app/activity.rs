//! Actividad de jobs y subagentes, resúmenes de nodo y avisos.

use super::*;

impl App {
    pub(super) fn internal_user_message(&mut self, text: &str) {
        if self.tool_inline() && self.apply_notice(text) {
            return;
        }
        let Some(notice) = parse_notice(text) else {
            self.transcript.push_entry(
                batten::state::LineKind::System,
                batten::state::Format::Plain,
                format!("[Mensaje interno]\n{text}"),
            );
            self.assistant = None;
            return;
        };
        let shown = match notice {
            Notice::Compaction { covers, body } => {
                format!("{}\n{body}", compaction_label(covers))
            }
            Notice::Job { .. } | Notice::SubagentInput { .. } => text.to_string(),
            Notice::Subagent { label } => {
                format!("[Subagent {label} completed; open its chat to read the response]")
            }
        };
        self.transcript.push_entry(
            batten::state::LineKind::System,
            batten::state::Format::Plain,
            shown,
        );
        self.assistant = None;
    }

    /// Sincroniza un elemento de actividad con su entrada del transcript. Como
    /// `sync_tool`, solo reemplaza la entrada si sus líneas cambiaron, y no se
    /// llama desde el camino de los deltas.
    pub(super) fn sync_activity(
        &mut self,
        key: &str,
        kind: &'static str,
        label: &str,
        lines: Vec<batten::state::Line>,
        from_notice: bool,
    ) {
        let shown = self.shown_activity_lines(kind, from_notice, &lines);
        if let Some(entry) = self.activity_entries.iter_mut().find(|e| e.key == key) {
            if entry.lines == lines {
                return;
            }
            entry.label = label.to_string();
            entry.lines = lines;
            self.transcript.replace(entry.id, shown);
            return;
        }
        // Como con las herramientas, la entrada se crea en su sitio cronológico y, al dejar
        // una línea visible, corta el texto del asistente.
        let id = self.transcript.push_lines(shown);
        self.activity_entries.push(ActivityEntry {
            key: key.to_string(),
            id,
            lines,
            kind,
            label: label.to_string(),
            from_notice,
            notice_consumed: false,
        });
        self.assistant = None;
    }

    /// Elemento de actividad de un job de esta sesión. Los jobs de tipo
    /// `subagent` se muestran desde su propio evento.
    pub(super) fn sync_job_activity(&mut self, job: &codeloom_core::job::JobSnapshot) {
        if job.kind == "subagent" || Some(job.owner) != self.session_id {
            return;
        }
        let status = job_tool_status(job.status);
        let body = if status == batten::state::ToolStatus::Running {
            None
        } else {
            job.detail
                .as_deref()
                .or_else(|| job.result.as_ref().map(|result| result.content.as_str()))
        };
        let progress = job
            .progress
            .as_ref()
            .map(|progress| (progress.fraction, progress.label.as_str()));
        let view = batten::state::ActivityView {
            kind: "job",
            label: &job.label,
            status,
            progress,
            body,
        };
        let lines = batten::state::activity_lines(&view);
        let key = format!("job:{}", job.id.0);
        self.sync_activity(&key, "job", &job.label, lines, false);
    }

    /// Elemento de actividad de un subagente de esta sesión: su nodo en el árbol.
    pub(super) fn sync_subagent_activity(
        &mut self,
        subagent: &codeloom_core::session::SubagentDescriptor,
    ) {
        let lines = self.subagent_node_lines(subagent);
        let key = node_key(subagent.child.0);
        self.sync_activity(&key, "subagent", &subagent.label, lines, false);
    }

    /// Líneas del nodo de un subagente: una cabecera y, si está desplegado, una línea por hijo.
    /// Regla de trazabilidad: el estado A2A exacto va entre paréntesis tras la etiqueta, de modo
    /// que el icono y el aspecto agrupado nunca sustituyan al estado real.
    pub(super) fn subagent_node_lines(
        &self,
        subagent: &codeloom_core::session::SubagentDescriptor,
    ) -> Vec<batten::state::Line> {
        let state = subagent.status.state;
        let agente = agent_state(state);
        let tag = agent_tag(subagent);
        let summary = self.node_summary(subagent.child.0, subagent);
        let preview = progress_preview(subagent);
        let preview: Vec<&str> = preview.iter().map(String::as_str).collect();
        let expanded = self.expanded_nodes.contains(&subagent.child.0);
        // Un nodo plegado no pinta a sus hijos: no se construyen para no gastar trabajo.
        let filas = if expanded {
            self.node_children(subagent)
        } else {
            Vec::new()
        };
        let children: Vec<batten::state::NodeChild<'_>> = filas
            .iter()
            .map(|(glyph, text)| batten::state::NodeChild {
                glyph: glyph.as_str(),
                text: text.as_str(),
            })
            .collect();
        // Un descendiente se presenta con el camino completo y sangrado por nivel.
        let (nivel, etiqueta) =
            match profundidad_y_camino(&self.subagents, self.session_id, subagent.child.0) {
                Some((nivel, camino)) if nivel > 1 => (nivel - 1, etiqueta_de_camino(&camino, 48)),
                _ => (0, subagent.label.clone()),
            };
        let view = batten::state::NodeView {
            glyph: agente.glyph(),
            kind: "subagent",
            label: &etiqueta,
            tag: tag.as_deref(),
            state: agente.label(),
            status: subagent_tool_status(state),
            summary: summary.as_deref(),
            expanded,
            indent: nivel,
            preview: &preview,
            children: &children,
        };
        batten::state::node_lines(&view)
    }

    /// Resumen de progreso de un nodo, p. ej. `step 2/30 · read_file · 3 queries`. Omite lo que
    /// falte: el paso solo si el hijo conoce su máximo, la herramienta solo si está ejecutando
    /// algo, y las consultas del padre solo si hay alguna.
    pub(super) fn node_summary(
        &self,
        child: i64,
        subagent: &codeloom_core::session::SubagentDescriptor,
    ) -> Option<String> {
        let stats = self.agent_stats.get(&child);
        let paso = stats
            .filter(|stats| stats.max_steps > 0)
            .map(|stats| format!("step {}/{}", stats.step, stats.max_steps));
        let herramienta = stats
            .and_then(|stats| stats.tool.as_deref())
            .filter(|tool| !tool.is_empty())
            .map(str::to_string);
        let tokens = stats
            .filter(|stats| stats.input_tokens + stats.output_tokens > 0)
            .map(|stats| {
                let provisional = self
                    .active_usage
                    .iter()
                    .any(|((session, _), usage)| *session == child && !usage.provider_final)
                    || self
                        .usage_records
                        .iter()
                        .rev()
                        .find(|record| record.session.0 == child)
                        .is_some_and(|record| record.source == UsageSource::Estimate);
                format!(
                    "{}↑{}/↓{}",
                    if provisional { "~" } else { "" },
                    format_tokens(stats.input_tokens),
                    format_tokens(stats.output_tokens)
                )
            });
        let consultas = self
            .node_actions
            .get(&child)
            .map(Vec::len)
            .filter(|&n| n > 0)
            .map(|n| format!("{n} {}", if n == 1 { "query" } else { "queries" }));
        let avances = Some(subagent.progress_artifacts().count())
            .filter(|&n| n > 0)
            .map(|n| format!("{n} {}", if n == 1 { "note" } else { "notes" }));
        let partes: Vec<String> = [paso, herramienta, tokens, consultas, avances]
            .into_iter()
            .flatten()
            .collect();
        (!partes.is_empty()).then(|| partes.join(" · "))
    }

    /// Filas de los hijos de un nodo, como pares (icono, texto), en el orden del registro. El
    /// texto de un mensaje del hijo y de una herramienta va en una sola línea. Si el subagente
    /// ha terminado y tiene resultado, la última fila es su primera línea no vacía.
    pub(super) fn node_children(
        &self,
        subagent: &codeloom_core::session::SubagentDescriptor,
    ) -> Vec<(String, String)> {
        use batten::state::ToolStatus;
        let state = subagent.status.state;
        let mut filas: Vec<(String, String)> = self
            .child_logs
            .get(&subagent.child.0)
            .map(|log| {
                log.items
                    .iter()
                    .map(|item| match item {
                        ChildItem::Text(texto) => ("›".to_string(), una_linea(texto)),
                        ChildItem::Tool {
                            name,
                            summary,
                            status,
                            ..
                        } => {
                            let glifo = match status {
                                ToolStatus::Running => "…",
                                ToolStatus::Success => "✓",
                                ToolStatus::Error => "✗",
                                _ => "·",
                            };
                            let texto = match summary.as_deref().filter(|s| !s.is_empty()) {
                                Some(summary) => format!("{name} {summary}"),
                                None => name.clone(),
                            };
                            (glifo.to_string(), una_linea(&texto))
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Las llamadas del padre sobre este hijo cuelgan tras su registro, en orden de llegada.
        // El glifo `↳` lo pinta el widget como `├─ ↳ parent: …` (o `└─` en la última fila).
        if let Some(acciones) = self.node_actions.get(&subagent.child.0) {
            filas.extend(
                acciones
                    .iter()
                    .map(|nombre| ("↳".to_string(), format!("parent: {nombre}"))),
            );
        }
        if subagent.status.state.is_terminal() {
            if matches!(
                state,
                codeloom_core::task::TaskState::Failed | codeloom_core::task::TaskState::Rejected
            ) {
                let error = subagent
                    .status
                    .message
                    .as_ref()
                    .map(|message| {
                        message
                            .parts
                            .iter()
                            .filter_map(codeloom_core::a2a::Part::as_text)
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| format!("subagent ended in {state:?}"));
                filas.push((
                    "✗".to_string(),
                    format!("error: {}", una_linea(&recortar(&error, MAX_CHILD_TEXT))),
                ));
            }
            let resultado = subagent.result_text().unwrap_or_default();
            let primera = resultado
                .lines()
                .map(str::trim)
                .find(|linea| !linea.is_empty());
            if let Some(primera) = primera {
                filas.push(("✓".to_string(), format!("result: {primera}")));
            }
        }
        filas
    }

    /// Líneas visibles de una entrada de actividad. Un nodo de subagente muestra **todas** sus
    /// líneas también con el look compacto: ya es plegable por sí mismo. Las demás entradas
    /// (jobs, avisos) solo muestran su cabecera con el look compacto. Los elementos que nacen
    /// de un aviso no son nodos: conservan su cuerpo y su visibilidad de siempre.
    pub(super) fn shown_activity_lines(
        &self,
        kind: &str,
        from_notice: bool,
        lines: &[batten::state::Line],
    ) -> Vec<batten::state::Line> {
        if (kind == "subagent" && !from_notice) || self.tool_inline() {
            lines.to_vec()
        } else {
            lines.iter().take(1).cloned().collect()
        }
    }

    /// Convierte un aviso en elemento de actividad. Devuelve `false` si el texto
    /// no es un aviso reconocible. Un aviso de job no crea nada si ya existe su
    /// elemento; uno de subagente reclama el primer elemento en vivo sin consumir
    /// con su misma etiqueta, o crea uno nuevo si no hay ninguno.
    pub(super) fn apply_notice(&mut self, text: &str) -> bool {
        let Some(notice) = parse_notice(text) else {
            return false;
        };
        match notice {
            Notice::Job {
                id,
                status,
                label,
                body,
            } => {
                let key = format!("job:{id}");
                if !self.activity_entries.iter().any(|e| e.key == key) {
                    self.push_notice_entry("job", status, label, body);
                }
            }
            Notice::Subagent { label } => {
                let live = self.activity_entries.iter_mut().find(|e| {
                    !e.from_notice && e.kind == "subagent" && e.label == label && !e.notice_consumed
                });
                match live {
                    Some(entry) => entry.notice_consumed = true,
                    // El aviso ya dice que el hijo terminó: el estado exacto es `completed`.
                    None => self.push_notice_entry(
                        "subagent",
                        batten::state::ToolStatus::Success,
                        &format!("{label} (completed)"),
                        "",
                    ),
                }
            }
            Notice::SubagentInput { label, body } => {
                if !self.activity_entries.iter().any(|entry| {
                    !entry.from_notice && entry.kind == "subagent" && entry.label == label
                }) {
                    self.push_notice_entry(
                        "subagent",
                        batten::state::ToolStatus::Running,
                        &format!("{label} (needs input)"),
                        body,
                    );
                }
            }
            // Mismo mecanismo que un aviso de subagente, pero con el tipo `codeloom`: la
            // cabecera no lleva ningún estado de A2A.
            Notice::Compaction { covers, body } => self.push_notice_entry(
                "codeloom",
                batten::state::ToolStatus::Success,
                &compaction_label(covers),
                &body,
            ),
        }
        true
    }

    /// Crea un elemento nacido de un aviso. Su clave usa el número de elementos
    /// existentes, que solo crece hasta el próximo `restore_messages`.
    pub(super) fn push_notice_entry(
        &mut self,
        kind: &'static str,
        status: batten::state::ToolStatus,
        label: &str,
        body: &str,
    ) {
        let key = format!("notice:{}", self.activity_entries.len());
        let view = batten::state::ActivityView {
            kind,
            label,
            status,
            progress: None,
            body: Some(body),
        };
        let lines = batten::state::activity_lines(&view);
        self.sync_activity(&key, kind, label, lines, true);
    }
}
