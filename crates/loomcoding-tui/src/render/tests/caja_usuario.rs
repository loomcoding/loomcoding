//! Caja redonda del mensaje del usuario en el carril `box`: bordes en las columnas esperadas, texto
//! que no pisa el borde al ajustarse, bloques sin trazo, alturas que vuelven a su sitio al cambiar
//! de carril y ancla de scroll sin pánico. Todo sobre `TestBackend`.

use super::*;
use crate::render::rail::{RailStyle, MIN_WIDTH};

/// Los símbolos de la fila `y` entre las columnas `desde` (incluida) y `hasta` (excluida).
fn simbolos(term: &Terminal<TestBackend>, desde: u16, hasta: u16, y: u16) -> String {
    (desde..hasta)
        .map(|x| term.backend().buffer()[(x, y)].symbol().to_string())
        .collect()
}

/// Símbolo de la celda `(x, y)`.
fn celda(term: &Terminal<TestBackend>, x: u16, y: u16) -> String {
    term.backend().buffer()[(x, y)].symbol().to_string()
}

/// Fila de pantalla de la fila `dentro` del bloque `entrada`, según el último fotograma.
fn fila_de(app: &App, area: Rect, entrada: usize, dentro: usize) -> u16 {
    let linea = app.transcript.entry_start(entrada);
    area.y + (app.layout.row_of(linea) + dentro - app.viewport.top()) as u16
}

/// Aplicación con el carril `box` y un mensaje del usuario de una sola línea.
fn un_usuario(texto: &str) -> App {
    let mut app = App::new();
    app.set_rail(RailStyle::Box);
    app.push_user(texto);
    app
}

#[test]
fn una_linea_de_usuario_lleva_caja_de_tres_filas() {
    let mut app = un_usuario("pregunta");
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (inicio, fin) = block_rows(&app, 0);
    assert_eq!(fin - inicio, 3, "etiqueta, texto y borde inferior");

    let y = fila_de(&app, area, 0, 0);
    // Borde izquierdo en x0 = area.x - 3; el prompt en x0 + 2, junto al texto.
    assert_eq!(simbolos(&term, area.x - 3, area.x, y), "╭──");
    assert_eq!(simbolos(&term, area.x - 3, area.x, y + 1), "│ »");
    assert_eq!(simbolos(&term, area.x - 3, area.x, y + 2), "╰──");
    assert_eq!(simbolos(&term, area.x, area.x + 8, y + 1), "pregunta");

    // Relleno en la columna siguiente al texto y borde en la de después.
    let dr = area.right();
    assert_eq!(celda(&term, dr, y), "─");
    assert_eq!(celda(&term, dr + 1, y), "╮");
    assert_eq!(celda(&term, dr + 1, y + 1), "│");
    assert_eq!(celda(&term, dr, y + 1), " ");
    assert_eq!(celda(&term, dr + 1, y + 2), "╯");
    assert_eq!(celda(&term, dr, y + 2), "─");
}

#[test]
fn el_texto_ajustado_no_pisa_los_bordes() {
    let mut app = un_usuario(&"palabra ".repeat(20));
    let term = terminal_of(&mut app, 60, 40);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (inicio, fin) = block_rows(&app, 0);
    assert!(
        fin - inicio > 3,
        "el mensaje debería ocupar varias filas de texto"
    );

    let dr = area.right();
    // Cuerpo: `│` a la izquierda y a la derecha, y la columna de relleno siempre vacía.
    for fila in inicio + 1..fin - 1 {
        let y = fila_de(&app, area, 0, fila - inicio);
        assert_eq!(celda(&term, area.x - 3, y), "│", "fila {fila} del cuerpo");
        assert_eq!(celda(&term, dr + 1, y), "│", "fila {fila} del cuerpo");
        assert_eq!(
            celda(&term, dr, y),
            " ",
            "fila {fila}: el texto pisa el relleno"
        );
    }
    let y_final = fila_de(&app, area, 0, fin - 1 - inicio);
    assert_eq!(celda(&term, area.x - 3, y_final), "╰");
    assert_eq!(celda(&term, dr + 1, y_final), "╯");
}

#[test]
fn el_asistente_no_lleva_caja_ni_trazo() {
    let mut app = App::new();
    app.set_rail(RailStyle::Box);
    app.push_user("pregunta");
    app.push_assistant("uno\ndos");
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (inicio, fin) = block_rows(&app, 1);
    let dr = area.right();

    for fila in inicio..fin {
        let y = fila_de(&app, area, 1, fila - inicio);
        let guia = simbolos(&term, area.x - 3, area.x, y);
        assert!(
            !guia.contains(['╭', '╮', '╰', '╯', '│', '─']),
            "fila {fila}: {guia:?}"
        );
        assert_eq!(
            celda(&term, dr + 1, y),
            " ",
            "fila {fila} sin borde derecho"
        );
    }
    // El glifo del rol sigue abriendo el bloque, en la primera columna del carril; el texto va en
    // la fila siguiente a la etiqueta, en la misma columna que el del usuario.
    let y0 = fila_de(&app, area, 1, 0);
    assert_eq!(simbolos(&term, area.x - 3, area.x, y0), "≋  ");
    assert_eq!(
        simbolos(&term, area.x, area.x + 3, fila_de(&app, area, 1, 1)),
        "uno"
    );
}

