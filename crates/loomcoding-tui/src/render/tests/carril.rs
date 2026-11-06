//! Carril del transcript configurable: símbolos por estilo, ancho que reserva y pintado en
//! pantalla. El árbol por defecto, con su trazo original, se prueba en `transcript_y_arbol.rs`.

use super::*;
use crate::render::rail::{posicion, rail_cells, rail_width, Posicion, RailStyle, MIN_WIDTH};

/// Los dos símbolos de la fila del carril, como texto.
fn celdas(style: RailStyle, pos: Posicion, enfocado: bool, glifo: Option<char>) -> String {
    rail_cells(style, pos, enfocado, glifo).iter().collect()
}

/// Fila de pantalla de la fila `dentro` del bloque `entrada`, según el último fotograma.
fn fila_de(app: &App, area: Rect, entrada: usize, dentro: usize) -> u16 {
    let linea = app.transcript.entry_start(entrada);
    area.y + (app.layout.row_of(linea) + dentro - app.viewport.top()) as u16
}

/// Fila de pantalla de la fila absoluta `row` del transcript.
fn fila_absoluta(app: &App, area: Rect, row: usize) -> u16 {
    area.y + (row - app.viewport.top()) as u16
}

/// Símbolos de las `ancho` columnas a la izquierda del texto en la fila `y`.
fn guia(term: &Terminal<TestBackend>, area: Rect, y: u16, ancho: u16) -> String {
    (area.x - ancho..area.x)
        .map(|x| term.backend().buffer()[(x, y)].symbol().to_string())
        .collect()
}

