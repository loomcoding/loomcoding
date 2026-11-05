//! Bloques `Session` y `Tokens` del rail: recordatorio de `/parent`, precios, fase, pasos,
//! tokens y coste.

use super::*;

/// Bloque `Session`: recordatorio de `/parent`, precios del modelo, fase y tiempo, y pasos.
/// La marca, el modelo y el esfuerzo ya van en la cabecera, así que no se repiten aquí. Los
/// tokens y el coste tienen bloque propio, `Tokens`.
pub(super) fn push_identity<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    out.push(title("Session", app));
    // Dentro de un subagente, recordatorio de cómo volver al padre. Se recorta al rail.
    if !app.parent_stack.is_empty() {
        out.push(Line::from(Span::styled(
            recortar("↩ /parent · Alt+←", width),
            app.theme.styles.muted,
        )));
    }
    // Precio por millón del modelo actual, solo si el catálogo lo conoce.
    if let Some((input, output)) = app.model_prices() {
        out.push(Line::from(Span::styled(
            recortar(&format_prices(input, output), width),
            app.theme.styles.muted,
        )));
    }
    out.push(Line::from(Span::styled(
        format!("{} · {:.1}s", app.phase, app.phase_duration().as_secs_f64()),
        app.theme.styles.muted,
    )));
    if app.max_steps > 0 {
        out.push(Line::from(Span::styled(
            format!("step {}/{}", app.current_step, app.max_steps),
            app.theme.styles.muted,
        )));
    }
}

/// Bloque `Tokens`: cifras de entrada y salida de la sesión y, si hay precios, su coste. Va justo
/// tras `Session` y no se pinta si no hay tokens ni coste que mostrar.
pub(super) fn push_tokens<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    // Coste de la sesión: solo con apuntes. Si ningún apunte tiene precio, un «$0+» no informa
    // de nada: se omite la línea.
    let coste = app.estimated_cost().filter(|(cost, _)| *cost > 0.0);
    if app.input_tokens == 0 && app.output_tokens == 0 && coste.is_none() {
        return;
    }
    push_gap(out);
    out.push(title("Tokens", app));
    out.push(Line::from(Span::styled(
        format!("↑{} ↓{}", app.input_tokens, app.output_tokens),
        app.theme.styles.muted,
    )));
    if let Some((cost, partial)) = coste {
        let label = if app.cost_is_estimated() {
            format_cost(cost, partial)
        } else {
            format_actual_cost(cost, partial)
        };
        out.push(Line::from(Span::styled(
            recortar(&label, width),
            app.theme.styles.muted,
        )));
    }
}
