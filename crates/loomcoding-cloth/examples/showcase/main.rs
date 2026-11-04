//! Showcase navegable de los widgets de cloth.
//! Uso: `cargo run -p loomcoding-cloth --example showcase`

#[cfg(test)]
mod tests;

mod app;

mod demo;

mod demos;

mod matrix;

mod prompt_keys;

mod theme_view;

use app::{route, Action, Focus, IndexRow, Showcase};

use loomcoding_cloth::widgets::{Anim, Theme, FRAME_INTERVAL};

use matrix::{draw_matrix, matrix_cell_at, matrix_name_at, Cell};

use ratatui::crossterm::event::{self, Event, MouseButton, MouseEventKind};

use ratatui::layout::{Constraint, Layout, Position, Rect};

use ratatui::style::Modifier;

use ratatui::text::{Line, Span};

use ratatui::widgets::{Block, Paragraph};

use ratatui::Frame;

use std::time::{Duration, Instant};

const INDEX_WIDTH: u16 = 34;

const WIDE_MIN: u16 = 70;

/// Columnas entre dos pestañas de tema.
const TAB_GAP: u16 = 1;

/// Filas de cabecera del panel de contenido (cabecera del demo o de la matriz).
const HEADER_HEIGHT: u16 = 3;

/// Pasos del índice por cada muesca de la rueda del ratón.
const WHEEL_STEPS: usize = 3;

fn main() -> std::io::Result<()> {
    let mut show = Showcase::new();
    let mut active_preset = show.theme_preset();
    let mut theme = Theme::preset(active_preset);
    let mut anim = Anim::new();
    let mut terminal = ratatui::init();
    ratatui::crossterm::execute!(std::io::stdout(), event::EnableMouseCapture)?;
    let mut last_tick = Instant::now();

    while !show.should_quit() {
        if last_tick.elapsed() >= FRAME_INTERVAL {
            anim.advance();
            show.tick(&anim);
            last_tick = Instant::now();
        }
        terminal.draw(|f| draw(f, &mut show, &anim, &theme))?;
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(k) => {
                    if route(&mut show, k.kind, k.code, k.modifiers) == Action::Quit {
                        break;
                    }
                    let preset = show.theme_preset();
                    if preset != active_preset {
                        active_preset = preset;
                        theme = Theme::preset(preset);
                    }
                }
                Event::Mouse(mouse)
                    if matches!(
                        mouse.kind,
                        MouseEventKind::Down(MouseButton::Left)
                            | MouseEventKind::ScrollUp
                            | MouseEventKind::ScrollDown
                    ) =>
                {
                    let screen = terminal.size()?.into();
                    handle_mouse(&mut show, mouse.kind, mouse.column, mouse.row, screen);
                }
                _ => {}
            }
        }
    }

    ratatui::crossterm::execute!(std::io::stdout(), event::DisableMouseCapture)?;
    ratatui::restore();
    Ok(())
}

fn draw(f: &mut Frame, show: &mut Showcase, _anim: &Anim, theme: &Theme) {
    let area = f.area();
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Block::default().style(theme.style(loomcoding_cloth::state::StyleToken::Default)),
        area,
    );
    let footer_lines = footer_texts(show, area.width);
    let geo = geometry(area, footer_height(show));
    draw_filters(f, geo.tabs, geo.query, show, theme);

    // En la matriz no se consulta `fullscreen()`: el panel de contenido es siempre la matriz.
    let matrix_mode = show.focus() == Focus::Matrix;
    let fullscreen = !matrix_mode && show.current().is_some_and(|demo| demo.fullscreen());

    if let Some(ia) = geo.index {
        draw_index(f, ia, show, theme);
        if !fullscreen {
            draw_header(f, geo.header, show, theme);
        }
    } else if !fullscreen {
        draw_collapsed(f, geo.header, show, theme);
    }

    if matrix_mode {
        draw_matrix(f, geo.content, show, theme);
    } else if let Some(demo) = show.current() {
        demo.render(
            f,
            if demo.fullscreen() {
                geo.panel
            } else {
                geo.content
            },
            theme,
        );
    }
    draw_footer(f, geo.footer, &footer_lines, theme);
}

