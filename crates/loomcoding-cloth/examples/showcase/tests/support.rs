//! Piezas comunes de los tests.

use super::*;

/// Cabecera de columnas de la matriz tal como se dibuja (sin el hueco del nombre).
pub(super) const HEADER: &str = "Cloth Martty Mindfork Pawan Crush MastraCode Loomcoding";

pub(super) fn press(show: &mut Showcase, code: KeyCode) {
    route(show, KeyEventKind::Press, code, KeyModifiers::NONE);
}

/// Dibuja el showcase completo en un buffer de `w`×`h`.
pub(super) fn render(show: &mut Showcase, w: u16, h: u16) -> Buffer {
    let anim = Anim::new();
    let theme = Theme::preset(show.theme_preset());
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, show, &anim, &theme)).unwrap();
    term.backend().buffer().clone()
}

/// Líneas de texto del buffer desde la columna `x0`.
pub(super) fn lines(buffer: &Buffer, x0: u16) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            (x0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

/// Filas de la matriz: las que hay entre la cabecera de columnas y la leyenda.
pub(super) fn matrix_table(lines: &[String]) -> &[String] {
    let head = lines
        .iter()
        .position(|line| line.contains(HEADER))
        .expect("cabecera de columnas");
    let legend = lines
        .iter()
        .position(|line| line.contains("genérico"))
        .expect("leyenda");
    &lines[head + 1..legend]
}

/// Mueve el cursor de la matriz a la celda (componente, columna), buscándolas por nombre.
pub(super) fn place(show: &mut Showcase, component: &str, column: &str) {
    let row = show
        .matrix()
        .rows
        .iter()
        .position(|row| row.component == component)
        .expect("fila del componente");
    let column = matrix::COLUMNS
        .iter()
        .position(|(name, _)| *name == column)
        .expect("columna");
    for _ in 0..1000 {
        press(show, KeyCode::Up);
        press(show, KeyCode::Left);
    }
    for _ in 0..row {
        press(show, KeyCode::Down);
    }
    for _ in 0..column {
        press(show, KeyCode::Right);
    }
}

/// Índice del demo del catálogo con ese componente y esa variante exacta.
pub(super) fn demo_index(show: &Showcase, component: &str, variant: &str) -> usize {
    (0..show.len())
        .find(|&i| show.component_at(i) == Some(component) && show.variant_at(i) == Some(variant))
        .expect("demo en el catálogo")
}

/// Índice de columna de `matrix::COLUMNS` por nombre.
pub(super) fn theme_index(name: &str) -> usize {
    matrix::COLUMNS
        .iter()
        .position(|(column, _)| *column == name)
        .expect("columna")
}

/// Texto de la columna del índice (las primeras `INDEX_WIDTH` columnas) de cada fila.
pub(super) fn index_text(buffer: &Buffer) -> Vec<String> {
    lines(buffer, 0)
        .into_iter()
        .map(|line| line.chars().take(usize::from(INDEX_WIDTH)).collect())
        .collect()
}

/// Primer demo del catálogo con ese componente y un proveedor dado como prefijo de variante.
pub(super) fn demo_of(show: &Showcase, component: &str, provider: &str) -> usize {
    (0..show.len())
        .find(|&i| {
            show.component_at(i) == Some(component)
                && show.variant_at(i).is_some_and(|v| v.starts_with(provider))
        })
        .expect("demo en el catálogo")
}

pub(super) const LEFT: MouseEventKind = MouseEventKind::Down(MouseButton::Left);

pub(super) const WHEEL_UP: MouseEventKind = MouseEventKind::ScrollUp;

pub(super) const WHEEL_DOWN: MouseEventKind = MouseEventKind::ScrollDown;

/// Pantalla de `w`×`h` celdas con origen en (0, 0), como la de `draw`.
pub(super) fn screen(w: u16, h: u16) -> Rect {
    Rect::new(0, 0, w, h)
}

/// Pulsación izquierda en (`x`, `y`) de una pantalla de `w`×`h`.
pub(super) fn click_at(show: &mut Showcase, x: u16, y: u16, w: u16, h: u16) {
    handle_mouse(show, LEFT, x, y, screen(w, h));
}

/// Showcase con el tema de la columna `theme` y el primer demo de `component` de ese proveedor.
pub(super) fn state(theme: &str, component: &str, provider: &str) -> Showcase {
    let mut show = Showcase::new();
    show.set_theme_column(theme_index(theme));
    show.select(demo_of(&show, component, provider), false);
    show
}

/// Columna de la primera celda visible de la pestaña `theme` en una fila de `width` columnas.
pub(super) fn tab_x(width: u16, theme: usize) -> u16 {
    tab_spans_layout(width)
        .iter()
        .find(|(column, _, _)| *column == theme)
        .map(|(_, x, _)| *x)
        .expect("pestaña visible")
}

/// Texto de la fila `y` del buffer entre las columnas `x0` (incluida) y `x1` (excluida).
pub(super) fn row_text(buffer: &Buffer, x0: u16, x1: u16, y: u16) -> String {
    (x0..x1).map(|x| buffer[(x, y)].symbol()).collect()
}

/// Fila de pantalla en la que el índice pinta `text`. Debe aparecer una sola vez.
pub(super) fn index_row_of(buffer: &Buffer, text: &str) -> u16 {
    let rows: Vec<usize> = index_text(buffer)
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(text))
        .map(|(y, _)| y)
        .collect();
    assert_eq!(rows.len(), 1, "«{text}» en el índice: filas {rows:?}");
    rows[0] as u16
}

