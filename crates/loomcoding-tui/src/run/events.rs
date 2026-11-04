//! Reacción a los eventos del motor y envío de órdenes al worker.

use super::*;

/// Aplica un evento de terminal. Devuelve el prompt a enviar, si lo hay.
pub fn handle_event(app: &mut App, ev: Event) -> Option<String> {
    // Teclado y pegado pueden cambiar el texto: cualquiera anula el arrastre en curso.
    if matches!(ev, Event::Key(_) | Event::Paste(_)) {
        app.mouse_drag = None;
    }
    match ev {
        Event::Key(k) if k.kind == KeyEventKind::Press => handle_key(app, k.code, k.modifiers),
        Event::Key(k)
            if k.kind == KeyEventKind::Repeat
                && !k
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && app.permission.is_none()
                && !app.session_picker_open
                && !app.session_loading
                && !app.queue_open
                && !app.settings_open
                && !app.help_open
                && !app.agents_open
                && !app.tools_open
                && match k.code {
                    KeyCode::Char(_) => app.focus == Focus::Composer,
                    KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Backspace
                    | KeyCode::Delete => true,
                    _ => false,
                } =>
        {
            handle_key(app, k.code, k.modifiers)
        }
        Event::Mouse(mouse)
            if !app.session_picker_open
                && !app.session_loading
                && !app.settings_open
                && !app.help_open
                && !app.agents_open
                && !app.tools_open
                && !app.queue_open
                && app.permission.is_none()
                && app.question.is_none() =>
        {
            match mouse.kind {
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    if app.composer_area.is_some_and(|area| {
                        mouse.column >= area.x
                            && mouse.column < area.right()
                            && mouse.row >= area.y
                            && mouse.row < area.bottom()
                    }) =>
                {
                    app.focus = Focus::Composer;
                    loomcoding_cloth::widgets::composer::scroll_cursor_by_rows(
                        &app.theme,
                        &mut app.composer,
                        crate::app::composer_text_area(app.composer_area.unwrap()),
                        mouse.kind == MouseEventKind::ScrollDown,
                        3,
                    );
                }
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    let down = mouse.kind == MouseEventKind::ScrollDown;
                    let pane = app.pane_at(mouse.column, mouse.row);
                    if let Some(pane) = pane.filter(|pane| {
                        matches!(
                            pane,
                            Focus::Activity | Focus::Mcp | Focus::Lsp | Focus::Skills
                        )
                    }) {
                        app.focus = pane;
                        scroll_dashboard_widget(app, pane, down, 2);
                    } else if down {
                        app.viewport.scroll_down(3);
                    } else {
                        app.viewport.scroll_up(3);
                    }
                }
                MouseEventKind::Down(MouseButton::Left) => app.mouse_down(mouse.column, mouse.row),
                MouseEventKind::Drag(MouseButton::Left) => {
                    app.mouse_drag_to(mouse.column, mouse.row);
                }
                MouseEventKind::Up(MouseButton::Left) => app.mouse_up(),
                _ => {}
            }
            None
        }
        // Con un diálogo abierto el ratón no llega a la interfaz: un arrastre
        // pendiente se descarta para que no sobreviva al diálogo.
        Event::Mouse(_) => {
            app.mouse_drag = None;
            app.text_drag = false;
            None
        }
        Event::Paste(text)
            if !app.session_picker_open
                && !app.session_loading
                && !app.help_open
                && !app.agents_open
                && !app.tools_open
                && app.permission.is_none()
                && app.question.is_none() =>
        {
            if app.settings_open {
                if app.settings_editing && !app.settings_saving {
                    let pasted = text.replace(['\r', '\n'], "");
                    if app.settings_field != 3
                        || app.settings_editor.text().len() + pasted.len() <= 4096
                    {
                        app.settings_editor.insert_str(&pasted);
                    } else {
                        app.settings_feedback = Some("API key is too long".into());
                    }
                }
            } else if app.queue_open && app.queue_editing {
                app.queue_editor.insert_str(&text);
            } else if !app.queue_open {
                app.text_sel = None;
                if !paste_clipboard_image(app) {
                    app.paste(&text);
                }
            }
            None
        }
        _ => None,
    }
}

/// Encola prompts y mantiene un solo turno enviado al worker. Un mensaje del
/// usuario rearma el presupuesto de turnos automáticos.
pub(super) fn send(
    app: &mut App,
    tx: &UnboundedSender<crate::app::PendingPrompt>,
    prompt: Option<String>,
) {
    if let Some(text) = prompt {
        app.rearm_auto_turns();
        let images = app.take_draft_images();
        app.pending_prompts
            .push_back(crate::app::PendingPrompt { text, images });
        app.queued = app.pending_prompts.len();
    }
    dispatch_next(app, tx);
}

/// Pide un despertar del padre si la interfaz lo permite. Si el canal del
/// trabajador está cerrado, suelta la marca para no bloquear la cola de prompts.
pub(super) fn dispatch_wake(app: &mut App, wake_tx: &UnboundedSender<()>) {
    if app.take_wake() && wake_tx.send(()).is_err() {
        app.wake_in_flight = false;
        app.status = "error: wake channel closed".into();
    }
}

/// Entrega al trabajador la compactación manual pedida. Si el canal está cerrado, la suelta para no
/// dejar la cola de prompts bloqueada.
pub(super) fn dispatch_compact(app: &mut App, compact_tx: &UnboundedSender<()>) {
    if app.take_compact() && compact_tx.send(()).is_err() {
        app.compact_in_flight = false;
        app.status = "error: compaction channel closed".into();
    }
}

pub(super) fn dispatch_next(app: &mut App, tx: &UnboundedSender<crate::app::PendingPrompt>) {
    // Un despertar o una compactación en vuelo ocupan el turno: no se solapa un prompt con ellos.
    if app.turn_in_flight
        || app.wake_in_flight
        || app.compact_requested
        || app.compact_in_flight
        || app.queue_open
        || app.queue_paused
        || app.session_loading
        || app.session_picker_open
        || app.settings_open
        || app.settings_saving
    {
        return;
    }
    if let Some(prompt) = app.pending_prompts.pop_front() {
        match tx.send(prompt.clone()) {
            Ok(()) => app.turn_in_flight = true,
            Err(_) => {
                app.status = "error: agent channel closed".into();
                app.pending_prompts.push_front(prompt);
            }
        }
        app.queued = app.pending_prompts.len();
        app.queue_selected = app.queue_selected.min(app.queued.saturating_sub(1));
    }
}

pub(super) fn cancel_active_turn(app: &mut App, active_turn: &ActiveTurn) {
    if !app.cancel_requested {
        return;
    }
    if let Ok(active) = active_turn.lock() {
        if let Some(token) = active.as_ref() {
            token.cancel();
            app.cancel_requested = false;
            app.status = "cancelling".into();
            while app.permission.is_some() {
                app.resolve_permission(loomcoding_core::permission::Decision::Deny);
            }
            app.cancel_question();
        }
    }
}

/// Aplica a la interfaz el catálogo recibido de la carga en segundo plano.
pub(super) fn apply_catalog(app: &mut App, catalog: Arc<Catalog>) {
    app.set_catalog(catalog);
}
