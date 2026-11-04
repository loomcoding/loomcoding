//! Segunda línea de métricas de un subagente.

use super::*;

/// Línea de métricas de un subagente, p. ej. `2/5 read_file · 1.2k/300 · ≈$0.004 · 7s`.
/// Omite lo que no exista: el paso sin `max_steps`, la herramienta sin curso y
/// los tokens, el coste y el tiempo sin `started_at`. `cost` llega ya formateado y el
/// recuento de ficheros (`files`) solo se muestra si es mayor que cero.
/// Recorta por columnas, terminando en `…`.
pub(super) fn metrics_line(
    stats: &AgentStats,
    cost: Option<String>,
    files: usize,
    now: Instant,
    width: usize,
) -> String {
    let mut grupos: Vec<String> = Vec::new();

    let mut paso = Vec::new();
    if stats.max_steps > 0 {
        paso.push(format!("{}/{}", stats.step, stats.max_steps));
    }
    if let Some(tool) = &stats.tool {
        paso.push(tool.clone());
    }
    if !paso.is_empty() {
        grupos.push(paso.join(" "));
    }

    if let Some(inicio) = stats.started_at {
        let fin = stats.finished_at.unwrap_or(now);
        grupos.push(format!(
            "{}/{}",
            format_tokens(stats.input_tokens),
            format_tokens(stats.output_tokens)
        ));
        grupos.extend(cost);
        if files > 0 {
            grupos.push(format!("{files}f"));
        }
        grupos.push(format_elapsed(fin.saturating_duration_since(inicio)));
    }

    recortar(&grupos.join(" · "), width)
}