/// Áreas de la pantalla. Es la única fuente de la geometría: `draw` y `handle_mouse` la usan, así
/// que el clic cae siempre en lo que se ve.
struct Geometry {
    /// Fila de pestañas de tema.
    tabs: Rect,
    /// Fila del filtro de texto.
    query: Rect,
    /// Columna del índice; `None` en terminal estrecha, donde no hay índice.
    index: Option<Rect>,
    /// Panel de contenido completo: cabecera y contenido.
    panel: Rect,
    /// Cabecera del panel (demo o matriz).
    header: Rect,
    /// Contenido del panel: el demo, o la matriz.
    content: Rect,
    /// Pie, de una o dos líneas.
    footer: Rect,
}

/// Reparte la pantalla en áreas. `footer_height` es la altura del pie (`footer_height(show)`).
fn geometry(screen: Rect, footer_height: u16) -> Geometry {
    let [top, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(footer_height),
    ])
    .areas(screen);
    let [tabs, query] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(top);

    let (index, panel) = if screen.width >= WIDE_MIN {
        let cols =
            Layout::horizontal([Constraint::Length(INDEX_WIDTH), Constraint::Min(1)]).split(body);
        (Some(cols[0]), cols[1])
    } else {
        (None, body)
    };
    let [header, content] =
        Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(1)]).areas(panel);

    Geometry {
        tabs,
        query,
        index,
        panel,
        header,
        content,
        footer,
    }
}

/// Ancho de una pestaña de tema: el nombre con un espacio a cada lado.
fn tab_width(name: &str) -> u16 {
    name.chars().count() as u16 + 2
}

/// Posición de cada pestaña de tema visible en la fila de pestañas de un área de `width`
/// columnas: índice en `matrix::COLUMNS`, x de inicio y ancho.
///
/// Las pestañas van separadas por `TAB_GAP`. Una que no cabe entera se recorta a lo que queda, y
/// las siguientes no aparecen. El dibujo y el ratón usan esta misma función, así que no pueden
/// divergir.
pub(crate) fn tab_spans_layout(width: u16) -> Vec<(usize, u16, u16)> {
    let mut spans = Vec::new();
    let mut x = 0u16;
    for (column, (name, _)) in matrix::COLUMNS.iter().enumerate() {
        if x >= width {
            break;
        }
        let full = tab_width(name);
        let shown = full.min(width - x);
        spans.push((column, x, shown));
        if shown < full {
            break;
        }
        x = x.saturating_add(full).saturating_add(TAB_GAP);
    }
    spans
}

fn draw_filters(f: &mut Frame, tabs_area: Rect, query_area: Rect, show: &Showcase, theme: &Theme) {
    draw_tabs(f, tabs_area, show, theme);

    let muted = theme.style(loomcoding_cloth::state::StyleToken::Muted);
    let cursor = if show.focus() == Focus::TextFilter {
        "▏"
    } else {
        ""
    };
    let query = if show.query().is_empty() && show.focus() != Focus::TextFilter {
        "componente · ejemplo".to_string()
    } else {
        show.query().to_string()
    };
    let query_style = if show.focus() == Focus::TextFilter {
        theme.style(loomcoding_cloth::state::StyleToken::Heading)
    } else {
        muted
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Filter: ", muted),
            Span::styled(query, query_style),
            Span::styled(
                cursor,
                theme.style(loomcoding_cloth::state::StyleToken::Accent),
            ),
        ])),
        query_area,
    );
}

