//! Etiqueta de modo/flujo en las dos cabeceras: se pinta junto al estado y el modelo, se acorta
//! antes de desaparecer y nunca desplaza ni rompe el modelo ni el estado.

use super::*;
use loomcoding_core::config::OddConfig;
use loomcoding_core::flow::FlowSummary;
use ratatui::layout::Rect;

/// Aplicación con ODD encendido, modelo `m` y estado `ready`.
fn app_con_odd() -> App {
    let mut app = App::with_context("/proj", "m").with_odd_config(OddConfig { enabled: true });
    app.status = "ready".into();
    app
}

fn fase_de_odd(app: &mut App, fase: &str) {
    app.apply(loomcoding_core::event::LiveEvent::FlowChanged {
        summary: FlowSummary {
            kind: "odd".into(),
            id: 0,
            phase: fase.into(),
            round: 0,
            max_rounds: 0,
            terminal: false,
            label: format!("ODD · {fase}"),
        },
        actor: String::new(),
        detail: String::new(),
    });
}

/// Primera fila de la cabecera pintada en un terminal de `ancho` columnas.
fn fila(app: &App, ancho: u16, plana: bool) -> String {
    let mut term = Terminal::new(TestBackend::new(ancho, 3)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, ancho, 1);
        if plana {
            draw_flat_header(f, area, app);
        } else {
            draw_workspace_header(f, Rect::new(0, 0, ancho, 3), app);
        }
    })
    .unwrap();
    let buffer = term.backend().buffer();
    buffer.content[..usize::from(ancho)]
        .iter()
        .map(|celda| celda.symbol())
        .collect()
}

#[test]
fn cabecera_plana_muestra_la_fase_de_odd_junto_al_estado() {
    let mut app = app_con_odd();
    fase_de_odd(&mut app, "implementing");
    let texto = fila(&app, 120, true);
    assert!(texto.contains("ready · ODD · implementing"), "{texto}");
    assert!(texto.contains(" m "), "{texto}");
}

#[test]
fn cabecera_de_caja_muestra_la_fase_de_odd_junto_al_modelo() {
    let mut app = app_con_odd();
    fase_de_odd(&mut app, "exploring");
    let texto = fila(&app, 120, false);
    assert!(texto.contains("m  · ODD · exploring"), "{texto}");
}

#[test]
fn sin_etiqueta_la_cabecera_plana_no_menciona_odd() {
    let app = App::with_context("/proj", "m");
    let texto = fila(&app, 120, true);
    assert!(!texto.contains("ODD"), "{texto}");
    assert!(texto.contains(" · m · ready"), "{texto}");
}

#[test]
fn sin_etiqueta_la_cabecera_de_caja_no_menciona_odd() {
    let app = App::with_context("/proj", "m");
    let texto = fila(&app, 120, false);
    assert!(!texto.contains("ODD"), "{texto}");
}

#[test]
fn en_cabecera_plana_la_fase_se_quita_antes_que_la_etiqueta_entera() {
    // " loom · m · ready" ocupa 17 columnas; " · ODD" son 6 más y " · ODD · implementing" son 21.
    let mut app = app_con_odd();
    fase_de_odd(&mut app, "implementing");
    let base = fila(&App::with_context("/proj", "m"), 17, true);
    assert!(base.contains("ready"), "{base}");

    let solo_odd = fila(&app, 23, true);
    assert!(solo_odd.contains("ready · ODD"), "{solo_odd}");
    assert!(!solo_odd.contains("implementing"), "{solo_odd}");

    let completa = fila(&app, 40, true);
    assert!(completa.contains("ODD · implementing"), "{completa}");

    let sin_etiqueta = fila(&app, 22, true);
    assert!(sin_etiqueta.contains("ready"), "{sin_etiqueta}");
    assert!(!sin_etiqueta.contains("ODD"), "{sin_etiqueta}");
}

/// Recorre anchos y comprueba las reglas del recorte en ambas cabeceras: la fila nunca pasa del
/// ancho, la etiqueta aparece entera o no aparece, la forma corta aparece antes que desaparecer,
/// y el modelo y el estado se ven en todo ancho en que se verían sin etiqueta.
#[test]
fn la_etiqueta_se_acorta_antes_de_desaparecer_en_ambas_cabeceras() {
    for plana in [true, false] {
        let mut con = app_con_odd();
        fase_de_odd(&mut con, "implementing");
        let sin = App::with_context("/proj", "m");
        let mut hubo_corta = false;
        // El modelo: en la plana va entre separadores; en la de caja, tras el estado.
        let modelo = |t: &str| {
            if plana {
                t.contains(" · m")
            } else {
                t.contains("  m")
            }
        };
        for ancho in 30..=120u16 {
            let texto = fila(&con, ancho, plana);
            let base = fila(&sin, ancho, plana);
            if texto.contains("implementing") {
                assert!(texto.contains("ODD · implementing"), "{ancho}: {texto:?}");
            } else if texto.contains("ODD") {
                hubo_corta = true;
            }
            if modelo(&base) {
                assert!(
                    modelo(&texto),
                    "el modelo desaparece en {ancho} (plana={plana}): {texto:?}"
                );
            }
            if base.contains("ready") {
                assert!(
                    texto.contains("ready"),
                    "el estado desaparece en {ancho} (plana={plana}): {texto:?}"
                );
            }
        }
        assert!(hubo_corta, "nunca se ve la forma corta (plana={plana})");
        // Ni la forma corta cabe: la etiqueta desaparece entera, y la cabecera sigue pintándose.
        let estrecho = fila(&con, 10, plana);
        assert!(!estrecho.contains("ODD"), "{estrecho:?}");
    }
}
