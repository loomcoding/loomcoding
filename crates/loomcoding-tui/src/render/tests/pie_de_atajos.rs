//! Pie de cuatro atajos del chrome `Flat` y la lista completa que abre `?`.
//!
//! Cabe señalar que las pruebas miran la pantalla pintada y no constantes internas: así vigilan
//! lo que de verdad ve quien usa la TUI. El medidor de contexto del pie lo cubre
//! `footer_shows_the_meter_when_wide_enough` (cabecera_y_pie), que sigue pasando sin cambios.

use super::*;
use loomcoding_cloth::widgets::LayoutStyle;

/// Pinta la app a `w`×`h` y devuelve el búfer resultante.
fn pintado(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    term.backend().buffer().clone()
}

/// Fila final de la pantalla pintada a `w`×`h`, que es el pie.
fn footer_row(app: &mut App, w: u16, h: u16) -> String {
    let buf = pintado(app, w, h);
    let y = h - 1;
    (0..w).map(|x| buf[(x, y)].symbol()).collect()
}

/// Texto de las filas interiores del recuadro de ayuda: las que quedan entre su título y su
/// borde inferior. Así no se confunde con el pie ni con el fondo que tapa.
fn overlay_text(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let rows: Vec<String> = (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect()
        })
        .collect();
    let top = rows
        .iter()
        .position(|row| row.contains("Keyboard shortcuts"))
        .expect("el recuadro de ayuda tiene título");
    let bottom = rows
        .iter()
        .position(|row| row.contains("F1/Esc close"))
        .expect("el recuadro de ayuda tiene cierre");
    rows[top + 1..bottom].join("\n")
}

#[test]
fn footer_has_exactly_four_shortcuts() {
    let mut app = crush_ctx();
    let footer = footer_row(&mut app, 100, 30);
    // Sin medidor (no hay ventana de contexto), el pie es exactamente esta línea.
    assert_eq!(
        footer.trim_end(),
        "enter send  ctrl+o fold  ctrl+b panel  ? shortcuts",
        "pie: {footer:?}"
    );
    for old in [
        "ctrl+j", "ctrl+k", "ctrl+t", "f2", "esc", "newline", "queue", "settings",
    ] {
        assert!(
            !footer.contains(old),
            "el pie no debe mostrar {old:?}: {footer:?}"
        );
    }
}

#[test]
fn footer_names_the_card_in_cards_layout() {
    let mut app = crush_ctx();
    app.theme.look.layout = LayoutStyle::Cards;
    let footer = footer_row(&mut app, 100, 30);
    assert!(footer.contains("ctrl+b card"), "pie en Cards: {footer:?}");
    assert!(!footer.contains("ctrl+b panel"), "pie en Cards: {footer:?}");

    // En las demás disposiciones, el mismo atajo nombra el panel.
    for layout in [
        LayoutStyle::Workspace,
        LayoutStyle::ContextRail,
        LayoutStyle::Minimal,
    ] {
        let mut app = crush_ctx();
        app.theme.look.layout = layout;
        let footer = footer_row(&mut app, 100, 30);
        assert!(footer.contains("ctrl+b panel"), "{layout:?}: {footer:?}");
        assert!(!footer.contains("ctrl+b card"), "{layout:?}: {footer:?}");
    }
}

#[test]
fn help_overlay_still_lists_every_old_shortcut() {
    let mut app = crush_ctx();
    app.help_open = true;
    let buf = pintado(&mut app, 100, 30);
    let overlay = overlay_text(&buf);
    let lower = overlay.to_lowercase();
    // Los ocho atajos del pie antiguo (Enter, Ctrl+J, Ctrl+K, Ctrl+O, Ctrl+T, Ctrl+B, F2, Esc),
    // más `?`, que es el atajo nuevo que abre esta lista.
    for needle in [
        "enter", "ctrl+j", "ctrl+k", "ctrl+o", "ctrl+t", "ctrl+b", "f2", "esc", "?",
    ] {
        assert!(
            lower.contains(needle),
            "falta {needle:?} en la ayuda:\n{overlay}"
        );
    }
}

#[test]
fn help_overlay_does_not_clip_its_last_line_at_80x24() {
    let mut app = App::new();
    app.help_open = true;
    let buf = pintado(&mut app, 80, 24);
    let overlay = overlay_text(&buf);
    let last = overlay
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    assert!(
        last.contains("Ctrl+C/Q") && last.contains("Quit"),
        "la última línea de la ayuda se corta a 80×24. Última visible: {last:?}\n{overlay}"
    );
}
