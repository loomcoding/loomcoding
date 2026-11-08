//! Vistas de `/help`: cada una se pinta a 40×12 y a 80×24 sin perder texto; por debajo de 40×10
//! avisa de que hay que agrandar el terminal; y el compositor no cambia con la ayuda abierta.

use super::*;
use crate::commands::Command;
use crate::help_topics::{HelpView, TOPICS};
use ratatui::backend::TestBackend;

/// Filas de la pantalla pintada a `w`×`h`.
fn filas(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

fn en_vista(vista: HelpView) -> App {
    let mut app = App::new();
    app.help_view = vista;
    app.help_open = true;
    app
}

/// Texto del recuadro con los bordes quitados y los espacios colapsados: el ajuste de línea parte
/// las frases en varias filas, así que las aserciones miran la secuencia de palabras.
fn texto_plano(filas: &[String]) -> String {
    filas
        .iter()
        .map(|fila| {
            fila.chars()
                .map(|c| {
                    if "│╭╮╰╯─".contains(c) {
                        ' '
                    } else {
                        c
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn la_vista_de_un_comando_muestra_uso_ejemplo_y_pagina_a_40x12_y_80x24() {
    for (w, h) in [(40, 12), (80, 24)] {
        let mut app = en_vista(HelpView::Command(Command::Model));
        let texto = texto_plano(&filas(&mut app, w, h));
        for needle in [
            "Change the model",
            "Usage: /model [<id>]",
            "Example: /model <id>",
            "Docs: docs/user/en/reference/commands.md",
        ] {
            assert!(
                texto.contains(needle),
                "{w}×{h}: falta {needle:?} en:\n{texto}"
            );
        }
    }
}

#[test]
fn ninguna_linea_de_la_vista_de_comando_se_pierde_a_40_columnas() {
    let mut app = en_vista(HelpView::Command(Command::Ui));
    let texto = texto_plano(&filas(&mut app, 40, 12));
    assert!(texto.contains("Usage: /ui [<row> [value]]"), "{texto}");
    assert!(texto.contains("Example: /ui mouse off"), "{texto}");
}

#[test]
fn el_indice_de_temas_lista_cada_tema_y_su_pie() {
    let mut app = en_vista(HelpView::Topics);
    let texto = texto_plano(&filas(&mut app, 80, 24));
    for (id, _, _) in TOPICS {
        assert!(texto.contains(id), "falta el tema {id}:\n{texto}");
    }
    assert!(texto.contains("F1/Esc close · Tab shortcuts"), "{texto}");
}

#[test]
fn la_vista_de_un_tema_enlaza_su_pagina() {
    let indice = TOPICS.iter().position(|(id, _, _)| *id == "flows").unwrap();
    let mut app = en_vista(HelpView::Topic(indice));
    let texto = texto_plano(&filas(&mut app, 80, 24));
    assert!(texto.contains("ODD flow modes"), "{texto}");
    assert!(
        texto.contains("Docs: docs/user/en/concepts/flows-odd.md"),
        "{texto}"
    );
}

#[test]
fn por_debajo_de_40_columnas_o_10_filas_pide_agrandar_el_terminal() {
    for (w, h) in [(39, 12), (40, 9)] {
        let mut app = en_vista(HelpView::Topics);
        let texto = texto_plano(&filas(&mut app, w, h));
        assert!(texto.contains("enlarge terminal"), "{w}×{h}:\n{texto}");
    }
}

#[test]
fn los_atajos_nombran_bien_enter_y_ctrl_j_y_el_tab() {
    let mut app = en_vista(HelpView::Shortcuts);
    let texto = texto_plano(&filas(&mut app, 80, 24));
    assert!(
        texto.contains("Send prompt · Ctrl+J inserts a newline"),
        "{texto}"
    );
    assert!(!texto.contains("Enter/Ctrl+J"), "{texto}");
    assert!(texto.contains("F1/Esc close · Tab topics"), "{texto}");
}

#[test]
fn el_compositor_no_cambia_con_la_ayuda_abierta() {
    let mut app = App::new();
    app.composer.set_text("hola");
    let antes = filas(&mut app, 100, 30);
    app.help_open = true;
    app.help_view = HelpView::Topics;
    let con_ayuda = filas(&mut app, 100, 30);
    // Las filas de abajo quedan fuera del recuadro: el composer y el pie deben pintarse igual.
    assert_eq!(antes[26..], con_ayuda[26..]);
    assert!(con_ayuda[26..].iter().any(|fila| fila.contains("hola")));
}
