//! Sección de subagentes y su estado visual.

use super::*;

/// Subagentes de la sesión con su estado A2A y sus métricas.
pub(super) fn push_subagents<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    let session = app.session_id;
    let subagents = || {
        app.subagents
            .values()
            .filter(move |subagent| Some(subagent.parent) == session)
    };
    let subagent_total = subagents().count();
    if subagent_total > 0 {
        push_gap(out);
        out.push(subagents_title(app));
        // Regla de trazabilidad: bajo el recuento de subagentes en curso se nombra su desglose
        // por estado A2A exacto, partido para caber en el rail. Sin subagentes en curso no hay nada que nombrar.
        if app.active_agents() > 0 {
            let activos: Vec<(TaskState, usize)> = app
                .subagent_state_counts()
                .into_iter()
                .filter(|(estado, _)| !estado.is_terminal())
                .collect();
            let grupo = format!("({})", formato_recuento(&activos));
            for linea in partir_por_palabras(&grupo, width) {
                out.push(Line::from(Span::styled(linea, app.theme.styles.muted)));
            }
        }
        // Un único `Instant::now()` por dibujado: el tiempo transcurrido de todos los hijos.
        let ahora = Instant::now();
        for subagent in subagents().take(SUBAGENT_LIMIT) {
            let estado = agent_state(subagent.status.state);
            // Con el foco en el rail, el hijo elegido con ↑/↓ lleva `›` de acento en vez del glifo.
            // El glifo y el `›` ocupan una columna más el espacio, así que la línea no cambia de anchura.
            let seleccionado =
                app.focus == Focus::Rail && app.rail_selected == Some(subagent.child.0);
            // La etiqueta del agente, si se conoce, va tenue y antes del nombre del hijo.
            let mut spans = if seleccionado {
                // El `›` sustituye al glifo, de modo que una espera del hijo se conserva
                // tiñéndolo con el color de aviso en lugar del acento.
                let marca = if estado == AgentState::InputRequired {
                    app.theme.styles.warning
                } else {
                    app.theme.styles.accent
                };
                vec![Span::styled("› ", marca)]
            } else {
                vec![Span::styled(
                    format!("{} ", estado.glyph()),
                    estado.style(&app.theme.styles),
                )]
            };
            if let Some(etiqueta) = agent_tag(subagent) {
                spans.push(Span::styled(etiqueta, app.theme.styles.muted));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::raw(subagent.label.as_str()));
            // Tiempo transcurrido al final de la primera línea, tenue. Sin arranque, no hay tiempo que dar.
            let stats = app.agent_stats.get(&subagent.child.0);
            if let Some(inicio) = stats.and_then(|stats| stats.started_at) {
                let fin = stats.and_then(|stats| stats.finished_at).unwrap_or(ahora);
                spans.push(Span::styled(
                    format!(" {}", format_elapsed(fin.saturating_duration_since(inicio))),
                    app.theme.styles.muted,
                ));
            }
            out.push(Line::from(spans));
            // Segunda línea tenue con las métricas, solo para los hijos en marcha y con estadísticas.
            // Un hijo terminado ya muestra su tiempo en la primera línea y ocupa una sola.
            let stats = stats.filter(|_| !subagent.status.state.is_terminal());
            if let Some(stats) = stats {
                let coste = child_cost_label(
                    app.child_cost(subagent.child),
                    app.child_cost_is_estimated(subagent.child),
                );
                let ficheros = app.child_file_count(subagent.child);
                let metricas = metrics_line(stats, coste, ficheros, ahora, width.saturating_sub(2));
                if !metricas.is_empty() {
                    out.push(Line::from(Span::styled(
                        format!("  {metricas}"),
                        app.theme.styles.muted,
                    )));
                }
            }
        }
        more(out, subagent_total, SUBAGENT_LIMIT, app);
    }
}

/// Presentación de un estado de tarea de A2A como estado visual. El `match` es exhaustivo
/// y sin comodín: un estado nuevo del núcleo obliga a decidir su aspecto. Un estado de A2A
/// tiene un único glifo, así que la presentación no agrupa nada aquí.
pub fn agent_state(state: TaskState) -> AgentState {
    match state {
        TaskState::Submitted => AgentState::Submitted,
        TaskState::Working => AgentState::Working,
        TaskState::InputRequired => AgentState::InputRequired,
        TaskState::AuthRequired => AgentState::AuthRequired,
        TaskState::Completed => AgentState::Completed,
        TaskState::Failed => AgentState::Failed,
        TaskState::Canceled => AgentState::Canceled,
        TaskState::Rejected => AgentState::Rejected,
    }
}

/// Título de Agents, en negrita y con el estilo por defecto. Con actividad o tokens acumulados
/// añade el número de agentes en marcha y los totales de la jerarquía: `Agents · 2 · 2k/400`.
pub(super) fn subagents_title(app: &App) -> Line<'static> {
    let (input, output) = app.hierarchy_tokens();
    let activos = app.active_agents();
    let texto = if (input, output) != (0, 0) || activos > 0 {
        format!(
            "Agents · {activos} · {}/{}",
            format_tokens(input),
            format_tokens(output)
        )
    } else {
        "Agents".to_owned()
    };
    Line::from(Span::styled(
        texto,
        app.theme.styles.default.add_modifier(Modifier::BOLD),
    ))
}
