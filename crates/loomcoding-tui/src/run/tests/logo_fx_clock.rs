//! Reloj y plazo del logo de cabecera. El reloj corre solo mientras hay una ráfaga en pantalla;
//! en reposo, el bucle solo despierta en el plazo del planificador. Ninguna prueba mira el reloj
//! del sistema: los instantes se inyectan.

use super::*;
use loomcoding_cloth::widgets::logo_fx::LogoFx;
use loomcoding_cloth::widgets::LayoutStyle;
use ratatui::layout::Rect;
use std::time::Duration;

/// App visible para el logo: layout `Workspace`, menú pintado y planificador de semilla fija
/// anclado a `t0` con el intervalo por defecto.
fn visible(t0: Instant) -> App {
    let mut app = App::with_context("/proj", "m");
    app.theme.look.layout = LayoutStyle::Workspace;
    app.pane_rects
        .push((Focus::Sidebar, Rect::new(0, 0, 23, 30)));
    app.logo_fx = LogoFx::new(Duration::from_secs(20), 7, t0);
    app.logo_fx_sin_color = false;
    app
}

#[test]
fn sin_rafaga_el_reloj_no_corre_por_el_logo() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.tick_logo_fx(t0 + Duration::from_secs(19));
    assert!(!wants_clock(&app, false, t0 + Duration::from_secs(19)));
}

#[test]
fn con_rafaga_el_reloj_corre_y_se_para_al_acabar() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    let inicio = t0 + Duration::from_secs(20);
    app.tick_logo_fx(inicio);
    assert!(wants_clock(&app, false, inicio));

    let fin = app.logo_fx_deadline().expect("en ráfaga hay plazo");
    app.tick_logo_fx(fin);
    assert!(!wants_clock(&app, false, fin));
}

#[test]
fn en_reposo_el_plazo_es_la_proxima_rafaga() {
    let t0 = Instant::now();
    let app = visible(t0);
    assert_eq!(app.logo_fx_deadline(), Some(t0 + Duration::from_secs(20)));
}

#[test]
fn earliest_elige_el_plazo_mas_proximo() {
    let t0 = Instant::now();
    let a = t0 + Duration::from_secs(1);
    let b = t0 + Duration::from_secs(2);
    assert_eq!(earliest(Some(b), Some(a)), Some(a));
    assert_eq!(earliest(Some(a), None), Some(a));
    assert_eq!(earliest(None, Some(b)), Some(b));
    assert_eq!(earliest(None, None), None);
}