#[test]
fn cambiar_de_box_a_tree_y_volver_restaura_las_alturas() {
    let mut app = App::new();
    app.push_user("pregunta");
    app.push_assistant("uno\ndos\ntres");
    app.push_user("otra");

    app.set_rail(RailStyle::Box);
    let _ = terminal_of(&mut app, 100, 30);
    let total_box = app.layout.total_rows();

    app.set_rail(RailStyle::Tree);
    let _ = terminal_of(&mut app, 100, 30);
    let total_tree = app.layout.total_rows();

    app.set_rail(RailStyle::Box);
    let _ = terminal_of(&mut app, 100, 30);
    assert_eq!(app.layout.total_rows(), total_box);

    // Cada mensaje del usuario gana exactamente una fila: su borde inferior.
    assert_eq!(total_box, total_tree + 2);
}

#[test]
fn con_scroll_parcial_los_bordes_caen_donde_corresponde() {
    let mut app = App::new();
    app.set_rail(RailStyle::Box);
    app.push_user("uno\ndos\ntres");
    app.push_assistant(&"fila\n".repeat(40));
    let _ = terminal_of(&mut app, 100, 12);
    let (inicio, fin) = block_rows(&app, 0);
    assert_eq!(fin - inicio, 5, "etiqueta, tres líneas y borde");

    // La vista empieza dos filas dentro de la caja: las visibles son dos de cuerpo y el borde.
    // Hay que salir del modo «seguir al final» antes de bajar, o el siguiente fotograma lo vuelve a subir.
    app.viewport.to_top();
    app.viewport.scroll_down(inicio + 2);
    let term = terminal_of(&mut app, 100, 12);
    let area = app.transcript_area.expect("el transcript fija su área");
    let dr = area.right();
    assert_eq!(
        app.viewport.top(),
        inicio + 2,
        "la vista se queda donde se pidió"
    );

    assert_eq!(celda(&term, area.x - 3, area.y), "│");
    assert_eq!(celda(&term, dr + 1, area.y), "│");
    assert_eq!(celda(&term, area.x - 3, area.y + 1), "│");
    assert_eq!(celda(&term, area.x - 3, area.y + 2), "╰");
    assert_eq!(celda(&term, dr + 1, area.y + 2), "╯");
    // Ni el borde superior ni el prompt, que están por encima de la vista.
    assert!(!simbolos(&term, area.x - 3, area.right() + 2, area.y).contains('╭'));
}

#[test]
fn por_debajo_de_cuarenta_columnas_no_hay_caja_ni_reserva() {
    // El panel de la conversación ya usa esquinas redondas: se compara el ancho del texto con el
    // del carril `tree`, que a esa anchura tampoco reserva nada.
    let mut tree = un_usuario("pregunta");
    tree.set_rail(RailStyle::Tree);
    let _ = terminal_of(&mut tree, MIN_WIDTH - 1, 20);
    let ancho_tree = tree
        .transcript_area
        .expect("el transcript fija su área")
        .width;

    let mut app = un_usuario("pregunta");
    let _ = terminal_of(&mut app, MIN_WIDTH - 1, 20);
    assert!(!app.theme.rail_box, "sin carril no hay caja en el tema");
    assert_eq!(app.transcript_area.expect("área").width, ancho_tree);
}

#[test]
fn con_cuarenta_columnas_o_mas_la_caja_reserva_dos_columnas() {
    let mut tree = un_usuario("pregunta");
    tree.set_rail(RailStyle::Tree);
    let _ = terminal_of(&mut tree, 100, 20);
    let ancho_tree = tree
        .transcript_area
        .expect("el transcript fija su área")
        .width;

    let mut app = un_usuario("pregunta");
    let term = terminal_of(&mut app, 100, 20);
    assert!(app.theme.rail_box);
    let area = app.transcript_area.expect("el transcript fija su área");
    assert_eq!(
        area.width,
        ancho_tree - 2,
        "el texto deja dos columnas a la derecha"
    );
    assert_eq!(
        celda(&term, area.right() + 1, fila_de(&app, area, 0, 0)),
        "╮"
    );
}

#[test]
fn el_bloque_enfocado_usa_trazo_grueso() {
    let mut app = un_usuario("pregunta");
    let _ = terminal_of(&mut app, 100, 30);
    app.focus = Focus::Transcript;
    app.block = Some(0);
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (inicio, _) = block_rows(&app, 0);
    let dr = area.right();
    let y = area.y + (inicio - app.viewport.top()) as u16;

    assert_eq!(simbolos(&term, area.x - 3, area.x, y), "┏━━");
    assert_eq!(simbolos(&term, area.x - 3, area.x, y + 1), "┃ »");
    assert_eq!(simbolos(&term, area.x - 3, area.x, y + 2), "┗━━");
    assert_eq!(celda(&term, dr + 1, y), "┓");
    assert_eq!(celda(&term, dr + 1, y + 2), "┛");
}
