//! Superposiciones de A2A y de agentes: paneles, detalles, listado y editor de agentes.

use super::*;

/// Texto del estado de aprobación, tal como se rotula en el popup de agentes.
pub(super) fn approval_label(approval: loomcoding_core::registry::Approval) -> &'static str {
    use loomcoding_core::registry::Approval;
    match approval {
        Approval::NotRequired => "usable",
        Approval::Approved => "approved",
        Approval::Pending => "awaiting approval",
    }
}

pub(super) fn a2a_clip(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let clipped: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

pub(super) fn a2a_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[Line<'static>],
    selected: usize,
    focused: bool,
    app: &App,
) {
    let style = if focused {
        app.theme.styles.accent
    } else {
        app.theme.styles.border
    };
    let block = Block::default()
        .title(format!(" {title} · {} ", items.len()))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(style);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let start = selected.saturating_sub(usize::from(inner.height).saturating_sub(1));
    let lines = items
        .iter()
        .skip(start)
        .take(usize::from(inner.height))
        .cloned()
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

pub(super) fn a2a_agent_lines(
    app: &App,
    agents: &[crate::app::A2aAgentView],
) -> Vec<Line<'static>> {
    agents
        .iter()
        .enumerate()
        .map(|(index, agent)| {
            let approval = if agent.internal {
                "internal · enabled"
            } else {
                agent.approval.map(approval_label).unwrap_or("local")
            };
            let style = if index == app.a2a_agent_selected {
                app.theme.styles.heading
            } else {
                Style::default()
            };
            Line::styled(
                format!(
                    "{} {} · {} · v{}",
                    if index == app.a2a_agent_selected {
                        "›"
                    } else {
                        " "
                    },
                    a2a_clip(&agent.name, 42),
                    approval,
                    agent.card.version
                ),
                style,
            )
        })
        .collect()
}

pub(super) fn a2a_task_lines(app: &App) -> Vec<Line<'static>> {
    app.subagents
        .values()
        .enumerate()
        .map(|(index, task)| {
            let state = crate::rail::agent_state(task.status.state);
            let style = if index == app.a2a_task_selected {
                app.theme.styles.heading
            } else {
                Style::default()
            };
            Line::styled(
                format!(
                    "{} {} {} · {} · local task {}",
                    if index == app.a2a_task_selected {
                        "›"
                    } else {
                        " "
                    },
                    state.glyph(),
                    a2a_clip(&task.label, 38),
                    state.label(),
                    task.child.0
                ),
                style,
            )
        })
        .collect()
}

pub(super) fn a2a_event_lines(app: &App) -> Vec<Line<'static>> {
    app.a2a_observations
        .iter()
        .rev()
        .enumerate()
        .map(|(index, event)| {
            let direction = match event.direction {
                A2aDirection::Local => "HOST",
                A2aDirection::Inbound => "IN ",
                A2aDirection::Outbound => "OUT",
            };
            let status = if event.succeeded { "✓" } else { "×" };
            let state = event
                .state
                .as_deref()
                .map(|value| format!(" · {value}"))
                .unwrap_or_default();
            let elapsed = event
                .elapsed_ms
                .map(|value| format!(" · {value} ms"))
                .unwrap_or_default();
            let task = event
                .task_id
                .as_deref()
                .map(|value| format!(" · #{value}"))
                .unwrap_or_default();
            Line::styled(
                format!(
                    "{} {direction} {} · {} {status}{task}{state}{elapsed}",
                    if index == app.a2a_event_selected {
                        "›"
                    } else {
                        " "
                    },
                    a2a_clip(&event.agent, 32),
                    a2a_operation_name(event.operation)
                ),
                if index == app.a2a_event_selected {
                    app.theme.styles.heading
                } else if event.succeeded {
                    Style::default()
                } else {
                    app.theme.styles.error
                },
            )
        })
        .collect()
}

pub(super) fn a2a_operation_name(operation: A2aOperation) -> &'static str {
    match operation {
        A2aOperation::SendMessage => "SendMessage",
        A2aOperation::SendStreamingMessage => "SendStreamingMessage",
        A2aOperation::GetTask => "GetTask",
        A2aOperation::CancelTask => "CancelTask",
        A2aOperation::SubscribeToTask => "SubscribeToTask",
        A2aOperation::TaskStatusUpdateEvent => "TaskStatusUpdateEvent",
        A2aOperation::TaskArtifactUpdateEvent => "TaskArtifactUpdateEvent",
        A2aOperation::TaskStateObserved => "host task state",
    }
}

