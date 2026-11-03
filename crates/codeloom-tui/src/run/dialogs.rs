//! Teclado de las ventanas modales: cola, temas, ajustes y selector.

use super::*;

pub(super) fn handle_queue_editor_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.queue_editing = false,
        KeyCode::Enter => {
            if !app.queue_editor.text().trim().is_empty() {
                if let Some(prompt) = app.pending_prompts.get_mut(app.queue_selected) {
                    prompt.text = app.queue_editor.text().to_string();
                }
                app.queue_editing = false;
            }
        }
        KeyCode::Char('j') if mods.contains(KeyModifiers::CONTROL) => app.queue_editor.newline(),
        KeyCode::Char('a') if mods.contains(KeyModifiers::CONTROL) => app.queue_editor.set_text(""),
        KeyCode::Backspace => app.queue_editor.backspace(),
        KeyCode::Delete => app.queue_editor.delete(),
        KeyCode::Left => app.queue_editor.move_left(),
        KeyCode::Right => app.queue_editor.move_right(),
        KeyCode::Up => app.queue_editor.move_up(),
        KeyCode::Down => app.queue_editor.move_down(),
        KeyCode::Home => app.queue_editor.move_home(),
        KeyCode::End => app.queue_editor.move_end(),
        KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            app.queue_editor.insert_char(c);
        }
        _ => {}
    }
}

/// Teclado de la ventana `/temas`: árbol con `↑`/`↓`, `Enter` para entrar,
/// `Esc` para subir un nivel y `←`/`→` para ajustar el campo del formulario.
pub(super) fn handle_temas_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => app.temas_back(),
        KeyCode::Up => app.temas_move(-1),
        KeyCode::Down => app.temas_move(1),
        KeyCode::Enter => app.temas_enter(),
        KeyCode::Left => app.temas_adjust(false),
        KeyCode::Right => app.temas_adjust(true),
        KeyCode::Char('s') => app.temas_save(),
        _ => {}
    }
}

pub(super) fn handle_settings_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    if app.settings_saving {
        if code == KeyCode::Esc {
            app.settings_open = false;
        }
        return;
    }
    if app.settings_editing {
        match code {
            KeyCode::Esc => app.settings_editing = false,
            KeyCode::Enter => app.commit_settings_value(),
            KeyCode::Char('a') if mods.contains(KeyModifiers::CONTROL) => {
                app.settings_editor.set_text("")
            }
            KeyCode::Backspace => app.settings_editor.backspace(),
            KeyCode::Delete => app.settings_editor.delete(),
            KeyCode::Left => app.settings_editor.move_left(),
            KeyCode::Right => app.settings_editor.move_right(),
            KeyCode::Home => app.settings_editor.move_home(),
            KeyCode::End => app.settings_editor.move_end(),
            KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                if app.settings_field != 3
                    || app.settings_editor.text().len() + c.len_utf8() <= 4096
                {
                    app.settings_editor.insert_char(c);
                }
            }
            _ => {}
        }
        return;
    }
    if app.settings_picker.is_some() {
        handle_picker_key(app, code, mods);
        return;
    }
    if code != KeyCode::Esc {
        app.settings_disarm();
    }
    match code {
        KeyCode::Esc => {
            app.close_settings();
        }
        KeyCode::Up => app.settings_field = app.settings_field.saturating_sub(1),
        KeyCode::Down => {
            app.settings_field = (app.settings_field + 1).min(app.settings_save_field())
        }
        KeyCode::Left | KeyCode::Right if app.settings_field == 0 => toggle_provider(app),
        KeyCode::Left if app.settings_field == SETTINGS_EFFORT_FIELD => app.cycle_effort(false),
        KeyCode::Right if app.settings_field == SETTINGS_EFFORT_FIELD => app.cycle_effort(true),
        KeyCode::Enter if app.settings_field == 0 => app.open_picker(SettingsPicker::Provider),
        // Con catálogo, «Model» abre la lista de su proveedor; sin él, se edita a mano.
        KeyCode::Enter if app.settings_field == 1 && !app.catalog.is_empty() => {
            app.open_picker(SettingsPicker::Model)
        }
        KeyCode::Enter if (1..=3).contains(&app.settings_field) => app.edit_settings_value(),
        KeyCode::Enter if app.settings_field == app.settings_save_field() => {
            app.request_settings_save()
        }
        KeyCode::Enter if app.settings_field == SETTINGS_EFFORT_FIELD => app.cycle_effort(true),
        KeyCode::Char('l') if mods.contains(KeyModifiers::CONTROL) => match app.settings_field {
            0 => app.open_picker(SettingsPicker::Provider),
            1 => app.open_picker(SettingsPicker::Model),
            _ => {}
        },
        KeyCode::Char('s') if mods.contains(KeyModifiers::CONTROL) => app.request_settings_save(),
        _ => {}
    }
}

/// Teclas con el selector de `/settings` abierto: escribir filtra, ↑/↓ mueven, Enter
/// elige y Esc cierra solo el selector, nunca el diálogo.
pub(super) fn handle_picker_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    if code == KeyCode::Esc {
        app.close_picker();
        return;
    }
    app.settings_disarm();
    match code {
        KeyCode::Up => app.picker_move(false),
        KeyCode::Down => app.picker_move(true),
        KeyCode::Enter => app.picker_choose(),
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

pub(super) fn toggle_provider(app: &mut App) {
    if let Some(settings) = app.provider_settings.as_mut() {
        settings.kind = match settings.kind {
            codeloom_core::config::ProviderKind::OpenAiCompat => {
                codeloom_core::config::ProviderKind::OpenAiResponses
            }
            codeloom_core::config::ProviderKind::OpenAiResponses => {
                codeloom_core::config::ProviderKind::Anthropic
            }
            codeloom_core::config::ProviderKind::Anthropic => {
                codeloom_core::config::ProviderKind::Gemini
            }
            codeloom_core::config::ProviderKind::Gemini => {
                codeloom_core::config::ProviderKind::OpenAiCompat
            }
        };
        let (base_url, api_key_env) = match settings.kind {
            codeloom_core::config::ProviderKind::OpenAiCompat => {
                ("https://openrouter.ai/api/v1", "OPENROUTER_API_KEY")
            }
            codeloom_core::config::ProviderKind::OpenAiResponses => {
                ("https://api.openai.com/v1", "OPENAI_API_KEY")
            }
            codeloom_core::config::ProviderKind::Anthropic => {
                ("https://api.anthropic.com", "ANTHROPIC_API_KEY")
            }
            codeloom_core::config::ProviderKind::Gemini => (
                "https://generativelanguage.googleapis.com/v1beta",
                "GEMINI_API_KEY",
            ),
        };
        settings.base_url = base_url.into();
        settings.api_key_env = api_key_env.into();
        app.settings_api_key.clear();
        app.settings_feedback = Some(
            "Provider endpoint and key variable updated; review the model before saving".into(),
        );
    }
}
