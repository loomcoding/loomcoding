//! Dibujo del bloque «Ficheros» del menú izquierdo: una fila por fichero, el diff desplegado,
//! los recortes y los avisos de `git`. Los datos son fijos; ninguna prueba lanza `git`.

use super::*;
use crate::render::sidebar_diff::{draw_files_block, files_block_height, MAX_DIFF_LINES};
use crate::session_diff::{DiffLine, DiffOutcome, FileDiff, FileKind, Hunk};
use loomcoding_cloth::state::FileChange;
use ratatui::layout::Rect;

fn fichero(path: &str, added: usize, removed: usize) -> FileChange {
    FileChange {
        path: path.into(),
        added,
        removed,
    }
}

/// App con los ficheros dados y el resultado de diff indicado.
fn app_con(changes: Vec<FileChange>, outcome: Option<DiffOutcome>) -> App {
    let mut app = App::with_context("/proj", "m");
    for change in changes {
        app.changes.upsert(change);
    }
    app.session_diff.outcome = outcome;
    app
}

/// Diff de `main.rs` con un hunk de tres líneas: una quitada, una añadida y una de contexto.
fn diff_main() -> DiffOutcome {
    DiffOutcome::Ok(vec![FileDiff {
        path: "src/main.rs".into(),
        kind: FileKind::Modified,
        added: 1,
        removed: 1,
        hunks: vec![Hunk {
            header: "@@ -1,2 +1,2 @@".into(),
            lines: vec![
                DiffLine::Removed("old".into()),
                DiffLine::Added("new".into()),
                DiffLine::Context("same".into()),
            ],
        }],
    }])
}