/// Fila de pestañas: los temas de `matrix::COLUMNS`, la activa en negrita y en vídeo inverso.
fn draw_tabs(f: &mut Frame, area: Rect, show: &Showcase, theme: &Theme) {
    let active = show.theme_column();
    let muted = theme.style(loomcoding_cloth::state::StyleToken::Muted);
    let active_style = theme
        .style(loomcoding_cloth::state::StyleToken::Accent)
        .add_modifier(Modifier::BOLD | Modifier::REVERSED);
    let mut spans = Vec::new();
    let mut x = 0u16;
    for (column, x0, width) in tab_spans_layout(area.width) {
        if x0 > x {
            spans.push(Span::raw(" ".repeat(usize::from(x0 - x))));
        }
        let Some((name, _)) = matrix::COLUMNS.get(column) else {
            break;
        };
        let style = if column == active {
            active_style
        } else {
            muted
        };
        let text = matrix::fit(&format!(" {name} "), usize::from(width));
        spans.push(Span::styled(text, style));
        x = x0 + width;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Atiende un evento de ratón sobre la geometría de `screen`: clic izquierdo y rueda.
///
/// Con `Focus::Matrix` la rueda mueve el cursor de la matriz, esté donde esté el puntero, y un
/// clic fuera de la tabla no cambia el foco ni cierra la matriz. Fuera de la matriz, la rueda solo
/// actúa sobre el índice.
fn handle_mouse(show: &mut Showcase, kind: MouseEventKind, column: u16, row: u16, screen: Rect) {
    if screen.width == 0 || screen.height == 0 {
        return;
    }
    // La altura del pie sale del estado actual, el mismo que pintó el último `draw`.
    let geo = geometry(screen, footer_height(show));
    match kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let up = kind == MouseEventKind::ScrollUp;
            if show.focus() == Focus::Matrix {
                show.matrix_move(if up { -1 } else { 1 }, 0);
            } else if geo.index.is_some_and(|area| inside(area, column, row)) {
                for _ in 0..WHEEL_STEPS {
                    if up {
                        show.prev();
                    } else {
                        show.next();
                    }
                }
            }
        }
        MouseEventKind::Down(MouseButton::Left) => click(show, &geo, column, row),
        _ => {}
    }
}

/// `true` si el punto (`column`, `row`) cae dentro de `area`.
fn inside(area: Rect, column: u16, row: u16) -> bool {
    area.contains(Position::new(column, row))
}

/// Resuelve un clic izquierdo sobre la geometría de `geo`.
fn click(show: &mut Showcase, geo: &Geometry, column: u16, row: u16) {
    if inside(geo.tabs, column, row) {
        if let Some(theme) = tab_at(geo.tabs, column) {
            show.set_theme_column(theme);
            show.set_focus(Focus::Index);
        }
        return;
    }
    if inside(geo.query, column, row) {
        show.set_focus(Focus::TextFilter);
        return;
    }
    if let Some(index) = geo.index.filter(|area| inside(*area, column, row)) {
        index_click(show, index, row);
        return;
    }
    if show.focus() == Focus::Matrix {
        matrix_click(show, geo.content, column, row);
        return;
    }
    if inside(geo.panel, column, row) {
        show.set_focus(Focus::Demo);
    }
}

/// Índice de la pestaña de tema bajo `column`, si esa columna pinta una (o parte de una).
fn tab_at(tabs: Rect, column: u16) -> Option<usize> {
    let x = column - tabs.x;
    tab_spans_layout(tabs.width)
        .into_iter()
        .find(|&(_, x0, shown)| x >= x0 && x < x0 + shown)
        .map(|(theme, _, _)| theme)
}

/// Clic en el índice: selecciona y abre la fila, salvo que sea cabecera, aviso o hueco.
fn index_click(show: &mut Showcase, area: Rect, row: u16) {
    let offset = usize::from(row - area.y);
    let (start, visible) = show.visible_index_rows(area.height);
    let total = show.index_rows().len();
    if index_notice(start, visible.len(), total, offset).is_some() {
        return;
    }
    if let Some(IndexRow {
        catalog_index: Some(index),
        ..
    }) = visible.get(offset)
    {
        show.select(*index, true);
    }
}

