//! Dibujo por look: crush, martty y mindfork.

use super::*;

/// Pinta el composer con el estilo que indica el look del tema.
pub fn render_for_look(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    model: &str,
    theme: &Theme,
) {
    let (text_area, scrollbar_area) = composer_areas(area);
    match theme.look.composer {
        ComposerStyle::Classic => {
            render_focused_with_model(frame, text_area, composer, focused, model, theme);
            if let Some(scrollbar_area) = scrollbar_area {
                let viewport = usize::from(text_area.height.saturating_sub(2));
                render_scrollbar(
                    frame,
                    Rect::new(
                        scrollbar_area.x,
                        scrollbar_area.y.saturating_add(1),
                        scrollbar_area.width,
                        scrollbar_area.height.saturating_sub(2),
                    ),
                    composer.view_top(),
                    classic_layout(composer, text_area.width).rows.len(),
                    viewport,
                    theme,
                );
            }
        }
        ComposerStyle::Crush => {
            render_crush(
                frame,
                text_area,
                composer,
                focused,
                "Describe what you want to build…",
                theme,
            );
            if let Some(scrollbar_area) = scrollbar_area {
                let total = crush_layout(composer, text_area.width).rows.len();
                render_scrollbar(
                    frame,
                    scrollbar_area,
                    composer.view_top(),
                    total,
                    usize::from(text_area.height),
                    theme,
                );
            }
        }
    }
}

/// Composer de Crush: textarea sin marco, prompt morado por línea y placeholder
/// integrado en el primer renglón. Hace wrap por grafemas y desplaza
/// verticalmente para mantener el cursor visible.
pub fn render_crush(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    placeholder: &str,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let prompt = theme.style(StyleToken::Accent).add_modifier(Modifier::BOLD);
    let muted = theme.style(StyleToken::Muted);
    let text_style = theme.style(StyleToken::Default);
    let height = usize::from(area.height);

    if composer.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("> ", prompt),
                Span::styled(placeholder, muted),
            ]))
            .style(theme.style(StyleToken::Default)),
            area,
        );
        if focused && area.x + 2 < area.right() {
            frame.set_cursor_position((area.x + 2, area.y));
        }
        return;
    }

    let layout = crush_layout(composer, area.width);
    composer.mark_painted();
    let (cursor_row, cursor_col) = layout.cursor;
    let scroll = stable_top(composer.view_top(), cursor_row, height, layout.rows.len());
    composer.set_view_top(scroll);
    let source = composer.text();
    let sel = composer.selection_bytes();
    let selected = theme.style(StyleToken::Selection);
    let lines = layout
        .rows
        .iter()
        .skip(scroll)
        .take(height)
        .map(|row| {
            let mut spans = Vec::with_capacity(4);
            let head = crush_gutter_head(composer, row, prompt, theme);
            spans.push(head);
            spans.push(Span::styled(&crush_gutter(row)[1..], prompt));
            push_parts(
                &mut spans,
                split_selected(source, row.start, row.end, sel),
                text_style,
                selected,
            );
            if newline_selected(source, row.end, sel) {
                spans.push(Span::styled(" ", selected));
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(theme.style(StyleToken::Default)),
        area,
    );

    if focused && cursor_row >= scroll && cursor_row - scroll < height {
        let gutter = layout
            .rows
            .get(cursor_row)
            .map_or(0, |row| UnicodeWidthStr::width(crush_gutter(row)));
        let x = area.x as usize + gutter + cursor_col;
        let y = area.y as usize + (cursor_row - scroll);
        if x < usize::from(area.right()) && y < usize::from(area.bottom()) {
            frame.set_cursor_position((x as u16, y as u16));
        }
    }
}

/// Composer de Martty: borde redondeado, consejo y workspace en la línea
/// superior, estado/atajos en la inferior y prompt azul dentro del área.
pub fn render_martty(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    theme: &Theme,
) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let muted = theme.style(StyleToken::Muted);
    let accent = theme.style(StyleToken::Accent);
    let border = if focused {
        accent
    } else {
        theme.style(StyleToken::Border)
    };
    let tip_label = " Tip";
    let tip = " · type ! to run a command in the session's persistent local shell ";
    let workspace = "· ~/project/Martty:main ";
    let cap_fill = (area.width as usize)
        .saturating_sub(tip_label.width() + tip.width() + workspace.width() + 2);
    let cap = Line::from(vec![
        Span::styled(tip_label, accent.add_modifier(Modifier::BOLD)),
        Span::styled(tip, muted),
        Span::raw("─".repeat(cap_fill)),
        Span::styled(workspace, muted),
    ]);
    let left_width = "· Standard ".width()
        + "ctrl+shift+a".width()
        + " · ".width()
        + "Full access ".width()
        + "shift+tab".width();
    let right_width = "^K Keys · ".width() + "deepseek-v4-flash".width() + " · max".width();
    let meta_fill = (area.width as usize).saturating_sub(left_width + right_width);
    let meta = Line::from(vec![
        Span::styled("· Standard ", muted),
        Span::styled("ctrl+shift+a", accent),
        Span::styled(" · ", muted),
        Span::styled("Full access ", theme.style(StyleToken::Warning)),
        Span::styled("shift+tab", accent),
        Span::raw(" ".repeat(meta_fill)),
        Span::styled("^K Keys · ", muted),
        Span::styled("deepseek-v4-flash", accent),
        Span::styled(" · max", muted),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(cap)
        .title_bottom(meta)
        .style(theme.style(StyleToken::Default));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    if composer.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("❯ ", accent.add_modifier(Modifier::BOLD)),
            Span::styled(
                "Describe what you want to build…",
                muted.add_modifier(Modifier::ITALIC),
            ),
        ]));
    } else {
        for (row, text) in composer.text().split('\n').enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    if row == 0 { "❯ " } else { "  " },
                    accent.add_modifier(Modifier::BOLD),
                ),
                Span::styled(text.to_string(), theme.style(StyleToken::Default)),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(theme.style(StyleToken::Default))
            .wrap(Wrap { trim: false }),
        inner,
    );
    if focused && !inner.is_empty() {
        let (row, col) = composer.cursor_row_col();
        let x = inner.x + 2 + col as u16;
        let y = inner.y + row as u16;
        if x < inner.right() && y < inner.bottom() {
            frame.set_cursor_position((x, y));
        }
    }
}

