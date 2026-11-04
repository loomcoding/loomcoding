//! Glifos y estilos del tema, y el estilo visual de cada estado de agente.

use super::*;

/// Glifos visuales que cambian con el tema (iconos de tool, todo, campo del logo).
#[derive(Debug, Clone, Copy)]
pub struct Glyphs {
    pub tool_success: char,
    pub tool_error: char,
    pub tool_pending: char,
    pub todo_pending: char,
    pub todo_in_progress: char,
    pub todo_completed: char,
    pub field: char,
}

impl Default for Glyphs {
    fn default() -> Self {
        Self {
            tool_success: '\u{2713}',     // ✓
            tool_error: '×',              // ×
            tool_pending: '\u{25cf}',     // ●
            todo_pending: '\u{2022}',     // •
            todo_in_progress: '\u{2192}', // →
            todo_completed: '\u{2713}',   // ✓
            field: '\u{2571}',            // ╱
        }
    }
}

/// Tokens concretos para cada aspecto visual. Un campo por token semántico +
/// mapa extensible.
#[derive(Debug, Clone)]
pub struct Styles {
    pub default: Style,
    pub muted: Style,
    pub dim: Style,
    pub heading: Style,
    pub user: Style,
    pub assistant: Style,
    pub system: Style,
    pub error: Style,
    pub warning: Style,
    pub success: Style,
    pub tool_name: Style,
    pub tool_path: Style,
    pub tool_command: Style,
    pub tool_count: Style,
    pub tool_pending: Style,
    pub tool_running: Style,
    pub tool_success: Style,
    pub tool_error: Style,
    pub tool_canceled: Style,
    pub todo_pending: Style,
    pub todo_in_progress: Style,
    pub todo_completed: Style,
    pub accent: Style,
    pub accent_from: Style,
    pub accent_to: Style,
    pub logo_field: Style,
    pub scrollbar: Style,
    pub border: Style,
    pub diff_insert: Style,
    pub diff_delete: Style,
    pub diff_hunk: Style,
    pub selection: Style,
    pub code: Style,
    pub strong: Style,
    pub emphasis: Style,
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            default: Style::default(),
            muted: Style::default().fg(Color::Indexed(244)),
            dim: Style::default().fg(Color::Indexed(240)),
            heading: Style::default()
                .fg(Color::Indexed(250))
                .add_modifier(Modifier::BOLD),
            user: Style::default().fg(Color::Cyan),
            assistant: Style::default().fg(Color::Indexed(252)),
            system: Style::default().fg(Color::Indexed(245)),
            error: Style::default().fg(Color::Red),
            warning: Style::default().fg(Color::Yellow),
            success: Style::default().fg(Color::Green),
            tool_name: Style::default()
                .fg(Color::Indexed(252))
                .add_modifier(Modifier::BOLD),
            // Derivados: ruta ← accent, comando ← default, recuento ← dim.
            tool_path: Style::default().fg(Color::Rgb(0, 212, 255)),
            tool_command: Style::default(),
            tool_count: Style::default().fg(Color::Indexed(240)),
            tool_pending: Style::default().fg(Color::Indexed(244)),
            tool_running: Style::default().fg(Color::Cyan),
            tool_success: Style::default().fg(Color::Green),
            tool_error: Style::default().fg(Color::Red),
            tool_canceled: Style::default().fg(Color::Indexed(240)),
            todo_pending: Style::default().fg(Color::Indexed(240)),
            todo_in_progress: Style::default().fg(Color::Green),
            todo_completed: Style::default().fg(Color::Green),
            accent: Style::default().fg(Color::Rgb(0, 212, 255)),
            accent_from: Style::default().fg(Color::Rgb(0, 212, 255)),
            accent_to: Style::default().fg(Color::Rgb(124, 92, 255)),
            logo_field: Style::default().fg(Color::Indexed(238)),
            scrollbar: Style::default().fg(Color::Indexed(240)),
            border: Style::default().fg(Color::Indexed(240)),
            diff_insert: Style::default().fg(Color::Green),
            diff_delete: Style::default().fg(Color::Red),
            diff_hunk: Style::default().fg(Color::Cyan),
            selection: Style::default().bg(Color::Indexed(238)),
            code: Style::default().fg(Color::Indexed(180)),
            strong: Style::default().add_modifier(Modifier::BOLD),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
        }
    }
}

impl AgentState {
    /// Estilo del estado a partir de los tokens del tema. `InputRequired` y
    /// `AuthRequired` comparten el aviso porque ambos esperan a un actor externo.
    pub fn style(self, styles: &Styles) -> Style {
        match self {
            Self::Submitted => styles.muted,
            Self::Working => styles.tool_running,
            Self::InputRequired | Self::AuthRequired => styles.warning,
            Self::Completed => styles.tool_success,
            Self::Failed | Self::Rejected => styles.tool_error,
            Self::Canceled => styles.tool_canceled,
        }
    }
}
