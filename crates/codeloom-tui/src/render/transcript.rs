//! Transcript y su canal de árbol: glifos, estilo y pintado de la conversación.

use super::*;

/// Columnas del carril de árbol a la izquierda del texto: guía, filete y un hueco.
pub(super) const TREE_GUTTER: u16 = 3;

/// Ancho mínimo del transcript para reservar el carril; por debajo, el texto manda.
pub(super) const TREE_MIN_WIDTH: u16 = 40;

/// Columnas que se reservan al carril de árbol en un transcript de `width` columnas.
pub(super) fn tree_gutter(width: u16) -> u16 {
    if width >= TREE_MIN_WIDTH {
        TREE_GUTTER
    } else {
        0
    }
}

/// Glifo de la cabecera de un bloque según el rol de su primera línea. Las herramientas, los
/// subagentes y la actividad no llevan: su texto ya abre con un icono propio del color de su estado.
pub(super) fn tree_glyph(kind: batten::state::LineKind) -> Option<char> {
    use batten::state::LineKind;
    match kind {
        LineKind::User => Some('»'),
        LineKind::Assistant => Some('≋'),
        LineKind::Thinking => Some('⋯'),
        LineKind::System => Some('◌'),
        LineKind::Error => Some('✕'),
        LineKind::Todo => Some('☐'),
        LineKind::Plan => Some('◊'),
        LineKind::Banner => Some('✦'),
        _ => None,
    }
}

/// Estilo de la guía de un bloque según el rol de su primera línea: el acento para quien
/// escribe y un gris tenue para el resto, de modo que los turnos se vean de un vistazo.
pub(super) fn tree_style(kind: batten::state::LineKind, app: &App) -> Style {
    match kind {
        batten::state::LineKind::User => app.theme.styles.accent,
        _ => app.theme.styles.muted,
    }
}

/// Pinta cada bloque del transcript (mensaje, herramienta, subagente) como su propia raíz en el
/// carril de las `TREE_GUTTER` columnas a la izquierda de `texto`: `╭─` en su primera fila, `│`
/// mientras dura su cuerpo y `╰─` en la última, o `──` si ocupa una sola. Entre bloques no hay
/// conexión. El bloque activo (transcript enfocado) usa el trazo grueso `┏━ ┃ ┗━` con su color
/// de rol, de modo que se ve entero de un vistazo. Al vivir fuera del área de texto, el ajuste de
/// línea no lo afecta. Recorre solo las entradas visibles y parchea celdas: no re-renderiza nada.
pub(super) fn draw_tree_gutter(frame: &mut Frame, texto: Rect, app: &App) {
    let Some(x0) = texto.x.checked_sub(TREE_GUTTER) else {
        return;
    };
    let top = app.viewport.top();
    let bottom = top + usize::from(texto.height);
    let Some((line, _)) = app.layout.line_at_row(top) else {
        return;
    };
    let Some((first, _)) = app.transcript.locate(line) else {
        return;
    };
    let activo = (app.focus == Focus::Transcript)
        .then(|| app.block_index())
        .flatten();
    let buf = frame.buffer_mut();
    for entry in first..app.transcript.entry_count() {
        let first_line = app.transcript.entry_start(entry);
        let start = app.layout.row_of(first_line);
        if start >= bottom {
            break;
        }
        let lines = app.transcript.entry_lines(entry);
        let end = app.layout.row_of(first_line + lines.len());
        let Some(kind) = lines.first().map(|line| line.kind) else {
            continue;
        };
        let grueso = activo == Some(entry);
        let glifo = tree_glyph(kind);
        let style = if grueso {
            active_marker_style(kind, app)
        } else {
            tree_style(kind, app)
        };
        for row in start.max(top)..end.min(bottom) {
            let Ok(dy) = u16::try_from(row - top) else {
                break;
            };
            // La cabecera lleva el glifo del rol en la segunda columna, en lugar del trazo.
            let cabecera = glifo.unwrap_or(if grueso { '━' } else { '─' });
            let cabecera = |esquina: char| format!("{esquina}{cabecera}");
            let guia = match (row == start, row + 1 == end, grueso) {
                (true, true, false) => cabecera('─'),
                (true, true, true) => cabecera('━'),
                (true, false, false) => cabecera('╭'),
                (true, false, true) => cabecera('┏'),
                (false, true, false) => "╰─".to_string(),
                (false, true, true) => "┗━".to_string(),
                (false, false, false) => "│".to_string(),
                (false, false, true) => "┃".to_string(),
            };
            for (i, simbolo) in guia.chars().enumerate() {
                if let Some(cell) = buf.cell_mut((x0 + i as u16, texto.y + dy)) {
                    cell.set_symbol(simbolo.encode_utf8(&mut [0; 4]))
                        .set_style(style);
                }
            }
        }
    }
}