/// Composer de Mindfork: campo fino con título contextual, borde gris suave
/// y prompt verde. La barra de estado queda en la línea siguiente.
pub fn render_mindfork(
    frame: &mut Frame,
    area: Rect,
    composer: &Composer,
    focused: bool,
    theme: &Theme,
) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let background = Style::default().bg(ratatui::style::Color::Rgb(15, 17, 21));
    let muted = Style::default().fg(ratatui::style::Color::Rgb(133, 139, 146));
    let prompt = Style::default().fg(ratatui::style::Color::Rgb(111, 192, 130));
    let text_style = theme.style(StyleToken::Default);
    let border = if focused {
        Style::default().fg(ratatui::style::Color::Rgb(121, 169, 219))
    } else {
        Style::default().fg(ratatui::style::Color::Rgb(54, 58, 66))
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(
            " input · Enter send · Shift+Enter newline ",
            muted,
        ))
        .style(background);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = if composer.is_empty() {
        vec![Line::from(vec![
            Span::styled("› ", prompt),
            Span::styled("Ask anything…", muted),
        ])]
    } else {
        composer
            .text()
            .split('\n')
            .enumerate()
            .map(|(row, text)| {
                Line::from(vec![
                    Span::styled(if row == 0 { "› " } else { "  " }, prompt),
                    Span::styled(text.to_string(), text_style),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(background)
            .wrap(Wrap { trim: false }),
        inner,
    );
    if focused && !inner.is_empty() {
        let (row, col) = composer.cursor_row_col();
        let x = inner.x + 2 + col as u16;
        let y = inner.y + row as u16;
        if x < inner.right() && y < inner.bottom() {
            frame.set_cursor_position((x, y));
        }
    }
}
