//! Raíz de la ventana `/temas` (panel `/ui`): una fila por `UiRow`, con su valor vigente.
//!
//! Las filas que se cambian con `←`/`→` llevan `‹ valor ›`. «Efectos», que solo se entra,
//! lleva `valor ›`. La lista recorta con desplazamiento si el área no cabe entera, y no
//! dibuja nada en áreas de cero filas o columnas.

use crate::app::{App, UiRow};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Ancho de la columna de etiquetas, para alinear los valores.
const LABEL_WIDTH: usize = 14;

/// Dibuja la raíz en `area`, con la fila seleccionada siempre a la vista.
pub(super) fn draw_ui_root(frame: &mut Frame, area: Rect, app: &App) {
    let visible = usize::from(area.height);
    if visible == 0 || area.width == 0 {
        return;
    }
    let total = UiRow::ALL.len();
    let selected = app.temas_selected.min(total - 1);
    let start = (selected + 1).saturating_sub(visible);
    let lines: Vec<Line> = UiRow::ALL
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
        .map(|(index, row)| linea(app, *row, index == selected))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// Una fila: marca, etiqueta alineada y valor con su indicador.
fn linea(app: &App, row: UiRow, selected: bool) -> Line<'static> {
    let (marca, estilo_fila, estilo_valor) = if selected {
        ("▸ ", app.theme.styles.accent, app.theme.styles.accent)
    } else {
        ("  ", app.theme.styles.default, app.theme.styles.muted)
    };
    let valor = app.ui_row_value(row);
    let pista = match row {
        UiRow::Effects => format!("{valor} ›"),
        _ => format!("‹ {valor} ›"),
    };
    Line::from(vec![
        Span::styled(
            format!("{marca}{:<ancho$}", row.label(), ancho = LABEL_WIDTH),
            estilo_fila,
        ),
        Span::styled(pista, estilo_valor),
    ])
}
