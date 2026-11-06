//! Cabecera y pie de la interfaz: resumen de capacidades, cabeceras del espacio y plana, pie con medidor.

use super::*;

pub(super) fn job_counts(app: &App) -> (usize, usize, usize, usize) {
    let (running, finished) = app
        .jobs
        .values()
        .filter(|job| Some(job.owner) == app.session_id && job.kind != "subagent")
        .fold((0, 0), |(running, finished), job| {
            let active = matches!(
                job.status,
                loomcoding_core::job::JobStatus::Running
                    | loomcoding_core::job::JobStatus::Stopping
            );
            (
                running + usize::from(active),
                finished + usize::from(!active),
            )
        });
    let (subagents_running, subagents_finished) = app
        .subagents
        .values()
        .filter(|subagent| Some(subagent.parent) == app.session_id)
        .fold((0, 0), |(running, finished), subagent| {
            if subagent.status.state.is_terminal() {
                (running, finished + 1)
            } else {
                (running + 1, finished)
            }
        });
    (running, finished, subagents_running, subagents_finished)
}

/// Altura de la cabecera de espacio de trabajo: dos líneas, o tres si hay subagentes que nombrar.
pub(super) fn workspace_header_height(app: &App) -> u16 {
    2 + u16::from(!app.subagent_state_counts().is_empty())
        + u16::from(app.runtime_capabilities.is_some())
}

/// Textos de MCP, LSP y skills, en ese orden. Los comparten la cabecera del espacio de trabajo
/// y el rail, así que ambos dicen lo mismo con las mismas palabras.
pub(crate) fn capabilities_parts(app: &App) -> Option<[String; 3]> {
    let capabilities = app.runtime_capabilities.as_ref()?;
    let mcp = match capabilities.mcp_config {
        crate::app::McpConfigState::Unknown => "MCP pending".to_owned(),
        crate::app::McpConfigState::NotConfigured => "MCP not configured".to_owned(),
        crate::app::McpConfigState::Invalid => "MCP config invalid".to_owned(),
        crate::app::McpConfigState::Loaded => {
            let initialized = capabilities
                .mcp_servers
                .iter()
                .filter(|server| server.initialized)
                .count();
            format!(
                "MCP {initialized}/{} initialized",
                capabilities.mcp_servers.len()
            )
        }
    };
    let lsp = match (
        capabilities.lsp_registered,
        capabilities.rust_analyzer_available,
    ) {
        (false, _) => "LSP unavailable".to_owned(),
        (true, false) => "LSP rust-analyzer missing".to_owned(),
        (true, true) => "LSP rust-analyzer found".to_owned(),
    };
    let skills = format!(
        "skills {}/{} cards, {} scopes{}",
        capabilities.available_skill_total,
        capabilities.announced_skill_total,
        capabilities.declared_skill_scope_total,
        if capabilities.local_skill_loader_available {
            ""
        } else {
            " · SKILL.md unavailable"
        }
    );
    Some([mcp, lsp, skills])
}

/// Línea de capacidades de la cabecera del espacio de trabajo.
pub(super) fn capabilities_summary(app: &App) -> Option<Line<'static>> {
    let [mcp, lsp, skills] = capabilities_parts(app)?;
    Some(Line::from(Span::styled(
        format!("   {mcp}  ·  {lsp}  ·  {skills}"),
        app.theme.styles.muted,
    )))
}

pub(super) fn draw_workspace_header(frame: &mut Frame, area: Rect, app: &App) {
    let (state, tone) = if app.status == "working" {
        ("WORKING", loomcoding_cloth::state::Tone::Success)
    } else if app.status.starts_with("error:") {
        ("ERROR", loomcoding_cloth::state::Tone::Error)
    } else {
        ("READY", loomcoding_cloth::state::Tone::Neutral)
    };
    let mut text = vec![
        Span::styled(
            " CODEL∞M / WORKSPACE  ",
            app.theme.styles.heading.add_modifier(Modifier::BOLD),
        ),
        loomcoding_cloth::widgets::chip(state, tone, &app.theme),
        Span::styled(format!("  {}", app.model), app.theme.styles.muted),
    ];
    if app.queued > 0 {
        text.push(Span::styled(
            format!("  ·  {} queued", app.queued),
            app.theme.styles.muted,
        ));
    }
    if app.queue_paused {
        text.push(Span::styled("  ·  queue paused", app.theme.styles.warning));
    }
    let (running, finished, subagents, subagents_finished) = job_counts(app);
    if running > 0 || finished > 0 || subagents > 0 || subagents_finished > 0 {
        // El desglose de los subagentes va en su propia línea (ver más abajo): aquí solo los jobs
        // y el total de «finished», que incluye los terminados de los subagentes.
        text.push(Span::styled(
            format!(
                "  ·  {running} jobs · {} finished",
                finished + subagents_finished
            ),
            app.theme.styles.muted,
        ));
    }
    let mut lineas = vec![Line::from(text)];
    // Regla de trazabilidad: el resumen de subagentes nombra sus estados de A2A exactos. Va en
    // una línea propia, así no se recorta a 100 columnas; sin subagentes no existe esa línea.
    if !app.subagent_state_counts().is_empty() {
        lineas.push(Line::from(Span::styled(
            format!("  {}{}", app.running_summary(), app.finished_summary()),
            app.theme.styles.muted,
        )));
    }
    frame.render_widget(
        Paragraph::new({
            lineas.push(Line::from(Span::styled(
                "   conversation · activity · context · changed files",
                app.theme.styles.muted,
            )));
            if let Some(capabilities) = capabilities_summary(app) {
                lineas.push(capabilities);
            }
            lineas
        }),
        area,
    );
}

