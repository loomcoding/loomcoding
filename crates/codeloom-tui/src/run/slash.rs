//! Ejecución de los comandos con barra y confirmación de preferencias.

use super::*;

pub(super) fn execute_slash_command(
    app: &mut App,
    command: crate::commands::Command,
) -> Option<String> {
    use crate::commands::Command;
    // `/verbose` lee su argumento del texto escrito, que se borra justo abajo.
    let verbose_arg = app.composer.text().trim().to_string();
    if command == Command::Compact {
        // Con motivo de rechazo queda en `slash_feedback` y se conserva el texto escrito.
        if !app.request_compact() {
            return None;
        }
    }
    if matches!(command, Command::New | Command::Sessions) && app.turn_in_flight {
        app.slash_feedback = Some("Wait for the active turn before switching sessions".into());
        return None;
    }
    if command == Command::Settings && app.provider_settings.is_none() {
        app.slash_feedback = Some("Provider settings are unavailable".into());
        return None;
    }
    if command == Command::Cancel && !app.turn_in_flight {
        app.slash_feedback = Some("No active turn to cancel".into());
        return None;
    }
    app.composer.set_text("");
    app.slash_selected = 0;
    app.slash_dismissed = None;
    app.history.reset();
    match command {
        Command::Help => handle_key(app, KeyCode::F(1), KeyModifiers::NONE),
        Command::Sessions => handle_key(app, KeyCode::F(3), KeyModifiers::NONE),
        Command::Settings => handle_key(app, KeyCode::F(2), KeyModifiers::NONE),
        Command::Queue => handle_key(app, KeyCode::Char('k'), KeyModifiers::CONTROL),
        Command::Tools => handle_key(app, KeyCode::Char('t'), KeyModifiers::CONTROL),
        Command::Theme => {
            app.cycle_theme();
            confirm_theme(app);
            None
        }
        Command::Temas => {
            app.open_temas();
            None
        }
        Command::Crush => {
            app.set_theme(batten::widgets::ThemePreset::Crush);
            confirm_theme(app);
            None
        }
        Command::Classic => {
            app.set_theme(batten::widgets::ThemePreset::Batten);
            confirm_theme(app);
            None
        }
        Command::Sidebar => handle_key(app, KeyCode::Char('b'), KeyModifiers::CONTROL),
        Command::Agent => {
            app.composer.set_text("/agent ");
            app.slash_feedback = Some(crate::agent_command::USAGE.into());
            None
        }
        Command::Focus => {
            app.begin_focus_command();
            None
        }
        Command::Agents => {
            app.show_agents();
            None
        }
        Command::A2a => {
            app.a2a_open = true;
            None
        }
        Command::RefreshAgents => {
            app.request_refresh_agents();
            None
        }
        Command::Mouse => {
            app.toggle_mouse_capture();
            confirm_mouse(app);
            None
        }
        Command::Verbose => {
            // El texto todavía contiene `/verbose ...`; nos quedamos con lo que sigue al nombre.
            let arg = verbose_arg
                .strip_prefix("/verbose")
                .map(str::trim)
                .unwrap_or("");
            match arg {
                "" => {
                    let on = app.toggle_provider_notices();
                    app.slash_feedback = Some(
                        if on {
                            "Provider notices: on"
                        } else {
                            "Provider notices: off"
                        }
                        .into(),
                    );
                }
                "on" => {
                    app.set_provider_notices_verbose(true);
                    app.slash_feedback = Some("Provider notices: on".into());
                }
                "off" => {
                    app.set_provider_notices_verbose(false);
                    app.slash_feedback = Some("Provider notices: off".into());
                }
                _ => {
                    app.slash_feedback = Some("Usage: /verbose [on|off]".into());
                }
            }
            None
        }
        Command::Cancel => {
            app.queue_paused = true;
            app.request_cancel();
            None
        }
        Command::Pause | Command::Resume => {
            app.queue_paused = command == Command::Pause;
            app.slash_feedback = Some(if app.queue_paused {
                "Pending prompts paused".into()
            } else {
                "Pending prompts resumed".into()
            });
            None
        }
        // Ya pedida arriba: el bucle la entrega al trabajador con `dispatch_compact`.
        Command::Compact => None,
        Command::Quit => handle_key(app, KeyCode::Char('q'), KeyModifiers::CONTROL),
        Command::Parent => {
            if !app.go_parent() {
                app.status = if app.turn_in_flight {
                    "wait for the turn to finish".into()
                } else {
                    "no parent session".into()
                };
            }
            None
        }
        Command::New => {
            app.session_choice = Some(SessionChoice::New);
            app.session_loading = true;
            None
        }
    }
}

/// Confirma el tema elegido en el composer. El look Crush oculta la cabecera con
/// el nombre del tema, así que se muestra aquí. Si el guardado falló, se conserva
/// ese aviso en lugar de la confirmación.
pub(super) fn confirm_theme(app: &mut App) {
    let name = app.theme_preset.name();
    app.slash_feedback
        .get_or_insert_with(|| format!("Theme: {name}"));
}

/// Confirma la captura del ratón elegida en el composer. Si el guardado falló,
/// se conserva ese aviso en lugar de la confirmación.
pub(super) fn confirm_mouse(app: &mut App) {
    let message = if app.mouse_capture {
        "Mouse capture on (wheel scrolls)"
    } else {
        "Mouse capture off — select text with the mouse"
    };
    app.slash_feedback
        .get_or_insert_with(|| message.to_string());
}
