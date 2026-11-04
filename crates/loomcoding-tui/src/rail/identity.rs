//! Cabecera del rail: cwd, modelo, fase y coste.

use super::*;

/// Cabecera: cwd, modelo, precio, esfuerzo, fase, pasos, tokens y coste de la sesión.
pub(super) fn push_identity<'a>(out: &mut Vec<Line<'a>>, app: &'a App, width: usize) {
    out.push(title("LOOMCODING", app));
    out.push(Line::from(Span::styled(
        app.cwd.as_str(),
        app.theme.styles.muted,
    )));
    // Dentro de un subagente, recordatorio de cómo volver al padre. Se recorta al rail.
    if !app.parent_stack.is_empty() {
        out.push(Line::from(Span::styled(
            recortar("↩ /parent · Alt+←", width),
            app.theme.styles.muted,
        )));
    }

    push_gap(out);
    out.push(title("MODEL", app));
    out.push(Line::from(app.model.as_str()));
    // Precio por millón del modelo actual, solo si el catálogo lo conoce.
    if let Some((input, output)) = app.model_prices() {
        out.push(Line::from(Span::styled(
            recortar(&format_prices(input, output), width),
            app.theme.styles.muted,
        )));
    }
    // El nivel se lee de la copia guardada, la misma que `app.model`.
    if let Some(level) = app
        .settings_baseline
        .as_ref()
        .and_then(|settings| settings.effort.as_deref())
    {
        out.push(Line::from(Span::styled(
            format!("effort {level}"),
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
    out.push(Line::from(Span::styled(
        format!("↑{} ↓{}", app.input_tokens, app.output_tokens),
        app.theme.styles.muted,
    )));
    // Coste de la sesión bajo los tokens: solo con apuntes, y recortado al rail.
    // Si ningún apunte tiene precio, un «$0+» no informa de nada: se omite la línea.
    if let Some((cost, partial)) = app.estimated_cost().filter(|(cost, _)| *cost > 0.0) {
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
