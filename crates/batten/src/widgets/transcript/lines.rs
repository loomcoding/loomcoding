//! Pintado de una línea del transcript: prefijo, tokens de estilo y medida.

use super::*;

pub(super) fn prefix(kind: LineKind) -> &'static str {
    match kind {
        LineKind::User | LineKind::Assistant => "",
        LineKind::System => "sys: ",
        LineKind::Thinking => "… ",
        LineKind::ToolCall | LineKind::ToolResult => "⚙ ",
        LineKind::Todo => "☐ ",
        LineKind::Plan => "plan: ",
        LineKind::Banner => "",
        LineKind::Error => "! ",
        LineKind::ToolHead(_) => "● ",
        LineKind::ActivityHead(_) => "◆ ",
        LineKind::TodoItem(ToolStatus::Running) => "◐ ",
        LineKind::TodoItem(ToolStatus::Success) => "☑ ",
        LineKind::TodoItem(_) => "☐ ",
        LineKind::ToolBody | LineKind::ToolDiff(_) => "│ ",
        LineKind::ToolMore => "  ",
        LineKind::NodeHead(_) | LineKind::NodeBody => "",
        LineKind::Fold => "",
    }
}

pub(super) fn kind_token(kind: LineKind) -> StyleToken {
    match kind {
        LineKind::User => StyleToken::User,
        LineKind::Assistant => StyleToken::Assistant,
        LineKind::System => StyleToken::System,
        LineKind::Thinking => StyleToken::Muted,
        LineKind::ToolCall | LineKind::ToolResult => StyleToken::ToolName,
        LineKind::Todo => StyleToken::Heading,
        LineKind::Plan => StyleToken::Heading,
        LineKind::Banner => StyleToken::Accent,
        LineKind::Error => StyleToken::Error,
        LineKind::ToolHead(status) | LineKind::ActivityHead(status) => status_token(status),
        LineKind::TodoItem(ToolStatus::Running) => StyleToken::TodoInProgress,
        LineKind::TodoItem(ToolStatus::Success) => StyleToken::TodoCompleted,
        LineKind::TodoItem(_) => StyleToken::TodoPending,
        LineKind::ToolBody => StyleToken::Muted,
        LineKind::ToolDiff(DiffKind::Insert) => StyleToken::DiffInsert,
        LineKind::ToolDiff(DiffKind::Delete) => StyleToken::DiffDelete,
        LineKind::ToolDiff(DiffKind::Hunk) => StyleToken::DiffHunk,
        LineKind::ToolDiff(DiffKind::Context) => StyleToken::Muted,
        LineKind::ToolMore => StyleToken::Dim,
        LineKind::NodeHead(status) => status_token(status),
        LineKind::NodeBody => StyleToken::Muted,
        LineKind::Fold => StyleToken::Dim,
    }
}

/// Token de la cabecera de una tool o de un elemento de actividad según su estado.
pub(super) fn status_token(status: ToolStatus) -> StyleToken {
    match status {
        ToolStatus::Pending => StyleToken::ToolPending,
        ToolStatus::Running => StyleToken::ToolRunning,
        ToolStatus::Success => StyleToken::ToolSuccess,
        ToolStatus::Error => StyleToken::ToolError,
        ToolStatus::Canceled => StyleToken::ToolCanceled,
        ToolStatus::AwaitingPermission => StyleToken::Warning,
    }
}

/// Convierte una `Line` de estado en lineas ratatui, aplicando los `span` por
/// offsets sobre el texto plano (los huecos usan el token del kind).
pub(super) fn render_line(line: &Line, theme: &Theme, entry_start: bool) -> Vec<TLine<'static>> {
    let base = theme.style(kind_token(line.kind));
    let role = match (entry_start, line.kind) {
        (true, LineKind::User) => Some(("👤", theme.styles.user)),
        (true, LineKind::Assistant) => Some(("🤖", theme.styles.assistant)),
        _ => None,
    };
    let text = line.plain();
    let mut rendered = Vec::new();
    if let Some((label, style)) = role {
        rendered.push(TLine::from(Span::styled(
            label,
            style.add_modifier(Modifier::BOLD),
        )));
    }
    let mut spans: Vec<Span> = vec![Span::styled(prefix(line.kind).to_string(), base)];

    if line.spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base));
        rendered.push(TLine::from(spans));
        return rendered;
    }

    let mut spans_sorted = line.spans.clone();
    spans_sorted.sort_by_key(|s| s.start);
    let mut cursor = 0usize;
    for span in spans_sorted {
        let start = span.start.min(text.len());
        let end = (span.start + span.len).min(text.len());
        if start < cursor {
            continue;
        }
        if cursor < start {
            spans.push(Span::styled(text[cursor..start].to_string(), base));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            theme.style(span.token),
        ));
        cursor = end;
    }
    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), base));
    }
    rendered.push(TLine::from(spans));
    rendered
}

/// Altura en filas de una línea a `width` columnas (mínimo 1).
pub(super) fn measure(line: &Line, width: u16, theme: &Theme, entry_start: bool) -> usize {
    Paragraph::new(render_line(line, theme, entry_start))
        .wrap(Wrap { trim: false })
        .line_count(width)
        .max(1)
}