/// Pinta solo el bloque en un terminal de `w`×`h` y devuelve sus filas, sin el borde.
fn filas(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw_files_block(f, Rect::new(0, 0, w, h), app))
        .unwrap();
    let buf = term.backend().buffer();
    (0..h)
        .map(|y| {
            (1..w)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn hay(filas: &[String], aguja: &str) -> bool {
    filas.iter().any(|fila| fila.contains(aguja))
}

#[test]
fn una_fila_por_fichero_con_nombre_y_cuentas() {
    let app = app_con(
        vec![
            fichero("crates/a/src/main.rs", 2, 1),
            fichero("crates/a/src/lib.rs", 0, 3),
        ],
        None,
    );
    let filas = filas(&app, 23, 10);
    assert!(hay(&filas, "Ficheros"), "{filas:?}");
    assert!(hay(&filas, "▸ main.rs +2 −1"), "{filas:?}");
    assert!(hay(&filas, "▸ lib.rs +0 −3"), "{filas:?}");
    assert!(
        !hay(&filas, "crates/a"),
        "solo el nombre, no la ruta: {filas:?}"
    );
}

#[test]
fn desplegado_muestra_la_cabecera_y_las_lineas_del_diff() {
    let mut app = app_con(vec![fichero("src/main.rs", 1, 1)], Some(diff_main()));
    app.session_diff.expanded = Some(0);
    let filas = filas(&app, 30, 10);
    assert!(hay(&filas, "▾ main.rs +1 −1"), "{filas:?}");
    assert!(hay(&filas, "@@ -1,2 +1,2 @@"), "{filas:?}");
    assert!(hay(&filas, "-old"), "{filas:?}");
    assert!(hay(&filas, "+new"), "{filas:?}");
    assert!(hay(&filas, " same"), "{filas:?}");
}

#[test]
fn sin_desplegar_no_aparece_el_diff() {
    let app = app_con(vec![fichero("src/main.rs", 1, 1)], Some(diff_main()));
    let filas = filas(&app, 30, 10);
    assert!(!hay(&filas, "@@"), "{filas:?}");
}

#[test]
fn las_lineas_largas_se_recortan_con_puntos_suspensivos() {
    let largo = "x".repeat(80);
    let outcome = DiffOutcome::Ok(vec![FileDiff {
        path: "src/main.rs".into(),
        kind: FileKind::Modified,
        added: 1,
        removed: 0,
        hunks: vec![Hunk {
            header: "@@ -0,0 +1 @@".into(),
            lines: vec![DiffLine::Added(largo)],
        }],
    }]);
    let mut app = app_con(vec![fichero("src/main.rs", 1, 0)], Some(outcome));
    app.session_diff.expanded = Some(0);
    let filas = filas(&app, 20, 6);
    assert!(hay(&filas, "+xxxx"), "{filas:?}");
    assert!(hay(&filas, "…"), "{filas:?}");
    assert!(
        filas.iter().all(|fila| fila.chars().count() <= 19),
        "{filas:?}"
    );
}

#[test]
fn el_diff_se_limita_a_doce_lineas_y_lo_indica() {
    // Cabecera + 30 de contexto = 31 líneas: se ven 12 y quedan 19.
    let lineas = (0..30)
        .map(|i| DiffLine::Context(format!("c{i}")))
        .collect();
    let outcome = DiffOutcome::Ok(vec![FileDiff {
        path: "src/main.rs".into(),
        kind: FileKind::Modified,
        added: 0,
        removed: 0,
        hunks: vec![Hunk {
            header: "@@ -1,30 +1,30 @@".into(),
            lines: lineas,
        }],
    }]);
    let mut app = app_con(vec![fichero("src/main.rs", 0, 0)], Some(outcome));
    app.session_diff.expanded = Some(0);
    assert_eq!(MAX_DIFF_LINES, 12);
    let filas = filas(&app, 30, 24);
    // Título, fila del fichero, doce líneas de diff y el aviso.
    assert!(filas[13].contains("c10"), "{filas:?}");
    assert!(!filas[13].contains("…"), "{filas:?}");
    assert!(filas[14].contains("19 líneas más"), "{filas:?}");
    assert!(!hay(&filas, "c11"), "{filas:?}");
}

#[test]
fn sin_git_lo_dice_y_las_filas_siguen_con_sus_cuentas() {
    let mut app = app_con(vec![fichero("src/main.rs", 4, 2)], Some(DiffOutcome::NoGit));
    app.session_diff.expanded = Some(0);
    let filas = filas(&app, 30, 8);
    assert!(hay(&filas, "▾ main.rs +4 −2"), "{filas:?}");
    assert!(hay(&filas, "sin git"), "{filas:?}");
}

#[test]
fn git_que_tarda_demasiado_lo_dice() {
    let mut app = app_con(
        vec![fichero("src/main.rs", 1, 0)],
        Some(DiffOutcome::TimedOut),
    );
    app.session_diff.expanded = Some(0);
    let filas = filas(&app, 30, 8);
    assert!(hay(&filas, "git tardó demasiado"), "{filas:?}");
}

#[test]
fn mientras_carga_muestra_puntos_suspensivos() {
    let mut app = app_con(vec![fichero("src/main.rs", 1, 0)], None);
    app.session_diff.expanded = Some(0);
    let filas = filas(&app, 30, 8);
    assert!(hay(&filas, "…"), "{filas:?}");
}

#[test]
fn el_fichero_seleccionado_se_resalta_con_el_estilo_de_seleccion() {
    let mut app = app_con(
        vec![fichero("src/a.rs", 1, 0), fichero("src/b.rs", 1, 0)],
        None,
    );
    app.session_diff.selected = 1;
    let mut term = Terminal::new(TestBackend::new(30, 6)).unwrap();
    term.draw(|f| draw_files_block(f, Rect::new(0, 0, 30, 6), &app))
        .unwrap();
    let buf = term.backend().buffer();
    // Columna 0: borde. Fila 0: título. Fila 1: a.rs. Fila 2: b.rs, la seleccionada.
    let fondo_sel = app
        .theme
        .styles
        .selection
        .bg
        .expect("el tema define el fondo de selección");
    assert_eq!(buf[(1, 2)].bg, fondo_sel);
    assert_ne!(buf[(1, 1)].bg, fondo_sel);
}

#[test]
fn sin_ficheros_no_hay_bloque_ni_titulo() {
    let app = app_con(Vec::new(), None);
    assert_eq!(files_block_height(&app), 0);
    let out = buffer_of(&mut App::with_context("/proj", "m"), 120, 40);
    assert!(!out.contains("Ficheros"));
}

#[test]
fn la_altura_del_bloque_cuenta_titulo_filas_y_desplegado() {
    let mut app = app_con(vec![fichero("src/main.rs", 1, 1)], Some(diff_main()));
    assert_eq!(files_block_height(&app), 2);
    app.session_diff.expanded = Some(0);
    assert_eq!(files_block_height(&app), 2 + 4);
}

#[test]
fn pantalla_completa_con_ficheros_muestra_el_bloque() {
    let mut app = App::with_context("/proj", "mistral-nemo");
    app.changes.upsert(fichero("src/main.rs", 3, 1));
    let out = buffer_of(&mut app, 120, 40);
    assert!(out.contains("Ficheros"), "{out}");
    assert!(out.contains("main.rs +3 −1"), "{out}");
    assert!(!out.contains("ficheros modificados"), "{out}");
}

#[test]
fn zero_y_una_celda_no_entran_en_panico() {
    let app = app_con(vec![fichero("src/main.rs", 1, 1)], Some(diff_main()));
    let mut app_desplegada = app_con(vec![fichero("src/main.rs", 1, 1)], Some(diff_main()));
    app_desplegada.session_diff.expanded = Some(0);
    for app in [&app, &app_desplegada] {
        let _ = filas(app, 1, 1);
        let _ = filas(app, 0, 0);
        let _ = filas(app, 30, 0);
        let _ = filas(app, 1, 40);
    }
}
