//! Despacho de teclas hacia la acción o hacia la demo abierta.

use super::*;

/// Avanza una posición dentro de `0..len` sin dar la vuelta.
pub(super) fn step(position: usize, delta: isize, len: usize) -> usize {
    (position as isize + delta).clamp(0, len as isize - 1) as usize
}

/// Resuelve una tecla. Las globales primero; el resto, al demo si tiene el foco.
pub fn route(show: &mut Showcase, kind: KeyEventKind, code: KeyCode, mods: KeyModifiers) -> Action {
    if kind != KeyEventKind::Press {
        return Action::None;
    }
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        show.quit = true;
        return Action::Quit;
    }
    if code == KeyCode::F(7) {
        show.cycle_theme_column(mods.contains(KeyModifiers::SHIFT));
        return Action::None;
    }
    match show.focus {
        Focus::Index => match code {
            KeyCode::Up | KeyCode::Char('k') => {
                show.prev();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                show.next();
                Action::None
            }
            KeyCode::Left => {
                show.cycle_theme_column(true);
                Action::None
            }
            KeyCode::Right => {
                show.cycle_theme_column(false);
                Action::None
            }
            KeyCode::Tab => {
                show.toggle_focus();
                Action::None
            }
            KeyCode::Char('/') => {
                show.focus = Focus::TextFilter;
                Action::None
            }
            KeyCode::Char('m') => {
                show.open_matrix();
                Action::None
            }
            KeyCode::Char('q') => {
                show.quit = true;
                Action::Quit
            }
            _ => Action::None,
        },
        Focus::TextFilter => match code {
            KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                show.push_query(c);
                Action::None
            }
            KeyCode::Backspace => {
                show.pop_query();
                Action::None
            }
            KeyCode::Enter | KeyCode::Tab => {
                show.focus = Focus::Index;
                Action::None
            }
            KeyCode::Esc => {
                show.clear_query();
                show.focus = Focus::Index;
                Action::None
            }
            KeyCode::Up | KeyCode::Down => {
                show.focus = Focus::Index;
                if code == KeyCode::Up {
                    show.prev();
                } else {
                    show.next();
                }
                Action::None
            }
            _ => Action::None,
        },
        Focus::Matrix => match code {
            KeyCode::Up | KeyCode::Char('k') => {
                show.matrix_move(-1, 0);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                show.matrix_move(1, 0);
                Action::None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                show.matrix_move(0, -1);
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                show.matrix_move(0, 1);
                Action::None
            }
            KeyCode::Enter => {
                show.open_matrix_cell();
                Action::None
            }
            KeyCode::Esc | KeyCode::Char('m') => {
                show.set_focus(Focus::Index);
                Action::None
            }
            _ => Action::None,
        },
        Focus::Demo => match code {
            KeyCode::Esc => {
                show.leave_demo();
                Action::None
            }
            KeyCode::Tab => {
                show.toggle_focus();
                Action::None
            }
            other => {
                if let Some(d) = show.current() {
                    d.handle_key_event(other, mods);
                }
                Action::None
            }
        },
    }
}
