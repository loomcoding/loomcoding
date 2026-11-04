//! Tests: mouse.

use super::support::*;
use super::*;

#[test]
fn click_on_a_tab_switches_the_theme() {
    let pawan = theme_index("Pawan");
    let mut show = Showcase::new();
    show.open_matrix();
    click_at(&mut show, tab_x(100, pawan) + 1, 0, 100, 30);
    assert_eq!(show.theme_column(), pawan);
    assert_eq!(show.focus(), Focus::Index);

    // El hueco entre dos pestañas no hace nada: el tema y el foco se quedan como estaban.
    let mut show = Showcase::new();
    show.open_matrix();
    let gap = tab_x(100, 0) + tab_width("Cloth");
    click_at(&mut show, gap, 0, 100, 30);
    assert_eq!(show.theme_column(), 0);
    assert_eq!(show.focus(), Focus::Matrix);

    // A 40 columnas, Crush se ve recortada a 3 celdas y MastraCode ya no aparece: la recortada
    // sí responde en lo que se ve.
    let crush = theme_index("Crush");
    let mastra = theme_index("MastraCode");
    assert!(tab_spans_layout(40)
        .iter()
        .all(|(column, _, _)| *column != mastra));
    let mut show = Showcase::new();
    click_at(&mut show, tab_x(40, crush) + 2, 0, 40, 30);
    assert_eq!(show.theme_column(), crush);
}

#[test]
fn click_on_an_index_row_selects_and_opens_it() {
    let mut show = Showcase::new();
    let (target, text, y) = visible_target(&show, 100, 30);
    let buffer = render(&mut show, 100, 30);
    assert_eq!(index_row_of(&buffer, &text), y, "fila pintada de «{text}»");
    click_at(&mut show, 1, y, 100, 30);
    assert_eq!(show.selected(), target);
    assert_eq!(show.focus(), Focus::Demo);
}

#[test]
fn click_on_a_child_example_selects_that_example() {
    // Crush: «Assistant message» tiene filas hijas. Partimos de «pending» para que la fila
    // «refusal», vecina, quede visible sin depender del desplazamiento.
    let mut show = state("Crush", "Assistant message", "Crush");
    let refusal = (0..show.len())
        .find(|&i| {
            show.component_at(i) == Some("Assistant message")
                && show.variant_at(i).is_some_and(|v| v.ends_with("refusal"))
        })
        .expect("demo refusal");
    let pending = (0..show.len())
        .find(|&i| {
            show.component_at(i) == Some("Assistant message")
                && show.variant_at(i).is_some_and(|v| v.ends_with("pending"))
        })
        .expect("demo pending");
    show.select(pending, false);
    let buffer = render(&mut show, 100, 30);
    let y = index_row_of(&buffer, "refusal");
    click_at(&mut show, 5, y, 100, 30);
    assert_eq!(show.selected(), refusal);
    assert_eq!(show.focus(), Focus::Demo);
}