/// Clic en la matriz (solo con `Focus::Matrix`): una celda mueve el cursor a ella; el nombre de
/// una fila lo lleva a esa fila, en la columna en la que ya estaba. Nada abre.
fn matrix_click(show: &mut Showcase, content: Rect, column: u16, row: u16) {
    if let Some((matrix_row, matrix_column)) = matrix_cell_at(content, show, column, row) {
        show.matrix_set_cursor(matrix_row, matrix_column);
    } else if let Some(matrix_row) = matrix_name_at(content, show, column, row) {
        let (_, current_column) = show.matrix_cursor();
        show.matrix_set_cursor(matrix_row, current_column);
    }
}

/// Aviso que ocupa la fila `offset` (0 = primera visible) del índice, si la hay. `start` es el
/// primer visible, `visible` el número de filas visibles y `total` el de filas del índice.
/// El dibujo y el ratón usan esta misma regla: un aviso no es seleccionable.
fn index_notice(start: usize, visible: usize, total: usize, offset: usize) -> Option<&'static str> {
    if offset == 0 && start > 0 {
        return Some("  ↑ más resultados");
    }
    // La última fila visible avisa si queda lista por debajo. La condición anterior comparaba
    // `start + offset + 1` con `total` y a la vez exigía `start + visible < total`, dos cosas que
    // no pueden cumplirse juntas, de modo que este aviso no salía nunca.
    if offset + 1 == visible && start + visible < total {
        return Some("  ↓ más resultados");
    }
    None
}

