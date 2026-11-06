//! Teclas del menú izquierdo con el foco en él: `↑`/`↓` eligen fichero y `Enter` despliega o
//! pliega su diff. Desde el compositor, `Shift+Tab` lleva al menú cuando muestra ficheros.
//!
//! Cabe señalar que con el foco en el compositor no se consume ninguna tecla de aquí: el texto
//! del compositor no cambia.

use super::*;

/// Atiende la tecla si le toca al menú izquierdo. Devuelve `true` si la consumió. Sin ficheros,
/// nada se consume y cada tecla sigue su camino de siempre.
pub(super) fn sidebar_key(app: &mut App, code: KeyCode) -> bool {
    if app.changes.is_empty() {
        return false;
    }
    match (app.focus, code) {
        (Focus::Sidebar, KeyCode::Up) => app.diff_select(-1),
        (Focus::Sidebar, KeyCode::Down) => app.diff_select(1),
        (Focus::Sidebar, KeyCode::Enter) => app.diff_toggle(),
        (Focus::Composer, KeyCode::BackTab) if menu_pintado(app) => {
            app.focus = Focus::Sidebar;
        }
        _ => return false,
    }
    true
}

/// `true` si el último fotograma pintó el menú izquierdo.
fn menu_pintado(app: &App) -> bool {
    app.pane_rects
        .iter()
        .any(|(pane, _)| *pane == Focus::Sidebar)
}
