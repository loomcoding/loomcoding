//! Selector rápido de `/provider`: una lista centrada con el filtro arriba. Cada fila muestra el
//! nombre del proveedor con su estado del sondeo, p. ej. «llama-server · up (3)», y marca con `●`
//! el proveedor vigente. Usa el mismo estilo que el selector rápido de `/model`.

use super::settings::PICKER_VISIBLE_ROWS;
use super::*;

/// Ancho máximo del recuadro: lo bastante para el identificador, el nombre con su estado y la URL.
const MAX_WIDTH: u16 = 72;

/// Dibuja el selector de proveedores centrado en `area`. Con menos sitio del mínimo no dibuja nada.
pub(super) fn draw_provider_picker(frame: &mut Frame, area: Rect, app: &App) {
    let items = app.provider_items();
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
        .title(" Provider ")
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
            "No matches: Enter tries the typed id",
            app.theme.styles.muted,
        ));
    }
    // El vigente se identifica por la URL base, como hace el catálogo en `/settings`.
    let current = app.current_provider_id();
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
        let is_current = current.as_deref() == Some(item.id.as_str());
        lines.push(Line::from(vec![
            Span::styled(
                if is_selected { "› " } else { "  " },
                app.theme.styles.accent_from,
            ),
            Span::styled(
                if is_current { "● " } else { "  " },
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