/// Texto de la fila `y` dentro del área de texto, sin el relleno final.
fn texto_de(term: &Terminal<TestBackend>, area: Rect, y: u16) -> String {
    (area.x..area.right())
        .map(|x| term.backend().buffer()[(x, y)].symbol().to_string())
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Conversación corta (sin ajuste de línea) con un usuario, un asistente de tres filas y otro usuario.
fn conversacion(style: RailStyle) -> App {
    let mut app = App::new();
    app.rail_style = style;
    app.push_user("pregunta");
    app.push_assistant("uno\ndos\ntres");
    app.push_user("otra");
    app
}

#[test]
fn tree_keeps_the_original_guide_in_every_row() {
    let t = RailStyle::Tree;
    assert_eq!(celdas(t, Posicion::Primera, false, Some('»')), "╭»");
    assert_eq!(celdas(t, Posicion::Intermedia, false, Some('»')), "│ ");
    assert_eq!(celdas(t, Posicion::Ultima, false, Some('»')), "╰─");
    assert_eq!(celdas(t, Posicion::Unica, false, Some('◌')), "─◌");
    assert_eq!(celdas(t, Posicion::Unica, false, None), "──");
    assert_eq!(celdas(t, Posicion::Primera, true, Some('≋')), "┏≋");
    assert_eq!(celdas(t, Posicion::Intermedia, true, None), "┃ ");
    assert_eq!(celdas(t, Posicion::Ultima, true, None), "┗━");
    assert_eq!(celdas(t, Posicion::Unica, true, None), "━━");
}

#[test]
fn dots_puts_the_role_glyph_on_the_header_and_dots_below_without_corners() {
    let d = RailStyle::Dots;
    assert_eq!(celdas(d, Posicion::Primera, false, Some('»')), "» ");
    assert_eq!(celdas(d, Posicion::Intermedia, false, Some('»')), "┆ ");
    assert_eq!(celdas(d, Posicion::Ultima, false, Some('»')), "┆ ");
    assert_eq!(celdas(d, Posicion::Unica, false, None), "┆ ");
    assert_eq!(celdas(d, Posicion::Intermedia, true, None), "┇ ");
    assert_eq!(celdas(d, Posicion::Ultima, true, None), "┇ ");
    assert_eq!(celdas(d, Posicion::Primera, true, Some('»')), "» ");
}

#[test]
fn bar_marks_only_the_focused_block_and_keeps_the_header_glyph() {
    let b = RailStyle::Bar;
    assert_eq!(celdas(b, Posicion::Primera, false, Some('»')), " »");
    assert_eq!(celdas(b, Posicion::Intermedia, false, Some('»')), "  ");
    assert_eq!(celdas(b, Posicion::Ultima, false, None), "  ");
    assert_eq!(celdas(b, Posicion::Primera, true, Some('»')), "▌»");
    assert_eq!(celdas(b, Posicion::Intermedia, true, None), "▌ ");
    assert_eq!(celdas(b, Posicion::Ultima, true, None), "▌ ");
}

#[test]
fn none_and_tint_draw_only_the_glyph_when_the_block_opens() {
    for style in [RailStyle::None, RailStyle::Tint] {
        assert_eq!(celdas(style, Posicion::Primera, false, Some('»')), "» ");
        assert_eq!(celdas(style, Posicion::Intermedia, false, Some('»')), "  ");
        assert_eq!(celdas(style, Posicion::Ultima, false, None), "  ");
        assert_eq!(celdas(style, Posicion::Unica, true, Some('◌')), "◌ ");
    }
}

#[test]
fn box_still_paints_like_tree_until_its_own_task_lands() {
    for pos in [
        Posicion::Primera,
        Posicion::Intermedia,
        Posicion::Ultima,
        Posicion::Unica,
    ] {
        for enfocado in [false, true] {
            for glifo in [Some('»'), None] {
                assert_eq!(
                    rail_cells(RailStyle::Box, pos, enfocado, glifo),
                    rail_cells(RailStyle::Tree, pos, enfocado, glifo),
                    "{pos:?} foco={enfocado} glifo={glifo:?}"
                );
            }
        }
    }
}

#[test]
fn posicion_follows_whether_the_row_opens_and_closes_its_block() {
    assert_eq!(posicion(true, true), Posicion::Unica);
    assert_eq!(posicion(true, false), Posicion::Primera);
    assert_eq!(posicion(false, true), Posicion::Ultima);
    assert_eq!(posicion(false, false), Posicion::Intermedia);
}

#[test]
fn the_rail_takes_no_room_below_forty_columns() {
    for style in RailStyle::ALL {
        assert_eq!(rail_width(style, MIN_WIDTH - 1), 0, "{style:?}");
        assert!(rail_width(style, MIN_WIDTH) > 0, "{style:?}");
    }
}

#[test]
fn the_width_is_three_columns_except_for_none_and_tint() {
    for style in [
        RailStyle::Tree,
        RailStyle::Dots,
        RailStyle::Bar,
        RailStyle::Box,
    ] {
        assert_eq!(rail_width(style, 100), 3, "{style:?}");
    }
    for style in [RailStyle::None, RailStyle::Tint] {
        assert_eq!(rail_width(style, 100), 2, "{style:?}");
    }
}

#[test]
fn names_round_trip_and_the_default_is_tree() {
    assert_eq!(RailStyle::default(), RailStyle::Tree);
    for style in RailStyle::ALL {
        assert_eq!(RailStyle::from_name(style.name()), Some(style));
    }
    assert_eq!(RailStyle::from_name("raro"), None);
    assert_eq!(RailStyle::from_name("Tree"), None);
}

#[test]
fn the_focused_block_stands_out_in_every_style() {
    for style in RailStyle::ALL {
        let mut app = conversacion(style);
        let _ = terminal_of(&mut app, 100, 30);
        app.focus = Focus::Transcript;
        app.block = Some(1);
        let term = terminal_of(&mut app, 100, 30);
        let area = app.transcript_area.expect("el transcript fija su área");
        let ancho = rail_width(style, 100);
        let (inicio, fin) = block_rows(&app, 1);
        let filas: Vec<String> = (inicio..fin)
            .map(|row| guia(&term, area, fila_absoluta(&app, area, row), ancho))
            .collect();
        let (primera, ultima) = (&filas[0], &filas[filas.len() - 1]);
        let interiores = &filas[1..filas.len() - 1];

        match style {
            RailStyle::Tree | RailStyle::Box => {
                assert!(primera.starts_with('┏'), "{style:?}: {primera:?}");
                assert!(interiores.iter().all(|f| f.starts_with('┃')), "{style:?}");
                assert!(ultima.starts_with('┗'), "{style:?}: {ultima:?}");
            }
            RailStyle::Dots => {
                assert!(primera.starts_with('≋'), "{style:?}: {primera:?}");
                assert!(interiores.iter().all(|f| f.starts_with('┇')), "{style:?}");
                assert!(ultima.starts_with('┇'), "{style:?}: {ultima:?}");
            }
            RailStyle::Bar => {
                assert!(
                    filas.iter().all(|f| f.starts_with('▌')),
                    "{style:?}: {filas:?}"
                );
            }
            RailStyle::None | RailStyle::Tint => {
                // Sin trazo: ningún símbolo grueso ni de punteado en el bloque enfocado.
                assert!(
                    filas
                        .iter()
                        .all(|f| !f.contains(['┃', '┇', '▌', '━', '┗', '┏'])),
                    "{style:?}: {filas:?}"
                );
            }
        }
    }
}

#[test]
fn changing_the_rail_only_moves_the_text_by_its_width() {
    let mut textos: Vec<Vec<String>> = Vec::new();
    for style in RailStyle::ALL {
        let mut app = conversacion(style);
        let term = terminal_of(&mut app, 100, 30);
        let area = app.transcript_area.expect("el transcript fija su área");
        textos.push(
            (area.y..area.bottom())
                .map(|y| texto_de(&term, area, y))
                .collect(),
        );
    }
    for (style, filas) in RailStyle::ALL.iter().zip(&textos).skip(1) {
        assert_eq!(filas, &textos[0], "{style:?}");
    }
}

#[test]
fn every_style_reserves_no_room_in_a_narrow_transcript() {
    let mut anchos = Vec::new();
    for style in RailStyle::ALL {
        let mut app = conversacion(style);
        let _ = terminal_of(&mut app, 30, 20);
        anchos.push(
            app.transcript_area
                .expect("el transcript fija su área")
                .width,
        );
    }
    assert!(anchos.iter().all(|ancho| *ancho == anchos[0]), "{anchos:?}");
}

#[test]
fn tint_fills_the_user_row_and_marks_the_assistant_with_a_bullet() {
    let mut app = conversacion(RailStyle::Tint);
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let fondo = app
        .theme
        .styles
        .selection
        .bg
        .expect("el tema trae fondo de selección");

    let y_usuario = fila_de(&app, area, 0, 0);
    // La cola de un glifo ancho (el «👤» del mensaje) no la reescribe el backend de pruebas: se salta.
    let mut x = area.x;
    while x < area.right() {
        let celda = &term.backend().buffer()[(x, y_usuario)];
        assert_eq!(celda.bg, fondo, "columna {x} de la fila del usuario");
        x += Span::raw(celda.symbol().to_string()).width().max(1) as u16;
    }
    // El carril del usuario no se tiñe: el fondo cubre solo el texto.
    assert_ne!(term.backend().buffer()[(area.x - 1, y_usuario)].bg, fondo);

    let y_asistente = fila_de(&app, area, 1, 0);
    assert_eq!(guia(&term, area, y_asistente, 2), "• ");
    assert_ne!(term.backend().buffer()[(area.x, y_asistente)].bg, fondo);
}

#[test]
fn the_dots_guide_spans_every_row_of_a_block() {
    let mut app = conversacion(RailStyle::Dots);
    let term = terminal_of(&mut app, 100, 30);
    let area = app.transcript_area.expect("el transcript fija su área");
    let (inicio, fin) = block_rows(&app, 1);
    assert!(fin - inicio > 2, "el bloque debería ocupar varias filas");

    assert_eq!(guia(&term, area, fila_de(&app, area, 1, 0), 3), "≋  ");
    for dentro in 1..fin - inicio {
        assert_eq!(
            guia(&term, area, fila_de(&app, area, 1, dentro), 3),
            "┆  ",
            "fila {dentro} del bloque"
        );
    }
}