pub(super) fn a2a_details(app: &App, agents: &[crate::app::A2aAgentView]) -> Vec<Line<'static>> {
    match app.a2a_focus {
        0 => {
            let Some(agent) = agents.get(app.a2a_agent_selected) else {
                return vec![Line::from("Sin tarjetas de agentes en el registro.")];
            };
            let card = &agent.card;
            let capability = |value: Option<bool>| match value {
                Some(true) => "yes",
                Some(false) => "no",
                None => "undeclared",
            };
            let mut lines = vec![
                Line::from(Span::styled(
                    a2a_clip(&card.description, 180),
                    app.theme.styles.heading,
                )),
                Line::from(format!(
                    "origin {} · input {} · output {}",
                    agent.origin,
                    a2a_clip(&card.default_input_modes.join(", "), 48),
                    a2a_clip(&card.default_output_modes.join(", "), 48)
                )),
                Line::from(format!(
                    "{} · {}",
                    if agent.internal {
                        "internal"
                    } else {
                        "external"
                    },
                    if agent.enabled {
                        "enabled"
                    } else {
                        "disabled or pending approval"
                    }
                )),
                Line::from(format!(
                    "streaming {} · push {} · extended card {} · extensions {}",
                    capability(card.capabilities.streaming),
                    capability(card.capabilities.push_notifications),
                    capability(card.capabilities.extended_agent_card),
                    card.capabilities.extensions.len()
                )),
            ];
            if let Some(provider) = &card.provider {
                lines.push(Line::from(format!(
                    "provider {} · {}",
                    a2a_clip(&provider.organization, 48),
                    a2a_clip(&provider.url, 100)
                )));
            }
            for interface in card.supported_interfaces.iter().take(16) {
                lines.push(Line::from(format!(
                    "{} · {} · A2A {}{}",
                    interface.protocol_binding,
                    a2a_clip(&interface.url, 100),
                    interface.protocol_version,
                    interface
                        .tenant
                        .as_deref()
                        .map(|tenant| format!(" · tenant {}", a2a_clip(tenant, 40)))
                        .unwrap_or_default()
                )));
            }
            if card.supported_interfaces.is_empty() {
                lines.push(Line::from("No supported interface declared."));
            } else if card.supported_interfaces.len() > 16 {
                lines.push(Line::from(format!(
                    "… {} more interfaces",
                    card.supported_interfaces.len() - 16
                )));
            }
            if let Some(url) = &card.documentation_url {
                lines.push(Line::from(format!("documentation {}", a2a_clip(url, 100))));
            }
            for skill in card.skills.iter().take(8) {
                lines.push(Line::from(format!(
                    "skill {} ({})",
                    a2a_clip(&skill.name, 48),
                    a2a_clip(&skill.id, 32)
                )));
                lines.push(Line::from(a2a_clip(&skill.description, 140)));
                lines.push(Line::from(format!(
                    "tags {} · input {} · output {}",
                    a2a_clip(&skill.tags.join(", "), 64),
                    a2a_clip(&skill.input_modes.join(", "), 40),
                    a2a_clip(&skill.output_modes.join(", "), 40)
                )));
                for example in skill.examples.iter().take(2) {
                    lines.push(Line::from(format!("example {}", a2a_clip(example, 100))));
                }
            }
            if card.skills.len() > 8 {
                lines.push(Line::from(format!(
                    "… {} more skills",
                    card.skills.len() - 8
                )));
            }
            for extension in card.capabilities.extensions.iter().take(16) {
                lines.push(Line::from(format!(
                    "extension {} · required {} · {}",
                    a2a_clip(&extension.uri, 80),
                    extension.required,
                    a2a_clip(&extension.description, 100)
                )));
            }
            if card.capabilities.extensions.len() > 16 {
                lines.push(Line::from(format!(
                    "… {} more extensions",
                    card.capabilities.extensions.len() - 16
                )));
            }
            if let Some(loomcoding_core::registry::Execution::Remote(remote)) = &agent.execution {
                let auth = remote
                    .credentials
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(Line::from(format!(
                    "auth schemes configured: {}",
                    if auth.is_empty() {
                        "none".to_string()
                    } else {
                        a2a_clip(&auth, 80)
                    }
                )));
                if let Some(fetched) = &remote.fetched {
                    lines.push(Line::from(format!(
                        "card hash {} · auth required {} · streaming {}{}",
                        fetched.card_hash.chars().take(12).collect::<String>(),
                        fetched.requires_auth,
                        fetched.streaming,
                        fetched
                            .auth_problem
                            .as_deref()
                            .map(|problem| {
                                format!(" · unavailable: {}", a2a_clip(problem, 100))
                            })
                            .unwrap_or_default()
                    )));
                }
            }
            lines
        }
        1 => {
            let Some(task) = app.subagents.values().nth(app.a2a_task_selected) else {
                return vec![Line::from("Sin tareas A2A en esta sesión.")];
            };
            let state = crate::rail::agent_state(task.status.state);
            let mut lines = vec![
                Line::from(format!(
                    "Loomcoding task {} · state {}",
                    task.child.0,
                    state.label()
                )),
                Line::from(format!("contextId {}", task.context_id())),
                Line::from(format!(
                    "agent {} · origin {}",
                    task.agent
                        .as_deref()
                        .map(|agent| a2a_clip(agent, 48))
                        .unwrap_or_else(|| "default".into()),
                    task.agent_origin
                        .as_deref()
                        .and_then(|origin| origin.split(':').next())
                        .unwrap_or("local")
                )),
                Line::from(format!(
                    "artifacts {} · reference_task_ids {}",
                    task.artifacts.len(),
                    task.reference_task_ids
                        .iter()
                        .map(|id| id.0.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            ];
            for artifact in task.artifacts.iter().take(16) {
                lines.push(Line::from(format!(
                    "artifact {} · {} parts",
                    a2a_clip(
                        artifact.name.as_deref().unwrap_or(&artifact.artifact_id),
                        80
                    ),
                    artifact.parts.len()
                )));
            }
            if task.artifacts.len() > 16 {
                lines.push(Line::from(format!(
                    "… {} more artifacts",
                    task.artifacts.len() - 16
                )));
            }
            lines
        }
        _ => {
            let Some(event) = app
                .a2a_observations
                .iter()
                .rev()
                .nth(app.a2a_event_selected)
            else {
                return vec![Line::from("Sin observaciones de actividad A2A.")];
            };
            vec![
                Line::from(format!(
                    "{} · {} · {}",
                    match event.direction {
                        A2aDirection::Local => "local host",
                        A2aDirection::Inbound => "inbound",
                        A2aDirection::Outbound => "outbound",
                    },
                    event.agent,
                    a2a_operation_name(event.operation)
                )),
                Line::from(format!(
                    "taskId {}",
                    event.task_id.as_deref().unwrap_or("—")
                )),
                Line::from(format!(
                    "contextId {} · rpc id {}",
                    event.context_id.as_deref().unwrap_or("—"),
                    event.rpc_id.as_deref().unwrap_or("—")
                )),
                Line::from(format!(
                    "state {} · artifact {}",
                    event.state.as_deref().unwrap_or("—"),
                    event.artifact.as_deref().unwrap_or("—")
                )),
                Line::from(format!(
                    "result {} · duration {} ms",
                    if event.succeeded { "ok" } else { "error" },
                    event
                        .elapsed_ms
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "—".into())
                )),
            ]
        }
    }
}

pub(super) fn draw_a2a_overlay(frame: &mut Frame, area: Rect, app: &App) {
    if area.width < 76 || area.height < 18 {
        draw_resize_hint(frame, area, "A2A observatory", app);
        return;
    }
    let width = area.width.min(136).saturating_sub(2);
    let height = area.height.min(36).saturating_sub(2);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" A2A · PROTOCOL OBSERVATORY ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.accent);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(7),
        Constraint::Length(1),
    ])
    .split(inner);
    let agents = app.a2a_agents();
    let tasks = a2a_task_lines(app);
    let events = a2a_event_lines(app);
    frame.render_widget(
        Paragraph::new(format!(
            " {} agents · {} tasks · {} exchanges · payloads and credentials excluded",
            agents.len(),
            app.subagents.len(),
            app.a2a_observations.len()
        ))
        .style(app.theme.styles.muted),
        rows[0],
    );
    let [agent_area, task_area, event_area] = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(34),
        Constraint::Percentage(36),
    ])
    .areas(rows[1]);
    a2a_panel(
        frame,
        agent_area,
        "AGENT CARDS",
        &a2a_agent_lines(app, &agents),
        app.a2a_agent_selected,
        app.a2a_focus == 0,
        app,
    );
    a2a_panel(
        frame,
        task_area,
        "TASKS",
        &tasks,
        app.a2a_task_selected,
        app.a2a_focus == 1,
        app,
    );
    a2a_panel(
        frame,
        event_area,
        "JSON-RPC / SSE",
        &events,
        app.a2a_event_selected,
        app.a2a_focus == 2,
        app,
    );
    let detail = a2a_details(app, &agents);
    frame.render_widget(
        Paragraph::new(detail)
            .scroll((app.a2a_detail_scroll, 0))
            .block(
                Block::default()
                    .title(" DETAIL · A2A OBJECT ")
                    .borders(Borders::ALL)
                    .border_style(app.theme.styles.border),
            ),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new("Tab focus · ↑/↓ select · PgUp/PgDn detail · Esc close")
            .style(app.theme.styles.muted)
            .alignment(Alignment::Right),
        rows[3],
    );
}