#[test]
fn click_on_a_header_row_does_nothing() {
    let mut show = state("Crush", "Assistant message", "Crush");
    let before = show.selected();
    let buffer = render(&mut show, 100, 30);
    let y = index_row_of(&buffer, "Assistant message ─");
    click_at(&mut show, 1, y, 100, 30);
    assert_eq!(show.selected(), before);
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn wheel_scrolls_the_index_with_a_cap() {
    let mut show = Showcase::new();
    let list = show.filtered_indices();
    assert!(
        list.len() > WHEEL_STEPS,
        "hacen falta más de {WHEEL_STEPS} filas"
    );
    // Con el foco en el demo, la rueda sobre el índice lo devuelve al índice.
    show.select(list[0], true);
    handle_mouse(&mut show, WHEEL_DOWN, 1, 10, screen(100, 30));
    assert_eq!(show.selected(), list[WHEEL_STEPS]);
    assert_eq!(show.focus(), Focus::Index);
    handle_mouse(&mut show, WHEEL_UP, 1, 10, screen(100, 30));
    assert_eq!(show.selected(), list[0]);
    // Con tope: en el último elemento sigue en el último, y el foco no cambia.
    for _ in 0..list.len() {
        handle_mouse(&mut show, WHEEL_DOWN, 1, 10, screen(100, 30));
    }
    assert_eq!(show.selected(), *list.last().expect("filas"));
    handle_mouse(&mut show, WHEEL_DOWN, 1, 10, screen(100, 30));
    assert_eq!(show.selected(), *list.last().expect("filas"));
    assert_eq!(show.focus(), Focus::Index);
}

#[test]
fn wheel_on_the_matrix_moves_the_cursor() {
    let mut show = Showcase::new();
    show.open_matrix();
    // Dentro de la tabla y fuera de ella, la rueda mueve la fila igual.
    handle_mouse(&mut show, WHEEL_DOWN, 50, 10, screen(100, 30));
    assert_eq!(show.matrix_cursor(), (1, 0));
    handle_mouse(&mut show, WHEEL_DOWN, 1, 29, screen(100, 30));
    assert_eq!(show.matrix_cursor(), (2, 0));
    handle_mouse(&mut show, WHEEL_UP, 50, 2, screen(100, 30));
    assert_eq!(show.matrix_cursor(), (1, 0));
    // Sobre el índice, con la matriz abierta, también mueve el cursor de la matriz.
    handle_mouse(&mut show, WHEEL_DOWN, 1, 10, screen(100, 30));
    assert_eq!(show.matrix_cursor(), (2, 0));
    assert_eq!(show.focus(), Focus::Matrix);
}

#[test]
fn click_on_a_matrix_cell_moves_the_cursor_and_keeps_the_matrix() {
    let mut show = Showcase::new();
    show.open_matrix();
    let buffer = render(&mut show, 100, 30);
    let content = geometry(screen(100, 30), footer_height(&show)).content;
    // La fila de Composer y la cabecera de Crush, tal como se pintan.
    let composer_y = matrix_row_of(&buffer, content, "Composer").expect("fila de Composer");
    let header = row_text(&buffer, content.x, content.right(), content.y);
    let crush_x = content.x + header.find("Crush").expect("cabecera de Crush") as u16;
    let composer = show
        .matrix()
        .rows
        .iter()
        .position(|row| row.component == "Composer")
        .expect("fila");
    click_at(&mut show, crush_x + 1, composer_y, 100, 30);
    assert_eq!(show.matrix_cursor(), (composer, theme_index("Crush")));
    assert_eq!(show.focus(), Focus::Matrix);
    assert_eq!(
        show.theme_column(),
        0,
        "un clic en una celda no cambia el tema"
    );
}

#[test]
fn click_outside_the_matrix_does_not_close_it() {
    let mut show = Showcase::new();
    show.open_matrix();
    show.matrix_set_cursor(3, 2);
    let buffer = render(&mut show, 100, 30);
    let content = geometry(screen(100, 30), footer_height(&show)).content;
    let legend_y = (content.y..content.bottom())
        .find(|&y| row_text(&buffer, content.x, content.right(), y).contains("genérico"))
        .expect("leyenda");
    // Leyenda, pie y cabecera del panel: fuera de la tabla.
    click_at(&mut show, 60, legend_y, 100, 30);
    assert_eq!(show.focus(), Focus::Matrix);
    click_at(&mut show, 50, 29, 100, 30);
    assert_eq!(show.focus(), Focus::Matrix);
    click_at(&mut show, 60, 3, 100, 30);
    assert_eq!(show.focus(), Focus::Matrix);
    assert_eq!(show.matrix_cursor(), (3, 2));
}

#[test]
fn click_on_the_demo_panel_focuses_the_demo() {
    let mut show = Showcase::new();
    click_at(&mut show, 60, 10, 100, 30);
    assert_eq!(show.focus(), Focus::Demo);
    // En terminal estrecha no hay índice: el panel ocupa todo el cuerpo y también enfoca.
    let mut show = Showcase::new();
    click_at(&mut show, 30, 10, 60, 20);
    assert_eq!(show.focus(), Focus::Demo);
}

#[test]
fn mouse_and_draw_share_the_same_geometry() {
    // Pawan con Composer seleccionado tiene pie de 2 líneas; Crush con Composer propio, de 1.
    for (w, h) in [(100u16, 30u16), (80, 24), (70, 20), (60, 20)] {
        for (theme, provider, footer) in [("Pawan", "Cloth", 2u16), ("Crush", "Crush", 1)] {
            let label = format!("{theme} {w}x{h}");
            let mut show = state(theme, "Composer", provider);
            assert_eq!(footer_height(&show), footer, "pie de {label}");

            // Índice (solo con terminal ancha): la fila pintada de un ejemplo es la que se
            // selecciona al hacer clic en su texto.
            if w >= WIDE_MIN {
                let (target, text, y) = visible_target(&show, w, h);
                let buffer = render(&mut show, w, h);
                assert!(
                    index_text(&buffer)[usize::from(y)].contains(&text),
                    "{label}: «{text}» no se pinta en la fila {y}"
                );
                let x = index_column_of(&buffer, y, &text);
                click_at(&mut show, x + 1, y, w, h);
                assert_eq!(show.selected(), target, "{label}: clic en «{text}»");
            }

            // Matriz (todas las anchuras): la celda invertida pintada es la que el ratón
            // reconoce. Con el cursor en la última fila, su posición depende de la altura del
            // pie, así que un desfase movería el cursor.
            let mut show = state(theme, "Composer", provider);
            show.open_matrix();
            let last = show.matrix().rows.len() - 1;
            show.matrix_set_cursor(last, 2);
            let buffer = render(&mut show, w, h);
            let content = geometry(screen(w, h), footer_height(&show)).content;
            let (x, y) = reversed_cell(&buffer, content);
            click_at(&mut show, x, y, w, h);
            assert_eq!(show.matrix_cursor(), (last, 2), "{label}: celda pintada");
            let header = row_text(&buffer, content.x, content.right(), content.y);
            let crush_x = content.x + header.find("Crush").expect("cabecera") as u16;
            click_at(&mut show, crush_x, y, w, h);
            assert_eq!(
                show.matrix_cursor(),
                (last, theme_index("Crush")),
                "{label}: columna pintada"
            );
        }
    }
}
