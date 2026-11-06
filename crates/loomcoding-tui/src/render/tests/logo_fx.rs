//! Dibujo del efecto del logo sobre el menú izquierdo: con un fotograma cambia la marca y nada
//! fuera de su rectángulo; sin fotograma el dibujo es el de siempre.

use super::*;
use loomcoding_cloth::widgets::logo_fx::LogoEffect;
use loomcoding_cloth::widgets::sidebar::logo_rect;
use loomcoding_cloth::widgets::LayoutStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// App en layout `Workspace` a 120 columnas, con el color permitido por el entorno del test.
fn workspace() -> App {
    let mut app = App::with_context("/proj", "m");
    app.theme.look.layout = LayoutStyle::Workspace;
    app.logo_fx_sin_color = false;
    app
}

/// Pinta la app completa en un terminal de 120×30 y devuelve el buffer.
fn pintar(app: &mut App) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    term.backend().buffer().clone()
}

/// Rectángulo de la marca tal como lo dibuja el último fotograma.
fn rect_de_la_marca(app: &App) -> Rect {
    let pane = app
        .pane_rects
        .iter()
        .find(|(foco, _)| *foco == Focus::Sidebar)
        .expect("el menú izquierdo está pintado")
        .1;
    logo_rect(pane, &app.sidebar()).expect("la marca cabe en 120×30")
}

#[test]
fn sin_fotograma_el_dibujo_no_cambia() {
    let mut app = workspace();
    let primero = pintar(&mut app);
    assert_eq!(app.logo_fx_frame, None);
    assert_eq!(primero, pintar(&mut app));
}

#[test]
fn con_fotograma_cambia_la_marca_y_nada_fuera_de_ella() {
    let mut app = workspace();
    let sin = pintar(&mut app);
    let rect = rect_de_la_marca(&app);

    app.logo_fx_frame = Some((LogoEffect::Flashlight, 0.5, 2));
    let con = pintar(&mut app);

    let dentro_cambia = (rect.x..rect.x + rect.width)
        .flat_map(|x| (rect.y..rect.y + rect.height).map(move |y| (x, y)))
        .any(|(x, y)| sin[(x, y)] != con[(x, y)]);
    assert!(dentro_cambia, "el foco debía iluminar algo de la marca");

    for y in 0..sin.area.height {
        for x in 0..sin.area.width {
            if !rect.contains(ratatui::layout::Position::new(x, y)) {
                assert_eq!(sin[(x, y)], con[(x, y)], "celda ({x}, {y}) fuera del logo");
            }
        }
    }
}

#[test]
fn cada_efecto_se_queda_dentro_del_rectangulo_del_logo() {
    let mut app = workspace();
    let sin = pintar(&mut app);
    let rect = rect_de_la_marca(&app);
    for efecto in LogoEffect::ALL {
        for progreso in [0.0f32, 0.5, 1.0] {
            app.logo_fx_frame = Some((efecto, progreso, 9));
            let con = pintar(&mut app);
            for y in 0..sin.area.height {
                for x in 0..sin.area.width {
                    if !rect.contains(ratatui::layout::Position::new(x, y)) {
                        assert_eq!(
                            sin[(x, y)],
                            con[(x, y)],
                            "{efecto:?} p={progreso} tocó ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn la_ayuda_muestra_logofx() {
    let mut app = App::with_context("/proj", "m");
    app.help_open = true;
    let texto = buffer_of(&mut app, 80, 24);
    assert!(texto.contains("/logofx"), "{texto}");
}