/// Popup de `/agents`: la lista del registro con el agente seleccionado y, debajo, su detalle
/// (origen y prompt), que es lo que se revisa antes de aprobar un agente de proyecto.
pub(super) fn draw_agents_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(100).saturating_sub(2);
    let height = area.height.min(30).saturating_sub(2);
    if width < 40 || height < 10 {
        draw_resize_hint(frame, area, "Agents", app);
        return;
    }
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(Line::styled(" Agents ", app.theme.styles.heading))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Reparto vertical: filtros, lista/detalle y dos líneas de pie.
    let util = inner.height.saturating_sub(3);
    let alto_lista = (util / 2).max(2);
    let por_pagina = (alto_lista / 2).max(1) as usize;
    let desde = app
        .agents_selected
        .saturating_sub(por_pagina.saturating_sub(1));
    let mut lineas: Vec<Line> = Vec::new();
    for (indice, fila) in app
        .agent_rows
        .iter()
        .enumerate()
        .skip(desde)
        .take(por_pagina)
    {
        let marca = if indice == app.agents_selected {
            "›"
        } else {
            " "
        };
        let nombre_estilo = if indice == app.agents_selected {
            app.theme.styles.heading.add_modifier(Modifier::BOLD)
        } else {
            app.theme.styles.accent
        };
        let estilo = if indice == app.agents_selected {
            app.theme.styles.heading.add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        // Línea 1: nombre: descripción [approval] origin — execution
        lineas.push(Line::from(vec![
            Span::styled(format!("{marca} "), estilo),
            Span::styled(fila.name.clone(), nombre_estilo),
            Span::styled(
                format!(
                    ": {} [{}] {} — {}",
                    fila.description,
                    approval_label(fila.approval),
                    fila.origin,
                    fila.execution
                ),
                estilo,
            ),
        ]));
        // Línea 2: directorio si existe
        if let Some(path) = &fila.profile_path {
            let dir = path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            if !dir.is_empty() {
                lineas.push(Line::styled(
                    format!("    (directory: {})", dir),
                    app.theme.styles.muted,
                ));
            }
        }
    }
    if app.agent_rows.is_empty() {
        lineas.push(Line::styled(
            "No agents loaded (no registry available)",
            app.theme.styles.muted,
        ));
    }
    while lineas.len() < alto_lista as usize {
        lineas.push(Line::from(""));
    }
    lineas.truncate(alto_lista as usize);
    if let Some(fila) = app.agent_rows.get(app.agents_selected) {
        lineas.push(Line::styled(
            format!("── Prompt of `{}` ──", fila.name),
            app.theme.styles.heading,
        ));
        let restante = util.saturating_sub(alto_lista).saturating_sub(1) as usize;
        if fila.body.trim().is_empty() {
            lineas.push(Line::styled(
                "(empty: inherits the parent's behaviour)",
                app.theme.styles.muted,
            ));
        } else {
            lineas.extend(fila.body.lines().take(restante).map(Line::from));
        }
    }
    let pie = app.agents_status.clone().unwrap_or_default();
    let mut todo = lineas;
    // El pie va siempre en las dos últimas filas del interior.
    todo.truncate(util as usize);
    while todo.len() < util as usize {
        todo.push(Line::from(""));
    }
    todo.push(Line::styled(pie, app.theme.styles.muted));
    todo.push(Line::styled(
        "↑/↓ select · / text · t tag · Tab switch · x clear · e edit · a approve · r revoke · Esc close",
        app.theme.styles.muted,
    ));
    // Mostrar filtros activos con indicador de foco
    let tags_str = app
        .agents_tag_filter
        .as_ref()
        .map(|v| v.join(", "))
        .unwrap_or_default();
    let focus_query = app.agents_focus == AgentsFocus::Query;
    let focus_tag = app.agents_focus == AgentsFocus::Tag;
    let focus_list = app.agents_focus == AgentsFocus::List;
    let query_cursor = if focus_query { "_" } else { "" };
    let tag_cursor = if focus_tag { "_" } else { "" };
    let list_marker = if focus_list { "▸ " } else { "  " };
    // En modo Tag, mostrar el buffer de entrada
    let tag_display = if focus_tag && !app.agents_tag_input.is_empty() {
        format!(
            "{tags_str}{}{}",
            if !tags_str.is_empty() { ", " } else { "" },
            app.agents_tag_input
        )
    } else {
        tags_str
    };
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!(
                "{list_marker}Text: {}{query_cursor} · Tag: {}{tag_cursor}",
                app.agents_query, tag_display
            ),
            app.theme.styles.muted,
        )),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new(todo),
        Rect::new(
            inner.x,
            inner.y.saturating_add(1),
            inner.width,
            inner.height.saturating_sub(1),
        ),
    );
}

