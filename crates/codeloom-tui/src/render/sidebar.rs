//! Panel lateral del espacio: métricas, avisos de MCP, desglose de contexto y de caché.

use super::*;

/// Umbrales de partida para avisar de un exceso de definiciones MCP (candidata A2): proceden de la
/// bibliografía citada en la spec, no de una medición propia.
pub(super) const MCP_TOOLS_WARNING: usize = 30;

pub(super) const MCP_TOKENS_WARNING: u64 = 10_000;

/// Línea de un servidor MCP en el panel de capacidades: herramientas y tokens estimados.
pub(super) fn mcp_server_summary(server: &crate::app::McpServerCapability) -> String {
    let noun = if server.tool_count == 1 {
        "tool"
    } else {
        "tools"
    };
    format!(
        "{} · {} {noun} · ~{} tok",
        server.id, server.tool_count, server.schema_tokens
    )
}

/// Aviso cuando el conjunto de servidores conectados supera alguno de los umbrales, o `None`.
pub(super) fn mcp_load_warning(servers: &[crate::app::McpServerCapability]) -> Option<String> {
    let live = servers.iter().filter(|server| server.initialized);
    let (tools, tokens) = live.fold((0usize, 0u64), |(tools, tokens), server| {
        (
            tools.saturating_add(server.tool_count),
            tokens.saturating_add(server.schema_tokens),
        )
    });
    if tools > MCP_TOOLS_WARNING {
        Some(format!(
            "{tools} tools (~{tokens} tok) exceed {MCP_TOOLS_WARNING}: selection may degrade"
        ))
    } else if tokens > MCP_TOKENS_WARNING {
        Some(format!(
            "~{tokens} tok of tool schemas exceed {MCP_TOKENS_WARNING}: consider fewer servers"
        ))
    } else {
        None
    }
}

/// Línea del desglose estimado de la última petición propia.
pub(super) fn context_breakdown_line(
    breakdown: &codeloom_core::agent::compact::ContextBreakdown,
) -> String {
    format!(
        "Context ~{} · sys {} · tools {} · msgs {}",
        breakdown.total(),
        breakdown.system,
        breakdown.tools,
        breakdown.messages
    )
}

/// Línea de la caché acumulada en la sesión, tal como la comunica el proveedor. `None` si ambos
/// acumulados son cero: cero significa «no informado o sin caché», no una medición.
pub(super) fn cache_usage_line(records: &[codeloom_core::session::UsageRecord]) -> Option<String> {
    let (read, write) = records.iter().fold((0u64, 0u64), |(read, write), record| {
        (
            read.saturating_add(record.cache_read_tokens),
            write.saturating_add(record.cache_write_tokens),
        )
    });
    (read > 0 || write > 0).then(|| format!("Cache read {read} · write {write}"))
}

