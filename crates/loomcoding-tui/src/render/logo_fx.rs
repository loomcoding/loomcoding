//! Efecto del logo de cabecera sobre el menú izquierdo.
//!
//! Solo lee `app.logo_fx_frame`, que fija `App::tick_logo_fx` fuera del render: aquí no se
//! consulta el reloj. El efecto se aplica únicamente sobre el rectángulo exacto de la marca.

use crate::app::App;
use loomcoding_cloth::widgets::logo_fx::apply;
use loomcoding_cloth::widgets::sidebar::logo_rect;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Pinta el fotograma vigente del logo sobre la marca del sidebar, si hay uno. `area` es el
/// rectángulo que recibió el sidebar; la marca está en su esquina superior izquierda, dentro del
/// borde. Sin fotograma, o sin marca que pintar, no toca nada.
pub(super) fn draw_logo_fx(frame: &mut Frame, area: Rect, app: &App) {
    let Some((effect, progress, seed)) = app.logo_fx_frame else {
        return;
    };
    let Some(rect) = logo_rect(area, &app.sidebar()) else {
        return;
    };
    apply(effect, progress, frame.buffer_mut(), rect, &app.theme, seed);
}
