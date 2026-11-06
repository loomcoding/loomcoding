//! Estado de los efectos del logo: cuándo hay fotograma, cuándo cesa y cuál es su progreso.
//! Todo el reloj se inyecta: el planificador arranca en un `t0` fijo y las pruebas avanzan
//! instantes concretos.

use super::*;
use loomcoding_cloth::widgets::logo_fx::LogoFx;
use loomcoding_cloth::widgets::theme::ColorDepth;
use loomcoding_cloth::widgets::LayoutStyle;
use ratatui::layout::Rect;

/// App visible: layout `Workspace` con el menú izquierdo pintado y un planificador de semilla
/// fija anclado a `t0`, con el intervalo por defecto (20 s).
fn visible(t0: Instant) -> App {
    let mut app = App::with_context("/proj", "m");
    app.theme.look.layout = LayoutStyle::Workspace;
    app.pane_rects
        .push((Focus::Sidebar, Rect::new(0, 0, 23, 30)));
    app.logo_fx = LogoFx::new(Duration::from_secs(20), 7, t0);
    // Fijado aquí para que la prueba no dependa de la variable `NO_COLOR` del entorno.
    app.logo_fx_sin_color = false;
    app
}

fn segundos(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn sin_rafaga_no_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    assert!(!app.tick_logo_fx(t0 + segundos(19)));
    assert_eq!(app.logo_fx_frame, None);
    assert!(!app.logo_fx.needs_clock());
}

#[test]
fn con_rafaga_hay_fotograma_y_al_acabar_vuelve_a_none() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    let inicio = t0 + segundos(20);
    assert!(app.tick_logo_fx(inicio), "la ráfaga cambia el fotograma");
    let (_, progreso, _) = app.logo_fx_frame.expect("debía haber fotograma");
    assert_eq!(progreso, 0.0);
    assert!(app.logo_fx.needs_clock());

    let fin = app
        .logo_fx
        .deadline()
        .expect("en ráfaga, el plazo es su fin");
    assert!(app.tick_logo_fx(fin), "al acabar también cambia");
    assert_eq!(app.logo_fx_frame, None);
    assert!(!app.logo_fx.needs_clock(), "al acabar se detiene el reloj");
}

#[test]
fn el_progreso_va_de_0_a_1() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    let inicio = t0 + segundos(20);
    app.tick_logo_fx(inicio);
    let fin = app.logo_fx.deadline().unwrap();
    let mitad = inicio + (fin - inicio) / 2;

    app.tick_logo_fx(mitad);
    let (_, medio, _) = app.logo_fx_frame.unwrap();
    assert!((medio - 0.5).abs() < 0.01, "progreso a mitad: {medio}");

    app.tick_logo_fx(fin - Duration::from_millis(1));
    let (_, casi, _) = app.logo_fx_frame.unwrap();
    assert!(casi > 0.99 && casi <= 1.0, "progreso casi al final: {casi}");
}

#[test]
fn cada_rafaga_tiene_semilla_distinta() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.tick_logo_fx(t0 + segundos(20));
    let (_, _, primera) = app.logo_fx_frame.unwrap();
    let fin = app.logo_fx.deadline().unwrap();
    app.tick_logo_fx(fin);
    let proxima = app.logo_fx.deadline().unwrap();
    app.tick_logo_fx(proxima);
    let (_, _, segunda) = app.logo_fx_frame.unwrap();
    assert_ne!(primera, segunda);
}

#[test]
fn con_la_preferencia_apagada_nunca_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    // El planificador sigue encendido: lo que oculta el fotograma es la preferencia.
    app.logo_fx_on = false;
    for s in [20, 21, 25, 45, 60] {
        app.tick_logo_fx(t0 + segundos(s));
        assert_eq!(app.logo_fx_frame, None, "a los {s} s");
    }
}

#[test]
fn con_otro_layout_nunca_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.theme.look.layout = LayoutStyle::Cards;
    app.tick_logo_fx(t0 + segundos(20));
    assert_eq!(app.logo_fx_frame, None);
}

#[test]
fn sin_menu_pintado_nunca_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.pane_rects.clear();
    app.tick_logo_fx(t0 + segundos(20));
    assert_eq!(app.logo_fx_frame, None);
}

#[test]
fn con_overlay_abierto_nunca_hay_fotograma() {
    let abridores: [fn(&mut App); 9] = [
        |a| a.help_open = true,
        |a| a.settings_open = true,
        |a| a.temas_open = true,
        |a| a.agents_open = true,
        |a| a.a2a_open = true,
        |a| a.tools_open = true,
        |a| a.queue_open = true,
        |a| a.session_picker_open = true,
        |a| a.session_loading = true,
    ];
    for (indice, abrir) in abridores.into_iter().enumerate() {
        let t0 = Instant::now();
        let mut app = visible(t0);
        abrir(&mut app);
        assert!(app.overlay_open(), "overlay {indice}");
        app.tick_logo_fx(t0 + segundos(20));
        assert_eq!(app.logo_fx_frame, None, "overlay {indice}");
    }
}

#[test]
fn sin_color_del_tema_nunca_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.theme.depth = ColorDepth::NoColor;
    app.tick_logo_fx(t0 + segundos(20));
    assert_eq!(app.logo_fx_frame, None);
}

#[test]
fn no_color_en_el_entorno_nunca_hay_fotograma() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.logo_fx_sin_color = true;
    app.tick_logo_fx(t0 + segundos(20));
    assert_eq!(app.logo_fx_frame, None);
}

#[test]
fn las_preferencias_guardadas_se_cargan_al_fijar_el_fichero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "logo_fx = false\nlogo_fx_interval = 9999\n").unwrap();
    let mut app = App::new();
    app.set_prefs_path(path);
    assert!(!app.logo_fx_on);
    assert_eq!(app.logo_fx.interval(), segundos(600));
}

#[test]
fn sin_preferencias_el_logo_arranca_encendido_cada_20_segundos() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new();
    app.set_prefs_path(dir.path().join("ui.toml"));
    assert!(app.logo_fx_on);
    assert_eq!(app.logo_fx.interval(), segundos(20));
}

#[test]
fn al_cerrar_el_overlay_el_fotograma_aparece_en_la_siguiente_pasada() {
    let t0 = Instant::now();
    let mut app = visible(t0);
    app.help_open = true;
    app.tick_logo_fx(t0 + segundos(20));
    assert_eq!(app.logo_fx_frame, None);
    app.help_open = false;
    assert!(app.tick_logo_fx(t0 + segundos(20)));
    assert!(app.logo_fx_frame.is_some());
}
