//! Pruebas del estado del diff de la sesión en el menú izquierdo: selección, despliegue, recorte
//! y recogida del resultado.
//!
//! Cabe señalar que ninguna prueba lanza `git`: los resultados llegan por un canal de prueba y
//! `refresh_session_diff` solo se llama con `changes` vacía o con una petición ya en vuelo.

use super::*;
use crate::session_diff::DiffOutcome;
use loomcoding_cloth::state::FileChange;
use std::sync::mpsc;

/// App con `n` ficheros tocados, `src/f0.rs` … `src/f{n-1}.rs`.
fn con_ficheros(n: usize) -> App {
    let mut app = App::new();
    for i in 0..n {
        app.changes.upsert(FileChange {
            path: format!("src/f{i}.rs"),
            added: i + 1,
            removed: 0,
        });
    }
    app
}

#[test]
fn diff_select_se_acota_sin_dar_la_vuelta() {
    let mut app = con_ficheros(3);
    app.diff_select(-1);
    assert_eq!(app.session_diff.selected, 0);
    app.diff_select(5);
    assert_eq!(app.session_diff.selected, 2);
    app.diff_select(1);
    assert_eq!(app.session_diff.selected, 2);
    app.diff_select(-1);
    assert_eq!(app.session_diff.selected, 1);
}

#[test]
fn diff_select_sin_ficheros_se_queda_en_cero() {
    let mut app = App::new();
    app.diff_select(1);
    assert_eq!(app.session_diff.selected, 0);
    app.diff_toggle();
    assert_eq!(app.session_diff.expanded, None);
}

#[test]
fn diff_toggle_despliega_y_pliega_el_seleccionado() {
    let mut app = con_ficheros(3);
    app.diff_select(1);
    app.diff_toggle();
    assert_eq!(app.session_diff.expanded, Some(1));
    app.diff_toggle();
    assert_eq!(app.session_diff.expanded, None);
}

#[test]
fn solo_un_fichero_desplegado_a_la_vez() {
    let mut app = con_ficheros(3);
    app.diff_toggle();
    app.diff_select(2);
    app.diff_toggle();
    assert_eq!(app.session_diff.expanded, Some(2));
}

#[test]
fn si_changes_se_acorta_se_corrige_seleccion_y_despliegue() {
    let mut app = con_ficheros(3);
    app.diff_select(2);
    app.diff_toggle();
    app.changes.pop();
    app.changes.pop();
    // Cualquier operación de selección ajusta el estado antes de actuar.
    app.diff_select(0);
    assert_eq!(app.session_diff.selected, 0);
    assert_eq!(app.session_diff.expanded, None);
}

#[test]
fn refresh_con_changes_vacia_no_lanza_nada() {
    let mut app = App::new();
    app.refresh_session_diff();
    assert!(app.session_diff.pending.is_none());
    assert!(app.session_diff.outcome.is_none());
}

#[test]
fn refresh_no_relanza_si_ya_hay_una_peticion_en_vuelo() {
    let mut app = con_ficheros(2);
    let (tx, rx) = mpsc::channel();
    app.session_diff.pending = Some(rx);
    app.refresh_session_diff();
    tx.send(DiffOutcome::TimedOut).unwrap();
    app.poll_session_diff();
    // Si `refresh` hubiera sustituido el receptor, el resultado de prueba no llegaría.
    assert_eq!(app.session_diff.outcome, Some(DiffOutcome::TimedOut));
}

#[test]
fn poll_recoge_el_resultado_y_libera_pending() {
    let mut app = App::new();
    let (tx, rx) = mpsc::channel();
    app.session_diff.pending = Some(rx);
    tx.send(DiffOutcome::NoGit).unwrap();
    app.poll_session_diff();
    assert_eq!(app.session_diff.outcome, Some(DiffOutcome::NoGit));
    assert!(app.session_diff.pending.is_none());
}

#[test]
fn poll_sin_resultado_todavia_no_cambia_nada() {
    let mut app = App::new();
    let (_tx, rx) = mpsc::channel::<DiffOutcome>();
    app.session_diff.pending = Some(rx);
    app.poll_session_diff();
    assert!(app.session_diff.outcome.is_none());
    assert!(app.session_diff.pending.is_some());
}

#[test]
fn emisor_soltado_no_entra_en_panico_y_se_vuelve_a_pedir() {
    let mut app = App::new();
    let (tx, rx) = mpsc::channel::<DiffOutcome>();
    drop(tx);
    app.session_diff.pending = Some(rx);
    app.poll_session_diff();
    assert!(app.session_diff.pending.is_none());
    assert!(app.session_diff.outcome.is_none());
    assert_eq!(app.session_diff.fetched_for, 0);
}

#[test]
fn restore_messages_reinicia_el_diff_de_la_sesion() {
    let mut app = con_ficheros(2);
    app.diff_toggle();
    app.session_diff.outcome = Some(DiffOutcome::NoGit);
    app.restore_messages(Vec::new());
    assert!(app.changes.is_empty());
    assert_eq!(app.session_diff.expanded, None);
    assert!(app.session_diff.outcome.is_none());
}