/// Columna (en caracteres) en la que la fila `y` del índice pinta `text`.
pub(super) fn index_column_of(buffer: &Buffer, y: u16, text: &str) -> u16 {
    let line = &index_text(buffer)[usize::from(y)];
    let byte = line
        .find(text)
        .unwrap_or_else(|| panic!("«{text}» no está en la fila {y}: {line}"));
    line[..byte].chars().count() as u16
}

/// Fila seleccionable del índice visible en una pantalla `w`×`h`, distinta de la seleccionada,
/// con un texto corto que aparece una sola vez en el índice. Devuelve (índice del catálogo,
/// texto, fila de pantalla en la que debe pintarse).
pub(super) fn visible_target(show: &Showcase, w: u16, h: u16) -> (usize, String, u16) {
    let index = geometry(screen(w, h), footer_height(show))
        .index
        .expect("índice visible");
    let (start, visible) = show.visible_index_rows(index.height);
    let total = show.index_rows();
    visible
        .iter()
        .enumerate()
        .filter(|(offset, _)| index_notice(start, visible.len(), total.len(), *offset).is_none())
        .find_map(|(offset, row)| {
            let target = row.catalog_index.filter(|&i| i != show.selected())?;
            let text = row.text.trim().to_string();
            let unique = total
                .iter()
                .filter(|other| other.text.contains(&text))
                .count()
                == 1;
            (unique && text.chars().count() <= 20).then(|| (target, text, index.y + offset as u16))
        })
        .expect("fila seleccionable visible")
}

/// Celda invertida (la del cursor) dentro de `area`: su primera columna y su fila.
pub(super) fn reversed_cell(buffer: &Buffer, area: Rect) -> (u16, u16) {
    (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .find(|&(x, y)| buffer[(x, y)].modifier.contains(Modifier::REVERSED))
        .expect("celda invertida")
}

/// Fila de la matriz cuyo nombre empieza en la columna `x` de `area`, o `None`.
pub(super) fn matrix_row_of(buffer: &Buffer, area: Rect, name: &str) -> Option<u16> {
    (area.y..area.bottom()).find(|&y| row_text(buffer, area.x, area.right(), y).starts_with(name))
}