pub(super) fn draw_agent_editor(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    edit: &crate::app::AgentEditorDraft,
) {
    if edit.editing_value {
        let label = [
            "A2A · Description",
            "A2A · Version",
            "A2A · Skills",
            "Loomcoding · MCP scope (not applied)",
            "Loomcoding · Local skills",
            "Loomcoding · Local tools",
        ][edit.field.min(5)];
        let box_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(2));
        frame.render_widget(
            Paragraph::new(app.agent_editor.text().to_string())
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(format!(" {label} · Enter save field · Esc cancel field "))
                        .borders(Borders::ALL)
                        .border_style(app.theme.styles.accent),
                ),
            box_area,
        );
        let (row, col) = app.agent_editor.cursor_row_col();
        let x = box_area.x.saturating_add(1).saturating_add(col as u16);
        let y = box_area.y.saturating_add(1).saturating_add(row as u16);
        if x < box_area.right() && y < box_area.bottom() {
            frame.set_cursor_position((x, y));
        }
        frame.render_widget(
            Paragraph::new("Ctrl+Enter accept · Ctrl+J newline · Esc cancel field")
                .style(app.theme.styles.muted),
            Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        );
        return;
    }

    let Some(row) = app.agent_rows.get(app.agents_selected) else {
        return;
    };
    let selected = |field| if field == edit.field { "›" } else { " " };
    let caps = |value: Option<bool>| match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "undeclared",
    };
    let mut lines = vec![
        Line::styled("A2A · PUBLIC CARD", app.theme.styles.accent),
        Line::from(format!("{} Description: {}", selected(0), a2a_clip(&edit.description, 96))),
        Line::from(format!("{} Version: {}", selected(1), a2a_clip(&edit.version, 64))),
        Line::from(format!(
            "{} Skills: {} (one per line: id | name | description | tags | examples | input | output)",
            selected(2),
            row.card_skills.len()
        )),
        Line::from(format!(
            "  Transport capabilities · streaming {} · push {} · extended card {} (runtime)",
            caps(row.capabilities.streaming),
            caps(row.capabilities.push_notifications),
            caps(row.capabilities.extended_agent_card)
        )),
        Line::from(""),
        Line::styled("Loomcoding · LOCAL EXECUTION", app.theme.styles.accent),
        Line::from(format!("{} MCP servers allowed (user-configured and connected): {}", selected(3), scope_count(&edit.mcp))),
        Line::from(format!("{} Local skills: {}", selected(4), scope_count(&edit.local_skills))),
        Line::from(format!("{} Local tools: {}", selected(5), scope_count(&edit.tools))),
        Line::from(""),
        Line::from(format!("{} Save", selected(6))),
    ];
    if let Some(status) = &app.agents_status {
        lines.push(Line::styled(status.clone(), app.theme.styles.warning));
    }
    lines.push(Line::styled(
        "↑/↓ field · Enter edit/save · s save · Esc discard",
        app.theme.styles.muted,
    ));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

pub(super) fn scope_count(text: &str) -> usize {
    text.lines()
        .flat_map(|line| line.split(','))
        .filter(|item| !item.trim().is_empty())
        .count()
}
