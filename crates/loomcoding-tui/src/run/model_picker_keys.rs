//! Teclado del selector rápido de `/model`: escribir filtra, ↑/↓ mueven, `Enter` aplica y `Esc`
//! cierra sin cambios. Se despacha antes que las demás ventanas, igual que `/settings`.

use super::*;

/// Teclas con el selector rápido de `/model` abierto.
pub(super) fn handle_model_picker_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.close_quick_model_picker(),
        KeyCode::Up => app.picker_move(false),
        KeyCode::Down => app.picker_move(true),
        KeyCode::Enter => app.choose_quick_model(),
        KeyCode::Backspace => {
            app.picker_query.pop();
            app.picker_selected = 0;
        }
        KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            app.picker_query.push(c);
            app.picker_selected = 0;
        }
        _ => {}
    }
}
