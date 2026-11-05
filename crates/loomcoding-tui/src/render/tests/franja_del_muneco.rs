//! Franja del muñeco de estado sobre el compositor: presencia, postura, `NO_COLOR` y espacio.
//!
//! Cabe señalar que las pruebas pasan el booleano de braille directamente a `draw_strip`, para
//! que el resultado no dependa de la variable `NO_COLOR` del entorno donde se ejecutan.

use super::*;
use crate::render::mascot_strip::{braille_allowed, draw_strip, strip_rows};
use loomcoding_cloth::state::ToolItem;
use ratatui::layout::Rect;
use std::time::Instant;

/// Herramienta en marcha con el nombre indicado.
fn running(name: &str) -> ToolItem {
    let mut tool = ToolItem::new("call", name);
    tool.start();
    tool
}

/// Dibuja solo la franja en un terminal de `w`×`h`, con el braille indicado.
fn strip_of(app: &App, w: u16, h: u16, braille: bool) -> Terminal<TestBackend> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, w, h);
        draw_strip(f, area, app, Instant::now(), braille);
    })
    .unwrap();
    term
}

/// Símbolo de cada celda, fila a fila.
fn rows_of(term: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = term.backend().buffer();
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        })
        .collect()
}

/// Indica si alguna celda del terminal contiene un carácter braille (U+2800..U+28FF).
fn has_braille(term: &Terminal<TestBackend>) -> bool {
    rows_of(term)
        .iter()
        .any(|row| row.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)))
}

#[test]
fn strip_is_absent_when_idle() {
    let app = App::new();
    assert_eq!(strip_rows(&app, Instant::now(), true), 0);
    let term = terminal_of(&mut App::new(), 100, 30);
    assert!(!has_braille(&term), "reposo sin braille");
}

#[test]
fn a_running_tool_draws_the_braille_mascot() {
    let mut app = App::new();
    app.tools.push_back(running("bash"));
    assert_eq!(strip_rows(&app, Instant::now(), true), 3);
    let term = strip_of(&app, 60, 3, true);
    assert!(has_braille(&term), "el muñeco debe usar braille");
    assert!(rows_of(&term).join("\n").contains("running"));
}

#[test]
fn a_search_tool_adds_the_lens() {
    let mut app = App::new();
    app.tools.push_back(running("grep"));
    let term = strip_of(&app, 60, 3, true);
    let texto = rows_of(&term).join("\n");
    assert!(texto.contains('⌕'), "la lupa acompaña a la búsqueda");
    assert!(texto.contains("searching"));
}

#[test]
fn mascot_off_draws_a_single_text_line() {
    let mut app = App::new();
    app.mascot = false;
    app.tools.push_back(running("bash"));
    assert_eq!(strip_rows(&app, Instant::now(), true), 1);
    let term = strip_of(&app, 60, 1, true);
    assert!(!has_braille(&term), "apagado no hay braille");
    assert!(rows_of(&term)[0].contains("running"));
}

#[test]
fn no_color_draws_no_braille() {
    let mut app = App::new();
    app.tools.push_back(running("bash"));
    assert_eq!(strip_rows(&app, Instant::now(), false), 1);
    let term = strip_of(&app, 60, 1, false);
    assert!(!has_braille(&term), "sin braille no hay muñeco");
    assert!(rows_of(&term)[0].contains("running"));
}

#[test]
fn the_strip_sits_above_the_composer_when_it_fits() {
    let mut app = App::new();
    app.tools.push_back(running("bash"));
    let term = terminal_of(&mut app, 100, 30);
    // El dibujo completo lee `NO_COLOR` por su cuenta: se compara con lo que decide esa función.
    assert_eq!(has_braille(&term), braille_allowed());
}

#[test]
fn the_strip_is_dropped_when_it_does_not_fit() {
    // En reposo, a 40×10, el transcript ya mide lo que mide: la franja no le quita nada.
    let mut idle = App::new();
    let _ = terminal_of(&mut idle, 40, 10);
    let idle_height = idle.transcript_area.map(|area| area.height);

    let mut app = App::new();
    app.tools.push_back(running("bash"));
    let term = terminal_of(&mut app, 40, 10);
    assert_eq!(app.transcript_area.map(|area| area.height), idle_height);
    assert!(!has_braille(&term), "no cabe, no hay franja");
}

#[test]
fn the_transcript_keeps_at_least_three_rows_when_the_strip_fits() {
    let mut app = App::new();
    app.tools.push_back(running("bash"));
    let _ = terminal_of(&mut app, 100, 30);
    let texto = app.transcript_area.expect("el transcript se dibuja");
    assert!(texto.height >= 3, "transcript con {} filas", texto.height);
}
