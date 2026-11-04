//! Selectores y ajustes: sesiones, temas, proveedor y selector de ajustes.

use super::*;

pub(super) fn draw_session_picker(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(76).saturating_sub(2);
    let sessions = app.matching_sessions().collect::<Vec<_>>();
    let search_rows = u16::from(app.session_searching);
    let height = (sessions
        .len()
        .saturating_add(usize::from(search_rows))
        .saturating_add(6)
        .min(usize::from(area.height.saturating_sub(2)))) as u16;
    if width < 24 || height < 5 {
        draw_resize_hint(frame, area, "Sessions", app);
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
        .title(" Open a session ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if let Some(edit) = &app.agent_edit {
        draw_agent_editor(frame, inner, app, edit);
        return;
    }
    let [search, list, footer] = Layout::vertical([
        Constraint::Length(search_rows),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    if app.session_searching {
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!(
                    "Filter: {}_  ({} matches · Ctrl+F search · Esc clear)",
                    app.session_query,
                    sessions.len()
                ),
                app.theme.styles.muted,
            )),
            search,
        );
    }
    let mut lines = Vec::new();
    lines.extend(sessions.iter().enumerate().map(|(index, session)| {
        let selected = index == app.session_selected;
        Line::from(vec![
            Span::styled(
                if selected { "› " } else { "  " },
                app.theme.styles.accent_from,
            ),
            Span::styled(
                if app.session_id == Some(session.id) {
                    format!("{}  · active", session.preview)
                } else {
                    session.preview.clone()
                },
                if selected {
                    app.theme.styles.heading.add_modifier(Modifier::BOLD)
                } else {
                    app.theme.styles.default
                },
            ),
            Span::styled(
                format!("  · {} msgs", session.message_count),
                app.theme.styles.muted,
            ),
        ])
    }));
    let new_selected = app.session_selected == sessions.len();
    lines.push(Line::from(vec![
        Span::styled(
            if new_selected { "› " } else { "  " },
            app.theme.styles.accent_from,
        ),
        Span::styled(
            "＋ New session",
            if new_selected {
                app.theme.styles.heading.add_modifier(Modifier::BOLD)
            } else {
                app.theme.styles.default
            },
        ),
    ]));
    let scroll = app
        .session_selected
        .saturating_sub(usize::from(list.height.saturating_sub(1)))
        .min(usize::from(u16::MAX)) as u16;
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), list);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "↑/↓ or j/k select · Home/End jump · Enter open · N new · Ctrl+F filter · Esc quit",
            app.theme.styles.muted,
        )),
        footer,
    );
}