/// Herramienta que ocupa la tarjeta fija sobre el composer (look `Pinned`). La tarjeta cuenta qué
/// hace el agente ahora: se muestra mientras dura el turno, o mientras la herramienta siga viva
/// (pendiente, en curso o esperando permiso), y desaparece al acabar, cuando ya solo repetiría lo
/// que la conversación tiene escrito. Quitarla una vez por turno, y no en cada herramienta, evita
/// que el composer salte de sitio a cada llamada.
pub(super) fn pinned_tool(app: &App) -> Option<&batten::state::ToolItem> {
    use batten::state::ToolStatus;
    if app.theme.look.tool != batten::widgets::ToolStyle::Pinned {
        return None;
    }
    let tool = app.tools.back()?;
    let viva = matches!(
        tool.status(),
        ToolStatus::Pending | ToolStatus::Running | ToolStatus::AwaitingPermission
    );
    (app.turn_in_flight || app.wake_in_flight || viva).then_some(tool)
}

pub(super) fn draw_transcript(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    panes: &mut Vec<(Focus, Rect)>,
) {
    panes.push((Focus::Transcript, area));
    app.transcript_area = None;
    // El chrome `Flat` no usa caja: el chat ocupa el área con un margen de una columna.
    let flat = app.theme.look.chrome == ChromeStyle::Flat;
    let inner = if flat {
        Rect::new(
            area.x.saturating_add(1),
            area.y,
            area.width.saturating_sub(2),
            area.height,
        )
    } else {
        batten::widgets::panel::render(
            frame,
            area,
            &batten::widgets::Panel {
                title: "Conversation",
                info: &app.status,
                tone: if app.status == "working" {
                    batten::state::Tone::Info
                } else {
                    batten::state::Tone::Neutral
                },
            },
            &app.theme,
        )
    };
    if inner.is_empty() {
        return;
    }
    // Con el look `Pinned` la última herramienta ocupa una tarjeta fija; con
    // `Inline` aparece dentro del transcript y no reserva filas aquí.
    let thinking_rows = u16::from(app.thinking);
    let max_tool_rows = inner.height.saturating_sub(4 + thinking_rows);
    let tool_rows = pinned_tool(app).map_or(0, |tool| {
        let args = usize::from(tool.summary.as_deref().is_some_and(|s| !s.is_empty()));
        let output_lines = tool
            .output
            .as_deref()
            .map_or(0, |output| output.lines().count());
        let visible_output = if tool.expanded {
            output_lines
        } else {
            output_lines.min(2) + usize::from(output_lines > 2)
        };
        let waiting = usize::from(tool.output.is_none());
        (1 + args + visible_output + waiting).min(usize::from(max_tool_rows)) as u16
    });
    let attachment_rows = u16::from(!app.draft_images.is_empty());
    let reserved_rows = 2u16
        .saturating_add(thinking_rows)
        .saturating_add(tool_rows)
        .saturating_add(attachment_rows);
    let max_composer_height = inner.height.saturating_sub(reserved_rows);
    let composer_height = batten::widgets::composer::height_for_look(
        &app.theme,
        &app.composer,
        inner.width,
        max_composer_height,
    );
    // El chrome `Flat` no muestra la cabecera de la conversación.
    let header_rows = if flat { 0 } else { 1 };
    let rows = Layout::vertical([
        Constraint::Length(header_rows),
        Constraint::Min(1),
        Constraint::Length(thinking_rows),
        Constraint::Length(tool_rows),
        Constraint::Length(attachment_rows),
        Constraint::Length(composer_height),
    ])
    .split(inner);
    app.composer_area = Some(rows[5]);
    panes.push((Focus::Composer, rows[5]));
    if !flat {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("CodeL∞M", app.theme.styles.heading),
                Span::styled("  ·  project conversation", app.theme.styles.muted),
                Span::styled(
                    format!("  ·  {}", app.theme_preset.name()),
                    app.theme.styles.muted,
                ),
            ])),
            rows[0],
        );
    }
    // El carril de árbol ocupa el borde izquierdo; el texto (y su selección) va a su derecha.
    let gutter = tree_gutter(rows[1].width);
    let texto = Rect::new(
        rows[1].x + gutter,
        rows[1].y,
        rows[1].width - gutter,
        rows[1].height,
    );
    app.transcript_area = Some(texto);
    batten::widgets::transcript::render(
        frame,
        texto,
        &app.transcript,
        &mut app.viewport,
        &mut app.layout,
        app.selection.as_ref(),
        &app.theme,
    );
    // Una selección de texto deja de valer si el diseño cambió de ancho o si ya no
    // cabe en el transcript (p. ej. tras una sesión más corta).
    let stale = app.text_sel.is_some_and(|sel| {
        app.text_sel_width != app.layout.width() || sel.ordered().0.row >= app.layout.total_rows()
    });
    if stale {
        app.text_sel = None;
    }
    if let Some(sel) = app.text_sel.as_ref() {
        batten::widgets::transcript::paint_text_selection(
            frame.buffer_mut(),
            texto,
            &app.layout,
            &app.viewport,
            sel,
            app.theme.styles.selection,
        );
    }
    draw_tree_gutter(frame, texto, app);
    if app.thinking {
        let line = app.reasoning.lines().last().unwrap_or("Thinking…");
        batten::widgets::thinking::render(frame, rows[2], line, &app.animation, &app.theme);
    }
    if let Some(tool) = pinned_tool(app) {
        batten::widgets::tool_item::render_crush(frame, rows[3], tool, &app.theme);
    }
    if attachment_rows > 0 {
        let chip = Style::default()
            .fg(ratatui::style::Color::Black)
            .bg(ratatui::style::Color::LightCyan)
            .add_modifier(Modifier::BOLD);
        let mut spans = vec![Span::styled(" Attachments ", app.theme.styles.muted)];
        for image in &app.draft_images {
            if let codeloom_core::message::ContentPart::Image { filename, .. } = image {
                spans.push(Span::styled(format!(" ▧ {filename} "), chip));
            }
        }
        spans.push(Span::styled(
            "  Backspace removes last",
            app.theme.styles.muted,
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), rows[4]);
    }
    // El composer anima su propio indicador: le pasamos el fotograma actual.
    app.composer.set_activity_frame(app.animation.frame);
    // Con un hijo como destinatario, el composer nombra a quién va lo que se escribe.
    let model_label: std::borrow::Cow<str> = match app.composer_target_label() {
        Some(target) => format!("{target} · {}", app.model).into(),
        None => std::borrow::Cow::Borrowed(&app.model),
    };
    batten::widgets::composer::render_for_look(
        frame,
        rows[5],
        &app.composer,
        app.focus == crate::app::Focus::Composer
            && app.permission.is_none()
            && app.question.is_none()
            && !app.session_picker_open
            && !app.session_loading
            && !app.queue_open
            && !app.settings_open
            && !app.help_open
            && !app.agents_open
            && !app.tools_open,
        &model_label,
        &app.theme,
    );
    if app.slash_query().is_some() {
        let matches = app.slash_matches();
        let commands = matches
            .iter()
            .map(
                |(_, usage, description)| batten::widgets::command_suggestions::MarttyCommand {
                    usage,
                    description,
                    section: None,
                    skill: false,
                },
            )
            .collect::<Vec<_>>();
        let chat = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            rows[4].y.saturating_sub(inner.y),
        );
        batten::widgets::command_suggestions::render_martty(
            frame,
            rows[4],
            chat,
            &commands,
            app.slash_selected,
            &app.theme,
        );
    }
}