fn draw_index(f: &mut Frame, area: Rect, show: &Showcase, theme: &Theme) {
    let (start, rows) = show.visible_index_rows(area.height);
    let all_rows = show.index_rows();
    let muted = theme.style(loomcoding_cloth::state::StyleToken::Muted);
    let mut lines = Vec::with_capacity(rows.len());
    for (offset, row) in rows.iter().enumerate() {
        if let Some(notice) = index_notice(start, rows.len(), all_rows.len(), offset) {
            lines.push(Line::styled(notice, muted));
            continue;
        }
        let mut spans = Vec::new();
        // El símbolo del componente va aparte, con su estilo de `draw_matrix`.
        let symbol = |row: &IndexRow| {
            let cell = if row.generic {
                Cell::Generic
            } else {
                Cell::Own(1)
            };
            let (glyph, style) = matrix::glyph(cell, theme);
            Span::styled(format!("{glyph} "), style)
        };
        match row.catalog_index {
            Some(i) => {
                let selected = i == show.selected();
                let focused = selected && show.focus() == Focus::Index;
                let marker = if focused {
                    "▸ "
                } else if selected {
                    "• "
                } else {
                    "  "
                };
                let mut style = theme.style(loomcoding_cloth::state::StyleToken::Default);
                if focused {
                    style = style.add_modifier(Modifier::BOLD);
                }
                spans.push(Span::styled(format!("  {marker}"), style));
                if row.marked {
                    spans.push(symbol(row));
                }
                spans.push(Span::styled(row.text.clone(), style));
            }
            None => {
                // Cabecera de componente con varios ejemplos, o el aviso «Sin resultados».
                spans.push(Span::styled("  ", muted));
                if row.marked {
                    spans.push(symbol(row));
                }
                spans.push(Span::styled(row.text.clone(), muted));
            }
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_header(f: &mut Frame, area: Rect, show: &Showcase, theme: &Theme) {
    let component = show.component_at(show.selected()).unwrap_or("");
    let variant = show.variant_at(show.selected()).unwrap_or("");
    let hint = show.current_hint();
    let lines = vec![
        Line::styled(
            component.to_string(),
            theme
                .style(loomcoding_cloth::state::StyleToken::Heading)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            format!("{variant} · {hint}"),
            theme.style(loomcoding_cloth::state::StyleToken::Muted),
        ),
        Line::styled(
            help(show.focus()),
            theme.style(loomcoding_cloth::state::StyleToken::Muted),
        ),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

/// Ayuda de teclas de una línea para cada foco; la usan la cabecera y la versión estrecha.
fn help(focus: Focus) -> &'static str {
    match focus {
        Focus::Index => "↑/↓: ejemplo · ←/→: tema · Tab: abrir · /: filtrar · m: matriz · q: salir",
        Focus::Demo => "F7: tema · Tab/Esc: volver",
        Focus::Matrix => "←/→/↑/↓: celda · Enter: abrir · Esc: índice · F7: paleta",
        Focus::TextFilter => "Escribe para filtrar · Enter: índice · Esc: limpiar",
    }
}

fn draw_collapsed(f: &mut Frame, area: Rect, show: &Showcase, theme: &Theme) {
    let component = show.component_at(show.selected()).unwrap_or("");
    let variant = show.variant_at(show.selected()).unwrap_or("");
    let lines = vec![
        Line::styled(
            component.to_string(),
            theme.style(loomcoding_cloth::state::StyleToken::Heading),
        ),
        Line::styled(
            format!("{variant} · {}", show.current_hint()),
            theme.style(loomcoding_cloth::state::StyleToken::Muted),
        ),
        Line::styled(
            help(show.focus()),
            theme.style(loomcoding_cloth::state::StyleToken::Muted),
        ),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

/// Textos del pie, una línea cada uno, para un área de `width` columnas.
///
/// La línea 1 da el tema, el recuento de sus componentes (propios `●`, genéricos `◐`) y cuántos
/// faltan. La línea 2 existe solo si el componente seleccionado es genérico: explica el estado.
/// El sufijo de F7 se añade solo si la línea 1 cabe entera con él.
fn footer_texts(show: &Showcase, width: u16) -> Vec<String> {
    let view = show.view();
    let generic = view.entries.iter().filter(|entry| entry.generic).count();
    let own = view.entries.len() - generic;
    let (name, _) = matrix::COLUMNS[show.theme_column()];
    let base = format!(
        "Tema: {name} · {} componentes: {own} ● · {generic} ◐ · faltan {} (m: matriz)",
        view.entries.len(),
        view.missing
    );
    let with_keys = format!("{base} · F7/Shift+F7 tema");
    let line = if with_keys.chars().count() <= usize::from(width) {
        with_keys
    } else {
        base
    };
    let mut lines = vec![matrix::fit(&line, usize::from(width))];
    if let Some(component) = footer_note_component(show) {
        let note = matrix::status_note(component, show.theme_column(), Cell::Generic);
        lines.push(matrix::fit(&note, usize::from(width)));
    }
    lines
}

/// Componente seleccionado si es genérico en el tema activo: solo entonces el pie tiene segunda
/// línea (la nota de estado). Es la única condición de la altura del pie.
fn footer_note_component(show: &Showcase) -> Option<&str> {
    let component = show.component_at(show.selected())?;
    let generic = show
        .view()
        .entries
        .iter()
        .find(|entry| entry.component == component)
        .is_some_and(|entry| entry.generic);
    generic.then_some(component)
}

/// Altura del pie, en filas: 2 si el componente seleccionado es genérico, 1 si no.
fn footer_height(show: &Showcase) -> u16 {
    if footer_note_component(show).is_some() {
        2
    } else {
        1
    }
}

/// Dibuja las líneas del pie, una por fila del área.
fn draw_footer(f: &mut Frame, area: Rect, lines: &[String], theme: &Theme) {
    let muted = theme.style(loomcoding_cloth::state::StyleToken::Muted);
    let rows: Vec<Line> = lines
        .iter()
        .map(|line| Line::styled(line.as_str(), muted))
        .collect();
    f.render_widget(Paragraph::new(rows), area);
}