/// Ventana `/temas`: árbol a la izquierda, detalle a la derecha.
pub(super) fn draw_temas(frame: &mut Frame, area: Rect, app: &App) {
    use crate::app::{TemasLevel, MATRIX_FIELDS};

    let width = area.width.min(76).saturating_sub(2);
    let height = 16.min(area.height.saturating_sub(2));
    if width < 44 || height < 10 {
        draw_resize_hint(frame, area, "Temas", app);
        return;
    }
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let title = format!(" {} ", app.temas_level.title());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Columna izquierda: el árbol. Derecha: el detalle del nivel.
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(16)])
        .split(inner);

    let mut lines: Vec<Line> = Vec::new();
    let marker = |selected: bool| if selected { "▸ " } else { "  " };
    match app.temas_level {
        TemasLevel::Root => {
            lines.push(Line::from(Span::styled(
                format!("{}Efectos", marker(app.temas_selected == 0)),
                if app.temas_selected == 0 {
                    app.theme.styles.accent
                } else {
                    app.theme.styles.default
                },
            )));
            lines.push(Line::from(Span::styled(
                format!("{}Tema", marker(app.temas_selected == 1)),
                if app.temas_selected == 1 {
                    app.theme.styles.accent
                } else {
                    app.theme.styles.default
                },
            )));
            lines.push(Line::from(Span::styled(
                format!("{}Spinner", marker(app.temas_selected == 2)),
                if app.temas_selected == 2 {
                    app.theme.styles.accent
                } else {
                    app.theme.styles.default
                },
            )));
        }
        TemasLevel::Effects => {
            lines.push(Line::from(Span::styled(
                format!("{}Matrix rain", marker(true)),
                app.theme.styles.accent,
            )));
        }
        TemasLevel::Matrix => {
            for (index, field) in MATRIX_FIELDS.iter().enumerate() {
                let selected = index == app.temas_selected;
                lines.push(Line::from(Span::styled(
                    format!("{}{}", marker(selected), field),
                    if selected {
                        app.theme.styles.accent
                    } else {
                        app.theme.styles.default
                    },
                )));
            }
        }
        TemasLevel::Theme => {
            for (index, preset) in loomcoding_cloth::widgets::ThemePreset::ALL
                .iter()
                .enumerate()
            {
                let selected = index == app.temas_selected;
                let current = *preset == app.theme_preset;
                let label = format!(
                    "{}{}{}",
                    marker(selected),
                    preset.name(),
                    if current { "  ✓" } else { "" }
                );
                lines.push(Line::from(Span::styled(
                    label,
                    if selected {
                        app.theme.styles.accent
                    } else {
                        app.theme.styles.default
                    },
                )));
            }
        }
        TemasLevel::Spinner => {
            for (index, choice) in SPINNER_CHOICES.iter().enumerate() {
                let selected = index == app.temas_selected;
                let current = *choice == app.spinner_choice;
                let label = format!(
                    "{}{}{}",
                    marker(selected),
                    choice.label(),
                    if current { "  ✓" } else { "" }
                );
                lines.push(Line::from(Span::styled(
                    label,
                    if selected {
                        app.theme.styles.accent
                    } else {
                        app.theme.styles.default
                    },
                )));
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), split[0]);

    // Panel derecho: valores del formulario o ayuda del nivel.
    let mut detail: Vec<Line> = Vec::new();
    match app.temas_level {
        TemasLevel::Matrix => {
            let cfg = &app.temas_matrix;
            let row = |label: &str, value: String, index: usize| {
                let selected = index == app.temas_selected;
                Line::from(vec![
                    Span::styled(
                        format!("{label:<10}"),
                        if selected {
                            app.theme.styles.accent
                        } else {
                            app.theme.styles.muted
                        },
                    ),
                    Span::styled(
                        format!("‹ {value} ›"),
                        if selected {
                            app.theme.styles.accent
                        } else {
                            app.theme.styles.default
                        },
                    ),
                ])
            };
            detail.push(row("Glifos", cfg.glyphs.preset_name().to_string(), 0));
            detail.push(row("Velocidad", format!("{:.2}", cfg.speed), 1));
            detail.push(row("Densidad", cfg.density.to_string(), 2));
            detail.push(row("Longitud", cfg.length.to_string(), 3));
            detail.push(row("Color", cfg.color.key().to_string(), 4));
            detail.push(Line::from(""));
            let save_selected = app.temas_selected == MATRIX_FIELDS.len() - 1;
            detail.push(Line::from(Span::styled(
                format!(
                    "{}Guardar{}",
                    marker(save_selected),
                    if app.temas_dirty { "  •" } else { "" }
                ),
                if save_selected {
                    app.theme.styles.accent
                } else {
                    app.theme.styles.default
                },
            )));
        }
        TemasLevel::Theme => {
            detail.push(Line::from(Span::styled(
                "Enter aplica el tema al instante.",
                app.theme.styles.muted,
            )));
        }
        TemasLevel::Spinner => {
            // Vista previa del indicador elegido, animada con el frame actual.
            let span = match app.spinner_choice {
                SpinnerChoice::Crush => loomcoding_cloth::widgets::activity::crush_glyph(
                    app.animation.frame,
                    &app.theme,
                ),
                choice => loomcoding_cloth::widgets::activity::activity_glyph(
                    choice.style_for(app.animation.frame),
                    app.animation.frame,
                    2,
                    &app.theme,
                )
                .unwrap_or_else(|| {
                    app.animation
                        .glyph(loomcoding_cloth::widgets::spinner::DOT_RING, &app.theme)
                }),
            };
            detail.push(Line::from(vec![
                Span::styled("Actual  ", app.theme.styles.muted),
                span,
                Span::styled(
                    format!("  {}", app.spinner_choice.label()),
                    app.theme.styles.heading,
                ),
            ]));
            detail.push(Line::from(""));
            detail.push(Line::from(Span::styled(
                "Enter elige el indicador; se guarda en ui.toml.",
                app.theme.styles.muted,
            )));
            detail.push(Line::from(Span::styled(
                "Aleatorio cambia de estilo en cada petición.",
                app.theme.styles.muted,
            )));
            detail.push(Line::from(Span::styled(
                "El composer lo muestra mientras el agente trabaja.",
                app.theme.styles.muted,
            )));
        }
        TemasLevel::Effects => {
            detail.push(Line::from(Span::styled(
                "Enter abre la configuración de la lluvia.",
                app.theme.styles.muted,
            )));
        }
        TemasLevel::Root => {
            detail.push(Line::from(Span::styled(
                "Efectos: animaciones de fondo.",
                app.theme.styles.muted,
            )));
            detail.push(Line::from(Span::styled(
                "Tema: paleta de colores.",
                app.theme.styles.muted,
            )));
        }
    }
    detail.push(Line::from(""));
    detail.push(Line::from(Span::styled(
        "↑↓ mover · Enter entrar · ←→ ajustar · Esc volver",
        app.theme.styles.muted,
    )));
    frame.render_widget(Paragraph::new(detail), split[1]);
}

