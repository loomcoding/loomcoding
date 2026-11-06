//! Teclas del menú izquierdo con el foco en él: `↑`/`↓` eligen fichero, `Enter` despliega o
//! pliega su diff, y `Shift+Tab` desde el compositor lleva al menú cuando muestra ficheros.

use super::*;
use loomcoding_cloth::state::FileChange;
use ratatui::layout::Rect;

/// App con `n` ficheros tocados y el menú izquierdo registrado como ventana pintada.
fn app_con_ficheros(n: usize) -> App {
    let mut app = App::new();
    for i in 0..n {
        app.changes.upsert(FileChange {
            path: format!("src/f{i}.rs"),
            added: 1,
            removed: 0,
        });
    }
    app.pane_rects
        .push((Focus::Sidebar, Rect::new(0, 0, 23, 20)));
    app
}

#[test]
fn con_foco_en_el_menu_las_flechas_eligen_fichero() {
    let mut app = app_con_ficheros(3);
    app.focus = Focus::Sidebar;
    assert_eq!(
        handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.session_diff.selected, 1);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.session_diff.selected, 2, "sin dar la vuelta");
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.session_diff.selected, 1);
    assert!(app.composer.is_empty());
}

#[test]
fn con_foco_en_el_menu_enter_despliega_y_pliega_sin_enviar() {
    let mut app = app_con_ficheros(2);
    app.focus = Focus::Sidebar;
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.session_diff.expanded, Some(0));
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        None
    );
    assert_eq!(app.session_diff.expanded, None);
    assert!(app.pending_prompts.is_empty());
}

#[test]
fn con_foco_en_el_compositor_las_flechas_no_tocan_el_diff_ni_el_texto() {
    let mut app = app_con_ficheros(3);
    app.composer.set_text("hola");
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.session_diff.selected, 0);
    assert_eq!(app.composer.text(), "hola");
}

#[test]
fn con_foco_en_el_compositor_enter_no_despliega_el_diff() {
    let mut app = app_con_ficheros(3);
    app.composer.set_text("");
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.session_diff.expanded, None);
}

#[test]
fn shift_tab_desde_el_compositor_lleva_al_menu_si_hay_ficheros() {
    let mut app = app_con_ficheros(2);
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(app.focus, Focus::Sidebar);
}

#[test]
fn shift_tab_sin_ficheros_conserva_el_ciclo_de_siempre() {
    let mut app = app_con_ficheros(0);
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(app.focus, Focus::Transcript);
}

#[test]
fn shift_tab_sin_menu_pintado_conserva_el_ciclo_de_siempre() {
    let mut app = app_con_ficheros(2);
    app.pane_rects.clear();
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(app.focus, Focus::Transcript);
}

#[test]
fn tab_desde_el_menu_vuelve_al_compositor() {
    let mut app = app_con_ficheros(2);
    app.focus = Focus::Sidebar;
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Composer);
}
