//! Tests: hit.

use super::support::*;
use super::*;

#[test]
fn hit_crush_first_row_start_and_end() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("abc");
    let area = Rect::new(2, 1, 20, 3);
    // Prefijo "> " en las celdas x = 2 y 3: cae al inicio de la fila.
    assert_eq!(hit_index(&theme, &c, area, 2, 1), Some(0));
    assert_eq!(hit_index(&theme, &c, area, 3, 1), Some(0));
    assert_eq!(hit_index(&theme, &c, area, 4, 1), Some(0));
    // Un carácter estrecho: su celda deja el cursor delante de él.
    assert_eq!(hit_index(&theme, &c, area, 5, 1), Some(1));
    assert_eq!(hit_index(&theme, &c, area, 6, 1), Some(2));
    // Más allá del final de la fila: tras la última «c».
    assert_eq!(hit_index(&theme, &c, area, 7, 1), Some(3));
    assert_eq!(hit_index(&theme, &c, area, 19, 1), Some(3));
}

#[test]
fn hit_crush_wrapped_rows() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    // Ancho 5: el prefijo deja 3 celdas en la primera fila → "ab" + "日c".
    c.set_text("ab日c");
    let area = Rect::new(0, 0, 5, 3);
    assert_eq!(hit_index(&theme, &c, area, 3, 0), Some(1)); // «b»
                                                            // Fin de la fila 0 = inicio de la fila 1: la posición es la misma.
    assert_eq!(hit_index(&theme, &c, area, 4, 0), Some(2));
    // Sangría "  " de la fila 1, sobre el «日».
    assert_eq!(hit_index(&theme, &c, area, 2, 1), Some(2));
    assert_eq!(hit_index(&theme, &c, area, 3, 1), Some(3)); // mitad derecha
    assert_eq!(hit_index(&theme, &c, area, 4, 1), Some(3)); // «c»
}

#[test]
fn hit_crush_respects_view_top() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("1\n2\n3\n4\n5\n6\n7\n8"); // el cursor queda en la línea "8"
    let area = Rect::new(0, 0, 20, 3);
    let _ = cursor_after_render(&c, area, &theme);
    assert_eq!(c.view_top(), 5, "el render deja la vista en la fila 5");
    assert_eq!(hit_index(&theme, &c, area, 4, 0), Some(10)); // «6», inicio
    assert_eq!(hit_index(&theme, &c, area, 5, 0), Some(11)); // tras la «6»
    assert_eq!(hit_index(&theme, &c, area, 4, 1), Some(12)); // «7»
    assert_eq!(hit_index(&theme, &c, area, 4, 2), Some(14)); // «8»
    assert_eq!(hit_index(&theme, &c, area, 19, 2), Some(15)); // fin del texto
}

#[test]
fn hit_crush_wide_chars() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("日本語");
    let area = Rect::new(0, 0, 10, 3);
    assert_eq!(hit_index(&theme, &c, area, 2, 0), Some(0)); // mitad izquierda de «日»
    assert_eq!(hit_index(&theme, &c, area, 3, 0), Some(1)); // mitad derecha de «日»
    assert_eq!(hit_index(&theme, &c, area, 4, 0), Some(1)); // mitad izquierda de «本»
    assert_eq!(hit_index(&theme, &c, area, 5, 0), Some(2));
    assert_eq!(hit_index(&theme, &c, area, 7, 0), Some(3));
    assert_eq!(hit_index(&theme, &c, area, 9, 0), Some(3)); // más allá del final

    // Un grafema combinado ocupa una sola celda.
    c.set_text("e\u{301}x");
    assert_eq!(hit_index(&theme, &c, area, 2, 0), Some(0));
    assert_eq!(hit_index(&theme, &c, area, 3, 0), Some(1));
    assert_eq!(hit_index(&theme, &c, area, 4, 0), Some(2));
}

#[test]
fn hit_crush_prefix_cells_map_to_row_start() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("uno\ndos");
    let area = Rect::new(0, 0, 20, 3);
    assert_eq!(hit_index(&theme, &c, area, 0, 0), Some(0)); // "> "
    assert_eq!(hit_index(&theme, &c, area, 1, 0), Some(0));
    assert_eq!(hit_index(&theme, &c, area, 0, 1), Some(4)); // "::: "
    assert_eq!(hit_index(&theme, &c, area, 3, 1), Some(4));
}

#[test]
fn hit_crush_out_of_range_is_clamped() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    c.set_text("1\n2");
    let area = Rect::new(0, 1, 20, 3);
    assert_eq!(hit_index(&theme, &c, area, 2, 0), Some(0)); // por encima del área
    assert_eq!(hit_index(&theme, &c, area, 4, 4), Some(2)); // por debajo: última fila
    assert_eq!(hit_index(&theme, &c, area, 19, 1), Some(1)); // fin de la primera
    assert_eq!(
        hit_index(&theme, &c, area, u16::MAX, u16::MAX),
        Some(3),
        "esquina inferior derecha: fin del texto"
    );
}

#[test]
fn hit_classic_single_and_multiline() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("abc\ndef");
    // Interior (1,1)-(18,3): el prefijo ocupa las celdas x = 1 y 2.
    let area = Rect::new(0, 0, 20, 5);
    assert_eq!(hit_index(&theme, &c, area, 3, 1), Some(0));
    assert_eq!(hit_index(&theme, &c, area, 5, 1), Some(2)); // antes de la «c»
    assert_eq!(hit_index(&theme, &c, area, 10, 1), Some(3)); // fin de la línea 0
    assert_eq!(hit_index(&theme, &c, area, 3, 2), Some(4)); // inicio de la línea 1
    assert_eq!(hit_index(&theme, &c, area, 5, 2), Some(6)); // antes de la «f»
}

