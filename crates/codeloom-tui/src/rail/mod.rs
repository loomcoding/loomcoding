//! Rail derecho de contexto del look Crush: modelo, ficheros, herramientas,
//! jobs y subagentes de la sesión abierta.

use crate::app::{agent_tag, formato_recuento, AgentStats, App, Focus};
use batten::state::{AgentState, ToolStatus};
use codeloom_core::job::JobStatus;
use codeloom_core::task::TaskState;
use codeloom_core::todo::TodoStatus;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::time::{Duration, Instant};

mod capabilities;
mod format;
mod identity;
mod lists;
mod metrics;
mod subagents;
mod text;

use capabilities::*;
use format::*;
pub use format::{format_cost, format_elapsed, format_prices, format_tokens};
use identity::*;
use lists::*;
use metrics::*;
pub use subagents::agent_state;
use subagents::*;
pub(crate) use text::partir_por_palabras;
use text::*;

#[cfg(test)]
mod tests;

/// Máximo de tareas mostradas; el resto se resume en una línea.
const TODO_LIMIT: usize = 8;

/// Máximo de ficheros mostrados; el resto se resume en una línea.
const FILE_LIMIT: usize = 8;

/// Máximo de herramientas recientes mostradas.
const TOOL_LIMIT: usize = 6;

/// Máximo de jobs de la sesión mostrados.
const JOB_LIMIT: usize = 5;

/// Máximo de subagentes mostrados.
const SUBAGENT_LIMIT: usize = 5;

/// Dibuja el rail en `area`, separado del chat por un borde izquierdo.
/// El contenido se recorta al alto disponible; no hay desplazamiento.
pub fn draw_context_rail(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(app.theme.styles.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    frame.render_widget(
        Paragraph::new(rail_lines(app, usize::from(inner.width))),
        inner,
    );
}

/// Construye las líneas del rail. `width` es la anchura útil del área interior
/// y solo se usa para recortar las métricas de los subagentes. Cada lista se
/// limita antes de construir sus líneas, así que el coste no crece con el
/// número de elementos.
fn rail_lines(app: &App, width: usize) -> Vec<Line<'_>> {
    let mut out: Vec<Line<'_>> = Vec::new();
    push_identity(&mut out, app, width);
    push_capabilities(&mut out, app, width);
    push_todos(&mut out, app);
    push_files(&mut out, app);
    push_tools(&mut out, app);
    push_jobs(&mut out, app);
    push_subagents(&mut out, app, width);
    out
}
