//! Tests: matrix.

use super::support::*;
use super::*;

#[test]
fn matrix_shows_the_three_symbols_and_all_columns() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    let buffer = render(&mut show, 100, 40);
    let lines = lines(&buffer, INDEX_WIDTH);
    assert!(
        lines.iter().any(|line| line.contains(HEADER)),
        "faltan los siete nombres de columna"
    );
    let table = matrix_table(&lines).join("\n");
    for symbol in ["●", "◐", "·"] {
        assert!(table.contains(symbol), "falta {symbol} en la matriz");
    }
}

#[test]
fn matrix_marks_the_cursor_column() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Crush");
    let buffer = render(&mut show, 100, 40);
    let reversed: Vec<(u16, u16)> = (INDEX_WIDTH..buffer.area.width)
        .flat_map(|x| (0..buffer.area.height).map(move |y| (x, y)))
        .filter(|&(x, y)| buffer[(x, y)].modifier.contains(Modifier::REVERSED))
        .collect();
    assert!(!reversed.is_empty(), "ninguna celda invertida");
    let y = reversed[0].1;
    assert!(reversed.iter().all(|&(_, row)| row == y));
    // La celda invertida es la de Composer en Crush: un demo propio, «●».
    assert_eq!(buffer[(reversed[0].0, y)].symbol(), "●");

    // El encabezado de Crush va resaltado en negrita.
    let header_y = lines(&buffer, INDEX_WIDTH)
        .iter()
        .position(|line| line.contains(HEADER))
        .expect("cabecera de columnas") as u16;
    let header = lines(&buffer, INDEX_WIDTH).swap_remove(usize::from(header_y));
    let crush_x = INDEX_WIDTH + header.find("Crush").expect("Crush en la cabecera") as u16;
    assert!(buffer[(crush_x, header_y)]
        .modifier
        .contains(Modifier::BOLD));
}

#[test]
fn matrix_scrolls_to_keep_the_cursor_visible() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    let rows = show.matrix().rows.len();
    let first = show.matrix().rows[0].component;
    let last = show.matrix().rows[rows - 1].component;
    for _ in 0..rows {
        press(&mut show, KeyCode::Char('j'));
    }
    let buffer = render(&mut show, 100, 40);
    let table = matrix_table(&lines(&buffer, INDEX_WIDTH)).to_vec();
    // Los nombres se recortan al ancho de la columna de nombre: se compara el prefijo.
    let prefix = |name: &str| name.chars().take(6).collect::<String>();
    assert!(
        table.iter().any(|line| line.starts_with(&prefix(last))),
        "la última fila ({last}) debe verse con el cursor al final"
    );
    assert!(
        table.iter().all(|line| !line.starts_with(&prefix(first))),
        "la primera fila ({first}) debe haber salido del área"
    );
}

#[test]
fn matrix_fits_in_a_narrow_terminal_without_panic() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    let buffer = render(&mut show, 60, 20);
    assert!(lines(&buffer, 0).iter().any(|line| line.contains(HEADER)));
    render(&mut show, 20, 5);
}

#[test]
fn footer_names_the_cell_under_the_cursor() {
    let mut show = Showcase::new();
    press(&mut show, KeyCode::Char('m'));
    place(&mut show, "Composer", "Pawan");
    let buffer = render(&mut show, 100, 40);
    assert!(
        lines(&buffer, INDEX_WIDTH)
            .iter()
            .any(|line| line.contains("Composer · Pawan: sin demo propio")),
        "el pie debe nombrar la celda bajo el cursor"
    );
}
