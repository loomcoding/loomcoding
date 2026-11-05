//! Franja del muñeco de estado sobre el compositor: tres filas con el muñeco a la izquierda y lo
//! que hace el agente a la derecha; apagada o sin braille, una sola línea de texto.
//!
//! Cabe señalar que la postura y el texto llegan de `crate::mascot::pose_of`: aquí solo se dibuja.

use super::*;
use crate::mascot::pose_of;
use loomcoding_cloth::widgets::mascot::{mascot_frame, Pose};
use std::time::Instant;

/// Nombre de la postura tal como aparece en la franja.
fn posture_name(pose: Pose) -> &'static str {
    match pose {
        Pose::Think => "thinking",
        Pose::Run => "running",
        Pose::Hunt => "searching",
        Pose::Wait => "waiting",
        Pose::Done => "done",
        Pose::Fail => "failed",
    }
}

/// Parte del texto de `pose_of` que no repite el nombre de la postura: «running · bash» queda en
/// «bash», y «done» queda vacío. Así la franja no dice dos veces lo mismo.
fn what_it_does<'a>(name: &str, text: &'a str) -> &'a str {
    let rest = text.strip_prefix(name).unwrap_or(text);
    rest.trim_start_matches([' ', '·'])
}

/// Filas que ocupa la franja: `0` sin actividad; `3` con muñeco en braille; `1` en otro caso.
pub(super) fn strip_rows(app: &App, now: Instant, braille: bool) -> u16 {
    if pose_of(app, now).is_none() {
        return 0;
    }
    if app.mascot && braille {
        3
    } else {
        1
    }
}

/// Dibuja la franja en `area`, que debe tener las filas que da `strip_rows` con los mismos
/// argumentos. Sin actividad no dibuja nada.
pub(super) fn draw_strip(frame: &mut Frame, area: Rect, app: &App, now: Instant, braille: bool) {
    let Some((pose, text)) = pose_of(app, now) else {
        return;
    };
    let name = posture_name(pose);
    let what = what_it_does(name, &text);
    let styles = &app.theme.styles;

    if !(app.mascot && braille) {
        let mut spans = vec![Span::styled(format!(" ◌ {name}"), styles.muted)];
        if !what.is_empty() {
            spans.push(Span::styled(format!(" · {what}"), styles.muted));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let paint = match pose {
        Pose::Fail => styles.error,
        Pose::Done => styles.success,
        _ => styles.accent,
    };
    let [top, bottom] = mascot_frame(pose, app.animation.frame);
    // Con la lupa, la búsqueda se distingue de la carrera de un vistazo.
    let lens = if pose == Pose::Hunt { "  ⌕" } else { "" };
    let top_line = Line::from(Span::styled(format!(" {top}{lens}"), paint));
    // El muñeco ocupa nueve columnas (margen, seis celdas y dos de hueco); lo que hace va después.
    let mut bottom_spans = vec![
        Span::styled(format!(" {bottom}  "), paint),
        Span::styled(name, styles.accent.add_modifier(Modifier::BOLD)),
    ];
    if !what.is_empty() {
        bottom_spans.push(Span::styled(format!("  {what}"), styles.muted));
    }
    let two_rows = Rect::new(area.x, area.y, area.width, area.height.min(2));
    frame.render_widget(
        Paragraph::new(vec![top_line, Line::from(bottom_spans)]),
        two_rows,
    );
}

/// Indica si el muñeco en braille puede dibujarse: `NO_COLOR` lo desactiva.
pub(super) fn braille_allowed() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}
