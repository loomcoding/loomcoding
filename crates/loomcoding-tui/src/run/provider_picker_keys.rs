//! Teclado del selector rápido de `/provider`: escribir filtra, ↑/↓ mueven, `Enter` aplica y `Esc`
//! cierra sin cambios. Se despacha antes que las demás ventanas, igual que el de `/model`.

use super::*;

/// Teclas con el selector de proveedores abierto.
pub(super) fn handle_provider_picker_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.close_quick_provider_picker(),
        KeyCode::Up => app.provider_picker_move(false),
        KeyCode::Down => app.provider_picker_move(true),
        KeyCode::Enter => app.choose_quick_provider(),
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
