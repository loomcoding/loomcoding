use crate::app::{AgentsFocus, App, Focus, SettingsPicker, SpinnerChoice, SPINNER_CHOICES};
use crate::rail;
use loomcoding_cloth::widgets::context_meter::{self, MeterStyle};
use loomcoding_cloth::widgets::{ChromeStyle, LayoutStyle};
use loomcoding_core::event::{A2aDirection, A2aOperation};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub(crate) mod chrome;
use chrome::*;

mod transcript;
use transcript::*;

mod mascot_strip;

mod agents_card;

mod sidebar;
use sidebar::*;

mod sidebar_diff;

mod logo_fx;

mod settings;
use settings::*;

mod ui_root;

mod overlays;
use overlays::*;

mod a2a;
use a2a::*;

mod marks;
use marks::*;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    app.matrix_rain.render(frame, area, &app.theme);
    // El mapa de ventanas reutiliza el mismo `Vec` en cada fotograma.
    let mut panes = std::mem::take(&mut app.pane_rects);
    panes.clear();
    app.composer_area = None;
    let flat = app.theme.look.chrome == ChromeStyle::Flat;
    let rows = Layout::vertical([
        // La cabecera plana es siempre una fila; la de espacio de trabajo calcula la suya.
        Constraint::Length(if flat {
            1
        } else {
            workspace_header_height(app)
        }),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    if flat {
        draw_flat_header(frame, rows[0], app);
    } else {
        draw_workspace_header(frame, rows[0], app);
    }
    if matches!(
        app.theme.look.layout,
        LayoutStyle::Minimal | LayoutStyle::Cards
    ) {
        // Solo el transcript, a todo el ancho: sin rail, sidebar ni métricas.
        draw_transcript(frame, rows[1], app, &mut panes);
    } else if app.theme.look.layout == LayoutStyle::ContextRail {
        // Sin sidebar izquierdo ni métricas: el chat y, si hay ancho, el rail derecho.
        if rows[1].width >= 80 && !app.rail_hidden {
            let [chat, context] =
                Layout::horizontal([Constraint::Min(1), Constraint::Length(32)]).areas(rows[1]);
            draw_transcript(frame, chat, app, &mut panes);
            rail::draw_context_rail(frame, context, app);
            panes.push((Focus::Rail, context));
        } else {
            draw_transcript(frame, rows[1], app, &mut panes);
        }
    } else if rows[1].width >= 105 {
        let [left, center, right] = Layout::horizontal([
            Constraint::Length(23),
            Constraint::Min(42),
            Constraint::Length(30),
        ])
        .areas(rows[1]);
        draw_workspace_sidebar(frame, left, app, &mut panes);
        draw_transcript(frame, center, app, &mut panes);
        draw_workspace_metrics(frame, right, app, &mut panes);
    } else if rows[1].width >= 68 {
        if app.sidebar_expanded {
            let [left, center] =
                Layout::horizontal([Constraint::Length(23), Constraint::Min(1)]).areas(rows[1]);
            draw_workspace_sidebar(frame, left, app, &mut panes);
            draw_transcript(frame, center, app, &mut panes);
        } else {
            let [center, right] =
                Layout::horizontal([Constraint::Min(1), Constraint::Length(28)]).areas(rows[1]);
            draw_transcript(frame, center, app, &mut panes);
            draw_workspace_metrics(frame, right, app, &mut panes);
        }
    } else {
        draw_transcript(frame, rows[1], app, &mut panes);
    }
    mark_focused_panes(frame, &panes, app, flat);
    // Una línea por bloque en el margen; después el bloque activo, que la sobreescribe.
    app.pane_rects = panes;
    if flat {
        draw_flat_footer(frame, rows[2], app);
    } else {
        let text_area = split_footer_meter(frame, rows[2], app);
        let status_line = if app.provider_in_flight > 0 && !app.verbose_provider_notices {
            Line::from(vec![
                Span::styled(
                    format!(" {}{} ", app.status, background_suffix(app)),
                    app.theme.styles.heading,
                ),
                provider_activity_span(app, 2),
                Span::styled(" Provider working… ", app.theme.styles.muted),
                Span::styled(app.model.clone(), app.theme.styles.heading),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    format!(" {}{} ", app.status, background_suffix(app)),
                    app.theme.styles.heading,
                ),
                Span::styled(
                    app.slash_feedback.as_deref().unwrap_or(
                        "  Enter send · Ctrl+J newline · Ctrl+K queue · Ctrl+O tool · Ctrl+T inspect · F2 settings · Esc cancel",
                    ),
                    app.theme.styles.muted,
                ),
                Span::styled(
                    format!("  ↑ {} · ↓ {} tokens", app.input_tokens, app.output_tokens),
                    app.theme.styles.muted,
                ),
            ])
        };
        frame.render_widget(Paragraph::new(status_line), text_area);
    }

    // Las preguntas se pintan antes que los permisos: un permiso queda por encima.
    if let Some(q) = &app.question {
        loomcoding_cloth::widgets::question::render(frame, area, &q.dialog, &app.theme);
    }
    if let Some(p) = &app.permission {
        loomcoding_cloth::widgets::permission::render(frame, area, &p.dialog, &app.theme);
    }
    if app.session_picker_open {
        draw_session_picker(frame, area, app);
    } else if app.queue_open {
        draw_prompt_queue(frame, area, app);
    } else if app.session_loading {
        let width = area.width.min(24);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + area.height / 2,
            width,
            u16::from(area.height > 0),
        );
        frame.render_widget(
            Paragraph::new("Loading session…").alignment(Alignment::Center),
            popup,
        );
    }
    if app.settings_open {
        draw_provider_settings(frame, area, app);
    }
    if app.temas_open {
        draw_temas(frame, area, app);
    }
    if app.help_open {
        draw_help_overlay(frame, area, app);
    }
    if app.agents_open {
        draw_agents_overlay(frame, area, app);
    }
    if app.a2a_open {
        draw_a2a_overlay(frame, area, app);
    }
    if app.tools_open {
        draw_tool_inspector(frame, area, app);
    }
    if app.slash_query().is_some() {
        frame.render_widget(
            Paragraph::new(if let Some(feedback) = app.slash_feedback.as_deref() {
                feedback
            } else if app.slash_matches().is_empty() {
                "Unknown command · Esc to send as text"
            } else {
                "↑/↓ select · Tab complete · Enter run · Esc dismiss"
            })
            .style(app.theme.styles.muted),
            rows[2],
        );
    }
}

#[cfg(test)]
mod tests;
