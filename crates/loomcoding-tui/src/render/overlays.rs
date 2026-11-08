//! Superposiciones sueltas: cola de prompts, inspector de herramientas y ayuda.

use super::ayuda::draw_help_page;
use super::*;
use crate::help_topics::HelpView;

pub(super) fn draw_prompt_queue(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(78).saturating_sub(2);
    let height = (app
        .pending_prompts
        .len()
        .saturating_add(10)
        .max(11)
        .min(usize::from(area.height.saturating_sub(2)))) as u16;
    if width < 62 || height < 11 {
        draw_resize_hint(frame, area, "Prompt queue", app);
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
        .title(format!(
            " Prompt queue ({}) · {} ",
            app.pending_prompts.len(),
            if app.queue_paused {
                "paused"
            } else {
                "resumes on close"
            },
        ))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let [list, preview, footer] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(5),
        Constraint::Length(3),
    ])
    .areas(inner);
    let mut lines = app
        .pending_prompts
        .iter()
        .enumerate()
        .map(|(index, prompt)| {
            let prefix = if index == app.queue_selected {
                "› "
            } else {
                "  "
            };
            let text = prompt.text.lines().next().unwrap_or("");
            let clipped = text
                .chars()
                .take(list.width.saturating_sub(3) as usize)
                .collect::<String>();
            Line::styled(
                format!("{prefix}{clipped}"),
                if index == app.queue_selected {
                    app.theme.styles.heading.add_modifier(Modifier::BOLD)
                } else {
                    app.theme.styles.default
                },
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(Line::styled("Queue is empty", app.theme.styles.muted));
    }
    let scroll = app
        .queue_selected
        .saturating_sub(usize::from(list.height.saturating_sub(1))) as u16;
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((scroll, 0)),
        list,
    );
    let selected = app.pending_prompts.get(app.queue_selected);
    let text = selected.map_or("Select a queued prompt", |prompt| prompt.text.as_str());
    if app.queue_editing {
        loomcoding_cloth::widgets::composer::render_focused(
            frame,
            preview,
            &app.queue_editor,
            true,
            &app.theme,
        );
    } else {
        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().title(" Preview ").borders(Borders::TOP))
                .wrap(Wrap { trim: true }),
            preview,
        );
    }
    frame.render_widget(
        Paragraph::new(if app.queue_editing {
            "Enter accept (nonempty) · Ctrl+J newline\nCtrl+A clear · Esc discard edit"
        } else {
            "↑/↓ select · Ctrl+↑/↓ move · Del remove · Enter edit\nSpace pause/resume · Ctrl+X cancel active and pause\nEsc close · Pending prompts wait while this dialog is open"
        })
            .style(app.theme.styles.muted)
            .wrap(Wrap { trim: true }),
        footer,
    );
}

