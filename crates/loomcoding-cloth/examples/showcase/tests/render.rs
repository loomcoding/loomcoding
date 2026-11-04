//! Tests: render.

use super::support::*;
use super::*;

#[test]
fn draw_does_not_panic_at_any_size() {
    let anim = Anim::new();
    let mut show = Showcase::new();
    let theme = Theme::preset(show.theme_preset());
    for (w, h) in [(0u16, 0u16), (1, 1), (60, 20), (120, 40)] {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| draw(f, &mut show, &anim, &theme)).unwrap();
    }
}

#[test]
fn footer_stays_one_line_without_a_row() {
    let mut show = Showcase::new();
    // dotmax es librería: el componente no tiene fila en la matriz.
    show.select(
        demo_index(&show, "Braille graphics", "dotmax · visual lab"),
        false,
    );
    let buffer = render(&mut show, 100, 30);
    let lines = lines(&buffer, 0);
    // ADAPTADO: el pie nuevo empieza por el tema; sin fila en la matriz no hay línea de nota.
    assert!(
        lines[29].starts_with("Tema: Cloth · "),
        "pie de una línea: {}",
        lines[29]
    );
    assert!(lines.iter().all(|line| !line.contains("parcialmente")));
    assert!(lines.iter().all(|line| !line.contains(" · Martty ●")));
}

#[test]
fn tabs_show_the_seven_themes_and_mark_the_active() {
    let mut show = Showcase::new();
    let buffer = render(&mut show, 100, 30);
    let row = lines(&buffer, 0).swap_remove(0);
    for (name, _) in matrix::COLUMNS {
        assert!(row.contains(name), "falta la pestaña {name}: {row}");
    }
    let bold = |buffer: &Buffer, line: &str, name: &str| {
        let x = line.find(name).expect("pestaña") as u16;
        buffer[(x, 0)].modifier.contains(Modifier::BOLD)
    };
    assert!(bold(&buffer, &row, "Cloth"));
    assert!(!bold(&buffer, &row, "Crush"));

    show.set_theme_column(theme_index("Crush"));
    let buffer = render(&mut show, 100, 30);
    let row = lines(&buffer, 0).swap_remove(0);
    assert!(bold(&buffer, &row, "Crush"));
    assert!(!bold(&buffer, &row, "Cloth"));
}

#[test]
fn index_marks_generic_components() {
    // Pawan: Composer es genérico (◐) y Transcript tiene demo propio (●).
    let mut show = Showcase::new();
    show.set_theme_column(theme_index("Pawan"));
    let buffer = render(&mut show, 100, 60);
    let index = index_text(&buffer);
    let composer = index
        .iter()
        .find(|l| l.contains("Composer"))
        .expect("Composer");
    assert!(composer.contains('◐'), "Composer sin ◐: {composer}");
    let transcript = index
        .iter()
        .find(|l| l.contains("Transcript"))
        .expect("Transcript");
    assert!(transcript.contains('●'), "Transcript sin ●: {transcript}");
}

#[test]
fn index_indents_child_examples() {
    // Crush: Assistant message tiene varios ejemplos, con filas hijas sangradas bajo la cabecera.
    let mut show = Showcase::new();
    show.set_theme_column(theme_index("Crush"));
    let buffer = render(&mut show, 100, 60);
    let index = index_text(&buffer);
    let header = index
        .iter()
        .find(|l| l.contains("Assistant message ─"))
        .expect("cabecera de Assistant message");
    assert!(header.contains('●'), "cabecera sin ●: {header}");
    // Columna en caracteres (no en bytes: «●» ocupa tres bytes) donde empieza un texto.
    let column_of =
        |line: &str, needle: &str| line.find(needle).map(|byte| line[..byte].chars().count());
    let name_x = column_of(header, "Assistant").expect("nombre");
    for child in ["pending", "refusal", "summary"] {
        let line = index
            .iter()
            .find(|l| l.contains(child))
            .unwrap_or_else(|| panic!("falta la fila hija {child}"));
        // Dos columnas más que el nombre de la cabecera: la hija va sangrada.
        assert_eq!(
            column_of(line, child),
            Some(name_x + 2),
            "sangría de {child}: {line}"
        );
    }
}

