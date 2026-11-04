//! Disposición del composer: áreas, barra de desplazamiento, gutters y altura por look.

use super::*;

pub(super) fn composer_areas(area: Rect) -> (Rect, Option<Rect>) {
    if area.width <= 1 {
        return (area, None);
    }
    let text = Rect {
        width: area.width - 1,
        ..area
    };
    let scrollbar = Rect::new(area.right() - 1, area.y, 1, area.height);
    (text, Some(scrollbar))
}

pub(super) fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    offset: usize,
    content: usize,
    viewport: usize,
    theme: &Theme,
) {
    if area.is_empty() || content <= viewport {
        return;
    }
    let mut state = ScrollbarState::new(content)
        .position(offset)
        .viewport_content_length(viewport);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("▏")
        .style(theme.style(StyleToken::Muted))
        .thumb_style(theme.style(StyleToken::Accent));
    frame.render_stateful_widget(scrollbar, area, &mut state);
}

/// Columnas del prefijo de cada línea del composer Classic (`❯ ` o `│ `).
pub(super) const CLASSIC_PREFIX: u16 = 2;

/// Interior del marco del composer Classic: un borde redondeado por lado. El
/// título y el pie van sobre el marco y no reducen el interior.
pub(super) fn classic_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

pub(super) fn classic_layout(composer: &Composer, width: u16) -> ComposerLayout {
    classic_layout_at(composer, composer.cursor(), width)
}

pub(super) fn classic_layout_at(composer: &Composer, cursor: usize, width: u16) -> ComposerLayout {
    let content = usize::from(width.saturating_sub(4));
    crate::state::composer_layout(composer.text(), cursor, content, content)
}

/// Primer span del gutter. Si el agente está trabajando, la primera fila de la
/// primera línea lógica muestra el indicador de actividad en vez de `> `. Ocupa
/// exactamente las mismas dos celdas, así que el layout no se mueve.
pub(super) fn crush_gutter_head(
    composer: &Composer,
    row: &VisualRow,
    prompt: Style,
    theme: &Theme,
) -> Span<'static> {
    let is_prompt_row = row.line == 0 && row.first;
    if is_prompt_row && composer.is_working() {
        let glyph = super::super::activity::activity_glyph(
            composer.activity_style(),
            composer.activity_frame(),
            1,
            theme,
        )
        .unwrap_or_else(|| Span::styled(">", prompt));
        return glyph;
    }
    let text = if is_prompt_row {
        ">"
    } else {
        &crush_gutter(row)[..1]
    };
    Span::styled(text.to_string(), prompt)
}

/// Prefijo visual de una fila del composer Crush: `> ` en la primera fila de
/// la primera línea lógica, `::: ` en la primera fila de las demás y sangría
/// de igual anchura en las filas de continuación.
pub(super) fn crush_gutter(row: &VisualRow) -> &'static str {
    match (row.line == 0, row.first) {
        (true, true) => "> ",
        (true, false) => "  ",
        (false, true) => "::: ",
        (false, false) => "    ",
    }
}

/// Layout de filas del composer Crush para un ancho de área dado. El ancho
/// descontado depende del prefijo de cada línea lógica (`> ` o `::: `).
pub(super) fn crush_layout(composer: &Composer, width: u16) -> ComposerLayout {
    crush_layout_at(composer, composer.cursor(), width)
}

/// Como `crush_layout`, pero con el cursor indicado. El cursor influye en las
/// filas (un cursor al final de una fila llena baja a una fila propia), así que
/// la traducción de celdas usa el pintado y el render, el vivo.
pub(super) fn crush_layout_at(composer: &Composer, cursor: usize, width: u16) -> ComposerLayout {
    crate::state::composer_layout(
        composer.text(),
        cursor,
        usize::from(width.saturating_sub(2)),
        usize::from(width.saturating_sub(4)),
    )
}

/// Mueve el cursor por filas visibles para que la rueda desplace también una
/// línea lógica larga que se parte visualmente en Crush.
pub fn scroll_cursor_by_rows(
    theme: &Theme,
    composer: &mut Composer,
    area: Rect,
    down: bool,
    rows: usize,
) {
    let (current, target, byte) = match theme.look.composer {
        ComposerStyle::Crush => {
            let layout = crush_layout(composer, area.width);
            let current = layout.cursor.0;
            let last = layout.rows.len().saturating_sub(1);
            let target = if down {
                current.saturating_add(rows).min(last)
            } else {
                current.saturating_sub(rows)
            };
            (
                current,
                target,
                layout.rows.get(target).map(|row| row.start),
            )
        }
        ComposerStyle::Classic => {
            let layout = classic_layout(composer, area.width);
            let current = layout.cursor.0;
            let last = layout.rows.len().saturating_sub(1);
            let target = if down {
                current.saturating_add(rows).min(last)
            } else {
                current.saturating_sub(rows)
            };
            (
                current,
                target,
                layout.rows.get(target).map(|row| row.start),
            )
        }
    };
    if target != current {
        if let Some(byte) = byte {
            composer.set_cursor(grapheme_count(composer.text(), byte));
        }
    }
}

/// Altura del composer según el look activo, acotada por `max`.
pub fn height_for_look(theme: &Theme, composer: &Composer, width: u16, max: u16) -> u16 {
    let max = usize::from(max);
    let height = match theme.look.composer {
        ComposerStyle::Classic => (classic_layout(composer, width.saturating_sub(1))
            .rows
            .len()
            .min(3)
            + 2)
        .min(max),
        ComposerStyle::Crush => crush_layout(composer, content_width(width))
            .rows
            .len()
            .clamp(1, 3)
            .min(max),
    };
    height as u16
}
