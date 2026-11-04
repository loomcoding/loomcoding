//! Mensajes de Pawan: papel, mensaje y espera.

use super::*;

/// Rol de mensaje de la vista Pawan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PawanRole {
    User,
    Assistant,
    System,
    Tool,
    Error,
}

/// Mensaje de ejemplo para el renderer Pawan.
pub struct PawanMessage<'a> {
    pub role: PawanRole,
    pub elapsed: &'a str,
    pub content: &'a str,
    pub streaming: bool,
}

/// Renderiza mensajes Pawan con badge de rol, tiempo y contenido indentado.
pub fn render_pawan(frame: &mut Frame, area: Rect, messages: &[PawanMessage<'_>]) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let muted = Style::default().fg(Color::Rgb(136, 136, 136));
    let mut lines = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            lines.push(TLine::raw(""));
        }
        let (label, badge_color) = match message.role {
            PawanRole::User => (" You ", Color::Rgb(0, 204, 204)),
            PawanRole::Assistant => (" Pawan ", Color::Rgb(80, 250, 123)),
            PawanRole::System => (" System ", Color::Rgb(255, 184, 108)),
            PawanRole::Tool => (" Tool ", Color::Rgb(0, 153, 153)),
            PawanRole::Error => (" Error ", Color::Rgb(255, 85, 85)),
        };
        lines.push(TLine::from(vec![
            Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(badge_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {}", message.elapsed), muted),
        ]));
        lines.extend(message.content.lines().map(|line| {
            TLine::from(vec![
                Span::raw("  "),
                Span::styled(line.to_string(), Style::default().fg(Color::White)),
            ])
        }));
        if message.streaming {
            lines.push(TLine::from(Span::styled(
                "  ▌",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::SLOW_BLINK),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

/// Estado de generación sin bloques, como el spinner de "Pawan is thinking…".
pub fn render_pawan_waiting(frame: &mut Frame, area: Rect, anim: &crate::widgets::Anim) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let glyph = anim.glyph(
        crate::widgets::spinner::PULSE,
        &Theme::preset(crate::widgets::ThemePreset::Pawan),
    );
    frame.render_widget(
        Paragraph::new(TLine::from(vec![
            Span::raw("  "),
            glyph,
            Span::styled(
                " Pawan is thinking…",
                Style::default()
                    .fg(Color::Rgb(255, 184, 108))
                    .add_modifier(Modifier::ITALIC),
            ),
        ])),
        area,
    );
}