#[test]
fn footer_counts_components_and_missing() {
    let mut show = Showcase::new();
    show.set_theme_column(theme_index("Pawan"));
    let buffer = render(&mut show, 100, 30);
    let all = lines(&buffer, 0);
    assert!(
        all.iter()
            .any(|l| l.contains("26 componentes: 9 ● · 17 ◐ · faltan 31")),
        "recuento de Pawan: {all:?}"
    );
}

#[test]
fn footer_notes_partial_implementation_for_generic() {
    let mut show = Showcase::new();
    show.set_theme_column(theme_index("Pawan"));
    show.select(demo_of(&show, "Composer", "Cloth"), false);
    let buffer = render(&mut show, 100, 30);
    let all = lines(&buffer, 0);
    // La línea 2 del pie es la última de la pantalla.
    assert!(
        all[29].contains("parcialmente implementado"),
        "línea 2 del pie: {}",
        all[29]
    );
    assert!(
        all[28].contains("componentes:"),
        "línea 1 del pie: {}",
        all[28]
    );
}

#[test]
fn footer_is_one_line_for_an_own_component() {
    let mut show = Showcase::new();
    show.set_theme_column(theme_index("Crush"));
    show.select(demo_of(&show, "Composer", "Crush"), false);
    let buffer = render(&mut show, 100, 30);
    let all = lines(&buffer, 0);
    assert!(all[29].contains("componentes:"), "pie: {}", all[29]);
    assert!(all.iter().all(|l| !l.contains("parcialmente")));
}

#[test]
fn narrow_terminal_does_not_panic() {
    for (name, component, variant) in [
        ("Pawan", "Composer", "Cloth"),
        ("Crush", "Composer", "Crush"),
    ] {
        let mut show = Showcase::new();
        show.set_theme_column(theme_index(name));
        show.select(demo_of(&show, component, variant), false);
        for (w, h) in [(60u16, 20u16), (30, 8), (20, 5)] {
            render(&mut show, w, h);
        }
    }
}

#[test]
fn tab_layout_positions_match_what_is_drawn() {
    let mut show = Showcase::new();
    for width in [100u16, 60] {
        let buffer = render(&mut show, width, 30);
        let layout = tab_spans_layout(width);
        assert!(!layout.is_empty());
        for (column, x, shown) in layout {
            let (name, _) = matrix::COLUMNS[column];
            let drawn: String = (x..x + shown).map(|cx| buffer[(cx, 0)].symbol()).collect();
            assert_eq!(
                drawn,
                matrix::fit(&format!(" {name} "), usize::from(shown)),
                "pestaña {name} a {width} columnas"
            );
        }
    }
    // A 60 columnas la última pestaña no cabe entera: aparece recortada.
    let layout = tab_spans_layout(60);
    let (column, _, shown) = *layout.last().expect("pestañas");
    let (name, _) = matrix::COLUMNS[column];
    assert!(shown < tab_width(name), "{name} debería recortarse");
}

/// El aviso de «más resultados» sale por arriba y por abajo, y solo cuando hay más.
#[test]
fn index_notice_marks_both_ends_only_when_there_is_more() {
    // Al principio de una lista larga: nada por arriba, aviso en la última fila visible.
    assert_eq!(index_notice(0, 10, 50, 0), None);
    assert_eq!(index_notice(0, 10, 50, 9), Some("  ↓ más resultados"));
    assert_eq!(index_notice(0, 10, 50, 8), None);
    // En medio: aviso arriba y abajo.
    assert_eq!(index_notice(20, 10, 50, 0), Some("  ↑ más resultados"));
    assert_eq!(index_notice(20, 10, 50, 9), Some("  ↓ más resultados"));
    // Al final: aviso arriba y ninguno abajo.
    assert_eq!(index_notice(40, 10, 50, 0), Some("  ↑ más resultados"));
    assert_eq!(index_notice(40, 10, 50, 9), None);
    // Si cabe todo, ninguno.
    assert_eq!(index_notice(0, 10, 10, 9), None);
}

/// En un terminal bajo, la última fila visible del índice dice que hay más por debajo.
#[test]
fn a_short_terminal_shows_the_more_below_notice() {
    let mut show = Showcase::new();
    let buffer = render(&mut show, 100, 12);
    let text = lines(&buffer, 0).join(
        "
",
    );
    assert!(text.contains("↓ más resultados"), "{text}");
}
