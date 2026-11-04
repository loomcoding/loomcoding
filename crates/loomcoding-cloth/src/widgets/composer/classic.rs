//! Dibujo del composer clásico.

use super::*;

/// Composer con foco (borde de acento y cursor).
pub fn render(frame: &mut Frame, area: Rect, composer: &Composer, theme: &Theme) {
    render_focused(frame, area, composer, true, theme);
}

/// Composer; sin foco, borde atenuado y sin cursor.
pub fn render_focused(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    theme: &Theme,
) {
    render_focused_with_model(frame, area, composer, focused, "", theme);
}

/// Composer principal con el modelo activo en su cabecera.
pub fn render_focused_with_model(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    model: &str,
    theme: &Theme,
) {
    if area.is_empty() {
        return;
    }
    let accent = theme.style(StyleToken::Accent);
    let muted = theme.style(StyleToken::Muted);
    let border = if focused {
        accent
    } else {
        theme.style(StyleToken::Dim)
    };
    let mut title = vec![
        Span::styled("✦ ", accent.add_modifier(Modifier::BOLD)),
        Span::styled("LOOMCODING", theme.style(StyleToken::Heading)),
        Span::styled("  /  PROMPT", muted),
    ];
    if !model.is_empty() {
        title.push(Span::styled(format!("   ·   {model}"), muted));
    }
    let keys = "Ctrl+J newline  ·  Enter send  ·  / commands";
    let line_count = composer.lines();
    let stats = format!(
        "{line_count} {}  ·  {} chars",
        if line_count == 1 { "line" } else { "lines" },
        composer.text().graphemes(true).count()
    );
    let fill =
        usize::from(area.width.saturating_sub(2)).saturating_sub(keys.width() + stats.width() + 2);
    let footer = Line::from(vec![
        Span::styled(format!(" {keys}"), muted),
        Span::raw(" ".repeat(fill)),
        Span::styled(stats, accent),
        Span::raw(" "),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(title))
        .title_bottom(footer)
        .border_style(border)
        .style(theme.style(StyleToken::Default));
    let inner = classic_inner(area);
    frame.render_widget(block, area);

    let layout = classic_layout(composer, area.width);
    let (row, col) = layout.cursor;
    let scroll = stable_top(
        composer.view_top(),
        row,
        usize::from(inner.height),
        layout.rows.len(),
    );
    composer.set_view_top(scroll);
    composer.mark_painted();
    let source = composer.text();
    let input = theme.style(StyleToken::Default);
    let selected = theme.style(StyleToken::Selection);
    let prompt = accent.add_modifier(Modifier::BOLD);
    let lines = if composer.is_empty() {
        vec![Line::from(vec![
            Span::styled("❯ ", prompt),
            Span::styled(
                "Describe what you want to build…",
                muted.add_modifier(Modifier::ITALIC),
            ),
        ])]
    } else {
        let sel = composer.selection_bytes();
        layout
            .rows
            .iter()
            .skip(scroll)
            .take(usize::from(inner.height))
            .map(|visual_row| {
                let mut spans = Vec::with_capacity(4);
                spans.push(Span::styled(
                    match (visual_row.line == 0, visual_row.first) {
                        (true, true) => "❯ ",
                        (false, true) => "│ ",
                        (_, false) => "  ",
                    },
                    prompt,
                ));
                push_parts(
                    &mut spans,
                    split_selected(source, visual_row.start, visual_row.end, sel),
                    input,
                    selected,
                );
                if newline_selected(source, visual_row.end, sel) {
                    spans.push(Span::styled(" ", selected));
                }
                Line::from(spans)
            })
            .collect()
    };
    let text = Paragraph::new(Text::from(lines)).style(theme.style(StyleToken::Default));
    frame.render_widget(text, inner);

    if !focused {
        return;
    }
    let row = row.saturating_sub(scroll);
    if row < usize::from(inner.height)
        && col < usize::from(inner.width.saturating_sub(CLASSIC_PREFIX))
    {
        let x = inner.x + CLASSIC_PREFIX + col as u16;
        if x < inner.right() {
            frame.set_cursor_position((x, inner.y + row as u16));
        }
    }
}