#[test]
fn classic_wrapped_hit_follows_the_cursor_painted_last() {
    let theme = Theme::test();
    let mut c = Composer::new();
    // Línea larga: el clic sigue la fila visual que se pintó.
    c.set_text("abcdefghijklmnopqrstuvwxyz0123456789");
    let area = Rect::new(0, 0, 20, 5);
    let _ = cursor_after_render(&c, area, &theme);
    // Tercera fila visual (inicia en el grafema 32).
    let first = hit_index(&theme, &c, area, 3, 3).unwrap();
    assert_eq!(first, 32);
    // Movemos el cursor sin repintar: la ventana pintada no debe cambiar.
    c.set_cursor(0);
    for col in 0..5 {
        assert_eq!(
            hit_index(&theme, &c, area, 3 + col, 3),
            Some(first + usize::from(col)),
            "la celda {col} debe corresponder a lo último pintado"
        );
    }
}

#[test]
fn crush_hit_follows_the_layout_painted_last() {
    let theme = Theme::preset(crate::widgets::ThemePreset::Crush);
    let mut c = Composer::new();
    // Con el cursor al final de una fila llena, el layout añade una fila propia.
    c.set_text("abcd");
    let area = Rect::new(0, 0, 6, 3);
    let _ = cursor_after_render(&c, area, &theme);
    c.set_cursor(0);
    // La fila 1 es la del cursor pintado: su sangría de 2 celdas cae al final.
    assert_eq!(hit_index(&theme, &c, area, 2, 1), Some(4));
}

#[test]
fn hit_classic_border_cells_are_clamped() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("abc\ndef");
    let area = Rect::new(0, 0, 20, 5);
    assert_eq!(hit_index(&theme, &c, area, 0, 0), Some(0)); // esquina superior izquierda
    assert_eq!(hit_index(&theme, &c, area, 0, 4), Some(4)); // borde inferior: última línea
    assert_eq!(hit_index(&theme, &c, area, 19, 0), Some(3)); // borde derecho, línea 0
    assert_eq!(hit_index(&theme, &c, area, 19, 2), Some(7)); // borde derecho, línea 1
    assert_eq!(
        hit_index(&theme, &c, area, u16::MAX, u16::MAX),
        Some(7),
        "esquina inferior derecha"
    );
}

#[test]
fn hit_classic_wrapped_rows() {
    let theme = Theme::test();
    let mut c = Composer::new();
    c.set_text("abcdefghijklmnop");
    let area = Rect::new(0, 0, 10, 3); // seis caracteres por fila tras el prefijo
    assert_eq!(hit_index(&theme, &c, area, 3, 1), Some(0)); // «a»
    assert_eq!(hit_index(&theme, &c, area, 5, 1), Some(2)); // «c»
    assert_eq!(hit_index(&theme, &c, area, 7, 1), Some(4)); // «e»
    assert_eq!(hit_index(&theme, &c, area, 8, 1), Some(5)); // fin de fila
}

#[test]
fn hit_empty_composer_is_zero() {
    let c = Composer::new();
    let area = Rect::new(0, 0, 20, 3);
    for theme in [
        Theme::test(),
        Theme::preset(crate::widgets::ThemePreset::Crush),
    ] {
        assert_eq!(hit_index(&theme, &c, area, 5, 1), Some(0));
        assert_eq!(hit_index(&theme, &c, area, 0, 0), Some(0));
    }
}

#[test]
fn hit_with_empty_area_is_none() {
    let mut c = Composer::new();
    c.set_text("abc");
    let crush = Theme::preset(crate::widgets::ThemePreset::Crush);
    let classic = Theme::test();
    assert_eq!(hit_index(&crush, &c, Rect::new(0, 0, 0, 0), 0, 0), None);
    assert_eq!(hit_index(&crush, &c, Rect::new(0, 0, 20, 0), 0, 0), None);
    assert_eq!(hit_index(&classic, &c, Rect::new(0, 0, 0, 0), 0, 0), None);
    // Classic sin interior: el marco ocupa toda el área.
    assert_eq!(hit_index(&classic, &c, Rect::new(0, 0, 2, 5), 0, 0), None);
    assert_eq!(hit_index(&classic, &c, Rect::new(0, 0, 20, 2), 0, 0), None);
}

#[test]
fn hit_and_cursor_agree() {
    // Texto con ajuste (Crush), recorte horizontal y desplazamiento vertical
    // (Classic), con caracteres anchos, emoji y acentos combinados.
    let text = "ab cdefghijklmnopqrstuvwxyz\nlínea 2 日本語 😀 👨‍👩‍👧 fin\n\nx";
    let cases = [
        (Theme::test(), Rect::new(0, 0, 16, 4)),
        (
            Theme::preset(crate::widgets::ThemePreset::Crush),
            Rect::new(1, 1, 12, 4),
        ),
    ];
    for (theme, area) in cases {
        let mut c = Composer::new();
        c.set_text(text);
        let total = text.graphemes(true).count();
        // Ida y vuelta: el desplazamiento se prueba subiendo y bajando.
        for pos in (0..=total).chain((0..=total).rev()) {
            c.set_cursor(pos);
            let (x, y) = cursor_after_render(&c, area, &theme)
                .unwrap_or_else(|| panic!("sin cursor pintado en {pos}"));
            let inside = x >= area.x && x < area.right() && y >= area.y && y < area.bottom();
            assert!(
                inside,
                "{:?}: cursor {pos} fuera del área en ({x}, {y})",
                theme.look.composer
            );
            assert_eq!(
                hit_index(&theme, &c, composer_areas(area).0, x, y),
                Some(pos),
                "{:?}: el cursor {pos} se pintó en ({x}, {y})",
                theme.look.composer
            );
        }
    }
}