/// Sufijo de la línea de estado mientras corre el resumen anticipado: `  · resumiendo… N s`.
/// Vacío si no corre. Va tras el estado, así que si no cabe se recorta como el resto de la línea.
pub(super) fn background_suffix(app: &App) -> String {
    app.background_summary_label()
        .map_or_else(String::new, |label| format!("  · {label}"))
}

/// Cabecera de una línea del chrome `Flat`: marca, modelo, esfuerzo (si lo hay), estado y, solo
/// cuando hay dato, la cola y los recuentos. Cabe señalar que el directorio y las capacidades no
/// van aquí: el rail ya las muestra, y la cabecera se queda en una fila.
pub(super) fn draw_flat_header(frame: &mut Frame, area: Rect, app: &App) {
    let status_style = if app.status.starts_with("error:") {
        app.theme.styles.error
    } else {
        app.theme.styles.muted
    };
    let mut spans = vec![
        Span::styled(
            " loom",
            app.theme.styles.heading.add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {}", app.model), app.theme.styles.muted),
    ];
    if let Some(effort) = app
        .provider_settings
        .as_ref()
        .and_then(|settings| settings.effort.as_deref())
        .filter(|effort| !effort.is_empty())
    {
        spans.push(Span::styled(format!(" · {effort}"), app.theme.styles.muted));
    }
    spans.push(Span::styled(
        format!(" · {}{}", app.status, background_suffix(app)),
        status_style,
    ));
    if app.queued > 0 {
        spans.push(Span::styled(
            format!(" · {} queued", app.queued),
            app.theme.styles.muted,
        ));
    }
    if app.queue_paused {
        spans.push(Span::styled(" · queue paused", app.theme.styles.warning));
    }
    let (running, finished, subagents, subagents_finished) = job_counts(app);
    if running > 0 || finished > 0 || subagents > 0 || subagents_finished > 0 {
        spans.push(Span::styled(
            format!(" · {} · {running} jobs", app.running_summary()),
            app.theme.styles.muted,
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Atajos del pie del chrome `Flat`, en el orden en que se muestran. Son cuatro; el resto vive
/// en la ayuda `?`. Cabe señalar que el tercero nombra lo que `ctrl+b` pliega: la tarjeta de
/// agentes en `Cards`, y el panel lateral en las demás disposiciones.
pub(super) fn flat_hints(layout: LayoutStyle) -> [(&'static str, &'static str); 4] {
    let plegable = if layout == LayoutStyle::Cards {
        "card"
    } else {
        "panel"
    };
    [
        ("enter", "send"),
        ("ctrl+o", "fold"),
        ("ctrl+b", plegable),
        ("?", "shortcuts"),
    ]
}

/// Ancho mínimo del pie para reservar a su derecha el medidor de contexto.
pub(super) const METER_MIN_FOOTER_WIDTH: u16 = 80;

/// Reserva a la derecha del pie el medidor de contexto, si hay medidor que pintar y el pie tiene
/// al menos `METER_MIN_FOOTER_WIDTH` columnas. Devuelve el área que queda para el texto del pie;
/// sin medidor, o en un pie más estrecho, devuelve el área entera.
pub(super) fn split_footer_meter(frame: &mut Frame, area: Rect, app: &App) -> Rect {
    if area.width < METER_MIN_FOOTER_WIDTH {
        return area;
    }
    let meter = app.context_meter();
    let width = context_meter::meter_width(&meter, MeterStyle::Segments);
    if width == 0 {
        return area;
    }
    let [left, right] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(width.min(area.width)),
    ])
    .areas(area);
    context_meter::render(frame, right, &meter, MeterStyle::Segments, &app.theme);
    left
}

/// Indicador de actividad del proveedor: braille de `dotmax` si está disponible,
/// glifo propio de `cloth` como respaldo.
pub(super) fn provider_activity_span(app: &App, width: u16) -> ratatui::text::Span<'static> {
    // `frame` del `Anim` que avanza el bucle: es la fuente de tiempo de la UI.
    let frame = app.animation.frame;
    loomcoding_cloth::widgets::activity::activity_glyph(
        loomcoding_cloth::widgets::activity::ACTIVITY_STYLES[0],
        frame,
        width,
        &app.theme,
    )
    .unwrap_or_else(|| {
        app.animation
            .glyph(loomcoding_cloth::widgets::spinner::DOT_RING, &app.theme)
    })
}

/// Pie del chrome `Flat`: el feedback de slash si lo hay; si no, atajos cortos.
/// Si hay peticiones de provider en vuelo y verbose_provider_notices=false, muestra un spinner.
pub(super) fn draw_flat_footer(frame: &mut Frame, area: Rect, app: &App) {
    let area = split_footer_meter(frame, area, app);
    let line = if let Some(feedback) = app.slash_feedback.as_deref() {
        Line::from(Span::styled(feedback, app.theme.styles.muted))
    } else {
        let hints = flat_hints(app.theme.look.layout);
        let mut spans = Vec::with_capacity(hints.len() * 3);
        for (index, (key, label)) in hints.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled("  ", app.theme.styles.muted));
            }
            spans.push(Span::styled(*key, app.theme.styles.heading));
            spans.push(Span::styled(format!(" {label}"), app.theme.styles.muted));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(line), area);
}
