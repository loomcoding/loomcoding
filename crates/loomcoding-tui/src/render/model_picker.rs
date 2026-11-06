//! Selector rápido de `/model`: una lista centrada sobre la pantalla, con el filtro arriba.
//! Usa las mismas filas (`picker_items`) y el mismo estilo que el selector de `/settings`, pero
//! se dibuja fuera de su ventana, porque `/model` no abre el formulario.

use super::settings::PICKER_VISIBLE_ROWS;
use super::*;

/// Ancho máximo del recuadro: lo bastante para el identificador, la etiqueta y el resumen.
const MAX_WIDTH: u16 = 72;

/// Dibuja el selector rápido centrado en `area`. Con menos sitio del mínimo no dibuja nada.
pub(super) fn draw_model_picker(frame: &mut Frame, area: Rect, app: &App) {
    let items = app.picker_items();
    let rows = items.len().clamp(1, PICKER_VISIBLE_ROWS) as u16;
    // Bordes (2) y línea de filtro (1); el recuadro nunca sale del área.
    let height = (rows + 3).min(area.height);
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
        .title(" Model ")
        .title_bottom(Line::styled(
            "Enter apply · Esc cancel",
            app.theme.styles.muted,
        ))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = vec![Line::from(vec![
        Span::styled("Filter: ", app.theme.styles.muted),
        Span::styled(app.picker_query.clone(), app.theme.styles.default),
    ])];
    if items.is_empty() {
        lines.push(Line::styled(
            "No matches: Enter uses the typed id",
            app.theme.styles.muted,
        ));
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