pub(super) fn draw_workspace_metrics(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    panes: &mut Vec<(Focus, Rect)>,
) {
    // Cada línea con dato (desglose, caché) añade una fila al panel «Token usage»; sin dato, no cambia.
    let cache_line = cache_usage_line(&app.usage_records);
    let extra_rows = u16::from(app.context_breakdown.is_some()) + u16::from(cache_line.is_some());
    let rows = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(6 + extra_rows),
        Constraint::Min(3),
    ])
    .split(area);
    // «Token usage» (rows[1]) pertenece a la misma ventana Agent.
    panes.push((Focus::Agent, rows[0]));
    panes.push((Focus::Agent, rows[1]));
    let (agent_info, agent_tone) = if app.status.starts_with("error:") {
        ("error", batten::state::Tone::Error)
    } else if app.status == "working" {
        ("active", batten::state::Tone::Info)
    } else {
        ("idle", batten::state::Tone::Neutral)
    };
    let agent = batten::widgets::panel::render(
        frame,
        rows[0],
        &batten::widgets::Panel {
            title: "Agent",
            info: agent_info,
            tone: agent_tone,
        },
        &app.theme,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(app.model.as_str()),
            Line::from(format!(
                "{} · {:.1}s",
                app.phase,
                app.phase_duration().as_secs_f64()
            )),
            Line::from(app.phase_summary()),
            Line::from(if app.max_steps > 0 {
                format!("step {}/{}", app.current_step, app.max_steps)
            } else {
                app.status.clone()
            }),
        ])
        .wrap(Wrap { trim: true }),
        agent,
    );
    let context = batten::widgets::panel::render(
        frame,
        rows[1],
        &batten::widgets::Panel {
            title: "Token usage",
            info: "turn/session",
            tone: batten::state::Tone::Neutral,
        },
        &app.theme,
    );
    let mut usage_lines = vec![
        Line::from(format!(
            "Turn ↑{} ↓{}",
            app.turn_input_tokens, app.turn_output_tokens
        )),
        Line::from(format!(
            "Session ↑{} ↓{}",
            app.input_tokens, app.output_tokens
        )),
        Line::from(format!(
            "Family ↑{} ↓{}",
            app.family_tokens().0,
            app.family_tokens().1
        )),
        Line::from(
            app.active_usage_label()
                .unwrap_or_else(|| "Request · sin stream activo".into()),
        ),
    ];
    if let Some(breakdown) = &app.context_breakdown {
        usage_lines.push(Line::from(context_breakdown_line(breakdown)));
    }
    usage_lines.extend(cache_line.map(Line::from));
    frame.render_widget(Paragraph::new(usage_lines), context);
    let [activity, mcp, lsp, skills] = Layout::vertical([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .areas(rows[2]);

    let activity_lines = app
        .tools
        .iter()
        .rev()
        .take(64)
        .map(|tool| {
            let (symbol, style, status) = match tool.status() {
                batten::state::ToolStatus::Pending => ("○", app.theme.styles.muted, "pending"),
                batten::state::ToolStatus::Running => {
                    ("◌", app.theme.styles.tool_running, "running")
                }
                batten::state::ToolStatus::Success => ("✓", app.theme.styles.success, "done"),
                batten::state::ToolStatus::Error => ("!", app.theme.styles.error, "failed"),
                batten::state::ToolStatus::Canceled => ("×", app.theme.styles.muted, "canceled"),
                batten::state::ToolStatus::AwaitingPermission => {
                    ("?", app.theme.styles.warning, "permission")
                }
            };
            Line::from(vec![
                Span::styled(format!("{symbol} "), style),
                Span::raw(format!("{} · {status}", tool.name)),
            ])
        })
        .collect::<Vec<_>>();
    let activity_lines = if activity_lines.is_empty() {
        vec![Line::from("No recent tools")]
    } else {
        activity_lines
    };
    draw_scrolled_panel(
        frame,
        activity,
        "Activity",
        Focus::Activity,
        &activity_lines,
        app.activity_scroll,
        None,
        batten::state::Tone::Info,
        app,
        panes,
    );

    let mut mcp_lines = Vec::new();
    if let Some(capabilities) = &app.runtime_capabilities {
        match capabilities.mcp_config {
            crate::app::McpConfigState::Unknown => {
                mcp_lines.push(Line::from("MCP state unknown"));
            }
            crate::app::McpConfigState::NotConfigured => {
                mcp_lines.push(Line::from("Not configured"));
            }
            crate::app::McpConfigState::Invalid => {
                mcp_lines.push(Line::from(Span::styled(
                    "● Configuration invalid",
                    app.theme.styles.error,
                )));
            }
            crate::app::McpConfigState::Loaded if capabilities.mcp_servers.is_empty() => {
                mcp_lines.push(Line::from("Configured · no servers"));
            }
            crate::app::McpConfigState::Loaded => {
                for server in &capabilities.mcp_servers {
                    let style = if server.initialized {
                        app.theme.styles.success
                    } else {
                        app.theme.styles.error
                    };
                    let label = if server.initialized {
                        mcp_server_summary(server)
                    } else {
                        server.id.clone()
                    };
                    mcp_lines.push(Line::from(vec![
                        Span::styled("● ", style),
                        Span::raw(label),
                    ]));
                }
                if let Some(warning) = mcp_load_warning(&capabilities.mcp_servers) {
                    mcp_lines.push(Line::from(Span::styled(warning, app.theme.styles.error)));
                }
            }
        }
    } else {
        mcp_lines.push(Line::from("Waiting for runtime snapshot"));
    }
    let mut mcp_usage =
        std::collections::BTreeMap::<(String, String), (u64, usize, f64, bool)>::new();
    for record in &app.usage_records {
        for component in record
            .components
            .iter()
            .filter(|component| component.kind == codeloom_core::session::UsageComponentKind::Mcp)
        {
            let agent = app
                .subagents
                .get(&record.session.0)
                .and_then(|child| child.agent.as_deref())
                .unwrap_or("parent")
                .to_owned();
            let entry = mcp_usage.entry((agent, component.id.clone())).or_default();
            entry.0 = entry.0.saturating_add(component.input_tokens);
            entry.1 = entry.1.saturating_add(1);
            if let Some(price) = record.input_price {
                entry.2 += price * component.input_tokens as f64 / 1_000_000.0;
                entry.3 = true;
            }
        }
    }
    let mcp_total_tokens = mcp_usage.values().map(|usage| usage.0).sum::<u64>();
    let mcp_summary = if mcp_usage.is_empty() {
        "—".to_owned()
    } else {
        format!("~{mcp_total_tokens} tok")
    };
    for ((agent, server), (tokens, requests, cost, priced)) in mcp_usage {
        let per_request = tokens / requests.max(1) as u64;
        let estimate = if priced {
            format!(" · ~${:.5}/r", cost / requests.max(1) as f64)
        } else {
            " · $?/r".to_owned()
        };
        mcp_lines.push(Line::from(format!(
            "{server}/{agent} ~{per_request} tok/r{estimate}"
        )));
    }
    draw_scrolled_panel(
        frame,
        mcp,
        "MCP",
        Focus::Mcp,
        &mcp_lines,
        app.capability_scroll[0],
        Some(&mcp_summary),
        batten::state::Tone::Neutral,
        app,
        panes,
    );

    let mut lsp_lines = if let Some(capabilities) = &app.runtime_capabilities {
        let available = capabilities.rust_analyzer_available;
        vec![Line::from(vec![
            Span::styled(
                "● ",
                if available {
                    app.theme.styles.success
                } else {
                    app.theme.styles.error
                },
            ),
            Span::raw(if available {
                "rust-analyzer found"
            } else {
                "rust-analyzer missing"
            }),
        ])]
    } else {
        vec![Line::from("Waiting for runtime snapshot")]
    };
    lsp_lines.push(Line::from(if app.lsp_invoked {
        "Query called this session"
    } else {
        "Query not called this session"
    }));
    draw_scrolled_panel(
        frame,
        lsp,
        "LSP",
        Focus::Lsp,
        &lsp_lines,
        app.capability_scroll[1],
        None,
        batten::state::Tone::Neutral,
        app,
        panes,
    );

    let mut skill_lines = Vec::new();
    let mut skill_summary = "—".to_owned();
    if let Some(capabilities) = &app.runtime_capabilities {
        if app.skill_statuses_by_child.is_empty() {
            for (agent, skill) in capabilities.declared_skill_scopes.iter().take(4) {
                skill_lines.push(Line::from(format!("◌ {agent}/{skill} · pending")));
            }
            if skill_lines.is_empty() {
                skill_lines.push(Line::from("No local skills assigned"));
            }
        } else {
            for (child, loaded_skills) in &app.skill_statuses_by_child {
                let agent = app
                    .subagents
                    .get(child)
                    .and_then(|child| child.agent.as_deref())
                    .unwrap_or("local agent");
                for skill in loaded_skills {
                    let (label, style) = match skill.state {
                        codeloom_core::skills::SkillLoadState::Loaded => {
                            ("loaded", app.theme.styles.success)
                        }
                        codeloom_core::skills::SkillLoadState::Missing => {
                            ("missing", app.theme.styles.error)
                        }
                        codeloom_core::skills::SkillLoadState::Duplicate => {
                            ("duplicate", app.theme.styles.warning)
                        }
                        _ => ("error", app.theme.styles.error),
                    };
                    skill_lines.push(Line::from(vec![
                        Span::styled("● ", style),
                        Span::raw(format!(
                            "{agent}/{} · {label} ~{} tok/r",
                            skill.id, skill.input_tokens_per_request
                        )),
                    ]));
                }
            }
        }
        let mut skill_usage =
            std::collections::BTreeMap::<(String, String), (u64, usize, f64, bool)>::new();
        for record in &app.usage_records {
            for component in record.components.iter().filter(|component| {
                component.kind == codeloom_core::session::UsageComponentKind::Skill
            }) {
                let agent = app
                    .subagents
                    .get(&record.session.0)
                    .and_then(|child| child.agent.as_deref())
                    .unwrap_or("parent")
                    .to_owned();
                let entry = skill_usage
                    .entry((agent, component.id.clone()))
                    .or_default();
                entry.0 = entry.0.saturating_add(component.input_tokens);
                entry.1 = entry.1.saturating_add(1);
                if let Some(price) = record.input_price {
                    entry.2 += price * component.input_tokens as f64 / 1_000_000.0;
                    entry.3 = true;
                }
            }
        }
        let skill_total_tokens = skill_usage.values().map(|usage| usage.0).sum::<u64>();
        if !skill_usage.is_empty() {
            skill_summary = format!("~{skill_total_tokens} tok");
        }
        for ((agent, skill), (tokens, requests, cost, priced)) in skill_usage {
            let per_request = tokens / requests.max(1) as u64;
            let estimate = if priced {
                format!(" · ~${:.5}/r", cost / requests.max(1) as f64)
            } else {
                " · $?/r".to_owned()
            };
            skill_lines.push(Line::from(format!(
                "{skill}/{agent} ~{per_request} tok/r{estimate}"
            )));
        }
    } else {
        skill_lines.push(Line::from("Waiting for runtime snapshot"));
    }
    draw_scrolled_panel(
        frame,
        skills,
        "Skills",
        Focus::Skills,
        &skill_lines,
        app.capability_scroll[2],
        Some(&skill_summary),
        batten::state::Tone::Neutral,
        app,
        panes,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_scrolled_panel(
    frame: &mut Frame,
    area: Rect,
    title: &'static str,
    focus: Focus,
    lines: &[Line<'static>],
    requested_scroll: u16,
    summary: Option<&str>,
    tone: batten::state::Tone,
    app: &App,
    panes: &mut Vec<(Focus, Rect)>,
) {
    panes.push((focus, area));
    let visible = area.height.saturating_sub(2).max(1);
    let max_scroll = lines
        .len()
        .saturating_sub(usize::from(visible))
        .min(usize::from(u16::MAX)) as u16;
    let scroll = requested_scroll.min(max_scroll);
    let end = (usize::from(scroll) + usize::from(visible)).min(lines.len());
    let scroll_info = if lines.is_empty() {
        "empty".to_owned()
    } else {
        format!("{}-{end}/{} ↕", usize::from(scroll) + 1, lines.len())
    };
    let info = summary
        .map(|summary| format!("{summary} ↕"))
        .unwrap_or(scroll_info);
    let inner = batten::widgets::panel::render(
        frame,
        area,
        &batten::widgets::Panel {
            title,
            info: &info,
            tone,
        },
        &app.theme,
    );
    frame.render_widget(Paragraph::new(lines.to_vec()).scroll((scroll, 0)), inner);
}

pub(super) fn draw_workspace_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    panes: &mut Vec<(Focus, Rect)>,
) {
    panes.push((Focus::Sidebar, area));
    let changes_height = if app.changes.is_empty() {
        0
    } else {
        (app.changes.len() as u16 + 2).min(area.height.saturating_sub(1))
    };
    let [sidebar, changes] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(changes_height)]).areas(area);
    batten::widgets::sidebar::render(frame, sidebar, &app.sidebar(), &app.theme);
    if changes_height > 0 {
        batten::widgets::file_changes::render(
            frame,
            changes,
            "ficheros modificados",
            &app.changes,
            &app.theme,
        );
    }
}

pub(super) fn draw_resize_hint(frame: &mut Frame, area: Rect, title: &str, app: &App) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(format!("{title}: enlarge terminal · Esc close"))
            .style(app.theme.styles.warning)
            .wrap(Wrap { trim: true }),
        area,
    );
}
