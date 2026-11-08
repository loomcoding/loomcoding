//! Vistas de `/help` distintas de los atajos: el índice de temas, la ayuda de un comando y la de un
//! tema. El texto sale de `help_topics` y de `COMMANDS`, la misma fuente que la referencia.

use super::*;
use crate::commands::COMMANDS;
use crate::help_topics::{command_help, HelpView, COMMANDS_DOC, TOPICS};

/// Tamaño mínimo de estas vistas; por debajo se pide agrandar el terminal.
const ANCHO_MINIMO: u16 = 40;
const ALTO_MINIMO: u16 = 10;

/// Pinta la vista `vista` como recuadro centrado. El texto envuelve; nunca se corta a 40 columnas.
pub(super) fn draw_help_page(frame: &mut Frame, area: Rect, app: &App, vista: HelpView) {
    if area.width < ANCHO_MINIMO || area.height < ALTO_MINIMO {
        draw_resize_hint(frame, area, "Help", app);
        return;
    }
    let (titulo, pie, lineas) = contenido(vista);
    let width = area.width.min(78).saturating_sub(2).max(ANCHO_MINIMO);
    let height = area.height.min(24).saturating_sub(1).max(ALTO_MINIMO);
    let popup = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(titulo)
        .title_bottom(Line::styled(pie, app.theme.styles.muted))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(app.theme.styles.border);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(Paragraph::new(lineas).wrap(Wrap { trim: true }), inner);
}

/// Título, pie y líneas de cada vista. Los atajos no pasan por aquí: los pinta `overlays.rs`.
fn contenido(vista: HelpView) -> (String, &'static str, Vec<Line<'static>>) {
    match vista {
        HelpView::Topics => {
            let mut lineas = vec![
                Line::from("/help <command>   one command, e.g. /help model"),
                Line::from("/help <topic>     one topic, e.g. /help flows"),
                Line::from(""),
            ];
            lineas.extend(
                TOPICS
                    .iter()
                    .map(|(id, descripcion, _)| Line::from(format!("{id:<12} {descripcion}"))),
            );
            (
                " CodeL∞M · Help topics ".into(),
                "F1/Esc close · Tab shortcuts",
                lineas,
            )
        }
        HelpView::Command(command) => {
            let nombre_y_descripcion = COMMANDS
                .iter()
                .find(|(c, _, _)| *c == command)
                .map_or(("", ""), |(_, nombre, descripcion)| (*nombre, *descripcion));
            let ayuda = command_help(command);
            let lineas = vec![
                Line::from(nombre_y_descripcion.1),
                Line::from(""),
                Line::from(format!("Usage: {}", ayuda.usage)),
                Line::from(format!("Example: {}", ayuda.example)),
                Line::from(""),
                Line::from(format!("Docs: {COMMANDS_DOC}")),
            ];
            (
                format!(" CodeL∞M · {} ", nombre_y_descripcion.0),
                "F1/Esc close · Tab topics",
                lineas,
            )
        }
        HelpView::Topic(indice) => {
            let (id, descripcion, ruta) =
                TOPICS
                    .get(indice)
                    .copied()
                    .unwrap_or(("", "", COMMANDS_DOC));
            let lineas = vec![
                Line::from(descripcion),
                Line::from(""),
                Line::from(format!("Docs: {ruta}")),
            ];
            (
                format!(" CodeL∞M · Topic: {id} "),
                "F1/Esc close · Tab topics",
                lineas,
            )
        }
        // Los atajos los pinta `overlays.rs`; esta rama solo existe para que la búsqueda sea exhaustiva.
        HelpView::Shortcuts => (String::new(), "", Vec::new()),
    }
}
