//! Teclado del selector de `/effort`: ↑/↓ mueven sin dar la vuelta, `Enter` aplica y `Esc`
//! cierra sin cambios. Se despacha antes que las demás ventanas, igual que el de `/model`.

use super::*;

/// Teclas con el selector de `/effort` abierto.
pub(super) fn handle_effort_picker_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.close_quick_effort_picker(),
        KeyCode::Up => app.effort_picker_move(false),
        KeyCode::Down => app.effort_picker_move(true),
        KeyCode::Enter => app.choose_quick_effort(),
        _ => {}
    }
}
