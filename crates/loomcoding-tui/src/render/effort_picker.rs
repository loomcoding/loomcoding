//! Selector de `/effort`: una lista pequeña centrada con «default» y los niveles del modelo. La
//! fila resaltada lleva `›` y el nivel vigente lleva `●`. Usa el mismo estilo que el selector
//! rápido de `/model`.

use super::settings::PICKER_VISIBLE_ROWS;
use super::*;

/// Ancho máximo del recuadro: lo bastante para el nombre del nivel y la marca.
const MAX_WIDTH: u16 = 40;

/// Dibuja el selector de esfuerzo centrado en `area`. Con menos sitio del mínimo no dibuja nada.
pub(super) fn draw_effort_picker(frame: &mut Frame, area: Rect, app: &App) {
    let Some(selected_row) = app.quick_effort_picker else {
        return;
    };
    let options = app.effort_options();
    let rows = options.len() + 1;
    let visible = rows.clamp(1, PICKER_VISIBLE_ROWS) as u16;
    // Bordes (2); el recuadro nunca sale del área.
    let height = (visible + 2).min(area.height);
    let width = area.width.min(MAX_WIDTH);
    if width < 24 || height < 3 {
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
        .title(" Effort ")
        .title_bottom(Line::styled(
            "Enter apply · Esc cancel",
            app.theme.styles.muted,
        ))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let current = app
        .provider_settings
        .as_ref()
        .and_then(|settings| settings.effort.as_deref());
    // Fila 0 es «default» (vigente cuando no hay nivel); la i + 1 es `options[i]`.
    let labels: Vec<(&str, bool)> = std::iter::once(("default", current.is_none()))
        .chain(
            options
                .iter()
                .map(|option| (option.as_str(), current == Some(option.as_str()))),
        )
        .collect();
    // El desplazamiento mantiene visible la fila resaltada.
    let selected = selected_row.min(labels.len().saturating_sub(1));
    let offset = (selected + 1).saturating_sub(PICKER_VISIBLE_ROWS);
    let lines: Vec<Line> = labels
        .iter()
        .enumerate()
        .skip(offset)
        .take(PICKER_VISIBLE_ROWS)
        .map(|(index, (label, is_current))| {
            let is_selected = index == selected;
            Line::from(vec![
                Span::styled(
                    if is_selected { "› " } else { "  " },
                    app.theme.styles.accent_from,
                ),
                Span::styled(
                    if *is_current { "● " } else { "  " },
                    app.theme.styles.accent_from,
                ),
                Span::styled(
                    (*label).to_string(),
                    if is_selected {
                        app.theme.styles.heading.add_modifier(Modifier::BOLD)
                    } else {
                        app.theme.styles.default
                    },
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