pub(super) fn draw_provider_settings(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(72).saturating_sub(2);
    let height = 12.min(area.height.saturating_sub(2));
    if width < 40 || height < 8 {
        draw_resize_hint(frame, area, "Provider settings", app);
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
        .title(" Provider settings ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let Some(settings) = &app.provider_settings else {
        return;
    };
    let kind = match settings.kind {
        loomcoding_core::config::ProviderKind::OpenAiCompat => "OpenAI compatible",
        loomcoding_core::config::ProviderKind::OpenAiResponses => "OpenAI Responses",
        loomcoding_core::config::ProviderKind::Anthropic => "Anthropic",
        loomcoding_core::config::ProviderKind::Gemini => "Gemini",
    };
    let value = |index: usize, current: &str| {
        if app.settings_editing && app.settings_field == index {
            if index == 3 {
                "•".repeat(app.settings_editor.text().chars().count())
            } else {
                app.settings_editor.text().to_string()
            }
        } else {
            current.to_string()
        }
    };
    let changes = app.settings_change_count();
    let save_label = match changes {
        0 => "Save and apply".to_string(),
        1 => "Save and apply (1 change)".to_string(),
        n => format!("Save and apply ({n} changes)"),
    };
    let mut rows: Vec<(&str, String)> = vec![
        ("Provider", kind.to_string()),
        ("Model", value(1, &settings.model)),
        ("Base URL", value(2, &settings.base_url)),
        (
            "API key",
            value(
                3,
                if app.settings_api_key_configured || !app.settings_api_key.is_empty() {
                    "••••••••"
                } else {
                    "Not configured"
                },
            ),
        ),
    ];
    // «Effort» solo aparece si el modelo admite niveles; si no, «Save» ocupa su posición.
    if app.settings_effort_visible() {
        rows.push((
            "Effort",
            settings
                .effort
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        ));
    }
    rows.push((
        save_label.as_str(),
        if app.settings_saving {
            "Applying…"
        } else {
            "Enter"
        }
        .to_string(),
    ));
    let mut lines = rows
        .iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let selected = app.settings_field == index;
            let mut spans = vec![
                Span::styled(
                    if selected { "› " } else { "  " },
                    app.theme.styles.accent_from,
                ),
                Span::styled(
                    format!("{label}: "),
                    if selected {
                        app.theme.styles.heading.add_modifier(Modifier::BOLD)
                    } else {
                        app.theme.styles.muted
                    },
                ),
                Span::styled(value.as_str(), app.theme.styles.default),
            ];
            if app.settings_field_changed(index) {
                spans.push(Span::styled(" ● unsaved", app.theme.styles.warning));
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    if let Some(feedback) = &app.settings_feedback {
        lines.push(Line::styled(feedback.clone(), app.theme.styles.warning));
    }
    lines.push(Line::styled(
        if app.settings_editing {
            "Ctrl+A clear · Enter accept · Esc cancel"
        } else if app.settings_saving {
            "Applying settings… · Esc close"
        } else if app.settings_picker.is_some() {
            "type to filter · ↑/↓ select · Enter choose · Esc close list"
        } else {
            "↑/↓ select · Enter edit/save · API key hidden · Ctrl+L catalog · Ctrl+S save · Esc close"
        },
        app.theme.styles.muted,
    ));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    if app.settings_picker.is_some() {
        draw_settings_picker(frame, popup, app);
    }
    if app.settings_editing {
        let label_width = [
            "Provider: ".len(),
            "Model: ".len(),
            "Base URL: ".len(),
            "API key: ".len(),
            "Effort: ".len(),
            "Save and apply: ".len(),
        ][app.settings_field];
        let (_, col) = app.settings_editor.cursor_row_col();
        let x = inner.x + 2 + label_width as u16 + col as u16;
        let y = inner.y + app.settings_field as u16;
        if x < inner.right() && y < inner.bottom() {
            frame.set_cursor_position((x, y));
        }
    }
}

/// Filas visibles del selector de `/settings` antes de desplazarse.
pub(super) const PICKER_VISIBLE_ROWS: usize = 8;

/// Pinta el selector de `/settings` como una lista encima del diálogo, con el filtro arriba.
pub(super) fn draw_settings_picker(frame: &mut Frame, dialog: Rect, app: &App) {
    let Some(picker) = app.settings_picker else {
        return;
    };
    let items = app.picker_items();
    let rows = items.len().clamp(1, PICKER_VISIBLE_ROWS) as u16;
    // Bordes (2) y línea de filtro (1); nunca cubre la primera fila del diálogo.
    let height = (rows + 3).min(dialog.height.saturating_sub(1));
    if height < 3 {
        return;
    }
    let area = Rect::new(
        dialog.x + 1,
        dialog.y + 1,
        dialog.width.saturating_sub(2),
        height,
    );
    frame.render_widget(Clear, area);
    let title = match picker {
        SettingsPicker::Provider => " Provider ",
        SettingsPicker::Model => " Model ",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(vec![
        Span::styled("Filter: ", app.theme.styles.muted),
        Span::styled(app.picker_query.clone(), app.theme.styles.default),
    ])];
    if items.is_empty() {
        lines.push(Line::styled("No matches", app.theme.styles.muted));
    }
    // El desplazamiento mantiene visible la fila resaltada.
    let selected = app.picker_selected.min(items.len().saturating_sub(1));
    let offset = (selected + 1).saturating_sub(PICKER_VISIBLE_ROWS);
    for (index, item) in items
        .iter()
        .enumerate()
        .skip(offset)
        .take(PICKER_VISIBLE_ROWS)
    {
        let is_selected = index == selected;
        lines.push(Line::from(vec![
            Span::styled(
                if is_selected { "› " } else { "  " },
                app.theme.styles.accent_from,
            ),
            Span::styled(
                item.label.clone(),
                if is_selected {
                    app.theme.styles.heading.add_modifier(Modifier::BOLD)
                } else {
                    app.theme.styles.default
                },
            ),
            Span::styled(format!("  {}", item.detail), app.theme.styles.muted),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