pub(super) fn draw_tool_inspector(frame: &mut Frame, area: Rect, app: &mut App) {
    let width = area.width.min(100).saturating_sub(2);
    let height = area.height.saturating_sub(2);
    if width < 54 || height < 13 {
        draw_resize_hint(frame, area, "Tool inspector", app);
        return;
    }
    let popup = Rect::new(area.x + (area.width - width) / 2, area.y + 1, width, height);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Tool inspector · last 8 calls ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let [list, detail, footer] = Layout::vertical([
        Constraint::Length(app.tools.len().clamp(1, 4) as u16),
        Constraint::Min(3),
        Constraint::Length(3),
    ])
    .areas(inner);
    let rows = if app.tools.is_empty() {
        vec![Line::styled("No tool calls yet", app.theme.styles.muted)]
    } else {
        app.tools
            .iter()
            .enumerate()
            .map(|(index, tool)| {
                Line::styled(
                    format!(
                        "{} {} · {:?}",
                        if index == app.tool_selected {
                            "›"
                        } else {
                            " "
                        },
                        tool.name,
                        tool.status()
                    ),
                    if index == app.tool_selected {
                        app.theme.styles.heading
                    } else {
                        app.theme.styles.muted
                    },
                )
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(rows).scroll((
            app.tool_selected
                .saturating_sub(usize::from(list.height.saturating_sub(1))) as u16,
            0,
        )),
        list,
    );
    let mut lines = Vec::new();
    if let Some(tool) = app.tools.get(app.tool_selected) {
        lines.push(Line::styled("Arguments", app.theme.styles.heading));
        lines.extend(
            tool.summary
                .as_deref()
                .unwrap_or("No arguments received")
                .split('\n')
                .map(Line::from),
        );
        lines.push(Line::styled("Output", app.theme.styles.heading));
        lines.extend(
            tool.output
                .as_deref()
                .unwrap_or("No output received")
                .split('\n')
                .map(Line::from),
        );
    }
    let max_y = lines
        .len()
        .saturating_sub(usize::from(detail.height))
        .min(usize::from(u16::MAX)) as u16;
    let max_x = lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or(0)
        .saturating_sub(usize::from(detail.width))
        .min(usize::from(u16::MAX)) as u16;
    app.tool_scroll.0 = app.tool_scroll.0.min(max_y);
    app.tool_scroll.1 = app.tool_scroll.1.min(max_x);
    frame.render_widget(Paragraph::new(lines).scroll(app.tool_scroll), detail);
    frame.render_widget(
        Paragraph::new("↑/↓ select · PgUp/PgDn scroll · ←/→ pan\nHome/End start/end · Y copy output\nEsc or Ctrl+T close")
            .style(app.theme.styles.muted),
        footer,
    );
}

/// Ayuda con `F1`, `?` o `/help`: los atajos, o la página de un tema o de un comando (`ayuda.rs`).
pub(super) fn draw_help_overlay(frame: &mut Frame, area: Rect, app: &App) {
    match app.help_view {
        HelpView::Shortcuts => draw_shortcuts_overlay(frame, area, app),
        vista => draw_help_page(frame, area, app, vista),
    }
}

fn draw_shortcuts_overlay(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(78).saturating_sub(2);
    // Veintiuna líneas de contenido más el marco: el recuadro ocupa 23 de las 24 filas de 80×24,
    // y así no se corta ninguna. La indicación de cierre va en el borde inferior.
    let height = area.height.min(24).saturating_sub(1);
    if width < 54 || height < 20 {
        draw_resize_hint(frame, area, "Keyboard shortcuts", app);
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
        .title(" CodeL∞M · Keyboard shortcuts ")
        .title_bottom(Line::styled(
            "F1/Esc close · Tab topics",
            app.theme.styles.muted,
        ))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Enter       Send prompt · Ctrl+J inserts a newline"),
            Line::from("Tab/Shift+Tab Switch composer/transcript"),
            Line::from("↑/↓ + Enter Open subagent (rail focused)"),
            Line::from("Alt+←/parent Back to the parent session"),
            Line::from("Mouse wheel Scroll transcript"),
            Line::from("Ctrl+B      Toggle sidebar at medium width"),
            Line::from("Ctrl+K      Open prompt queue · Ctrl+T inspect recent tool calls"),
            Line::from("Ctrl+O      Expand latest tool output"),
            Line::from("Space       Fold/unfold the active long block"),
            Line::from("Ctrl+G      Send to a subagent (cycle)"),
            Line::from("/           Commands at start of composer"),
            Line::from("/ui         Interface panel: theme, style, layout, effects, spinner,"),
            Line::from("            mascot, side menu, mouse. Aliases: /theme /layout /crush"),
            Line::from("            /classic /mascot /sidebar /mouse still work as aliases"),
            Line::from("/logofx     Logo effects: on|off, or seconds between bursts"),
            Line::from("/model      /model <id> · /effort <level|default> · no arg: pick"),
            Line::from("/provider   /provider <id> · no arg: pick from the list"),
            Line::from("F2 / F3     Provider settings · switch or create session (idle)"),
            Line::from("?           Open this help (empty composer)"),
            Line::from("Esc         Cancel turn; press twice to quit"),
            Line::from("Ctrl+C/Q    Quit"),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}
