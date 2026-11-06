//! Teclado del composer, del transcript y del diálogo de preguntas.

use super::sidebar_keys::sidebar_key;
use super::*;

/// Teclas con el foco en el transcript (como Crush).
pub(super) fn transcript_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let len = app.transcript.len();
    if let Some(mut s) = app.selection {
        match code {
            KeyCode::Up | KeyCode::Char('k') => s.extend(-1, len),
            KeyCode::Down | KeyCode::Char('j') => s.extend(1, len),
            KeyCode::Char('y') | KeyCode::Char('c') => {
                app.status = format!("copiado ({} líneas)", s.range().count());
                app.clipboard = Some(s.text(&app.transcript));
                app.selection = None;
                return;
            }
            KeyCode::Char('v') => {
                app.selection = None;
                return;
            }
            _ => return,
        }
        app.selection = Some(s);
        return;
    }
    if code == KeyCode::Char('v') && len > 0 {
        // La selección de líneas y la de texto son excluyentes.
        app.text_sel = None;
        app.selection = Some(loomcoding_cloth::state::Selection::at(len - 1));
        return;
    }
    // En un nodo de subagente, → despliega, ← pliega y Enter alterna.
    let despliegue = match code {
        KeyCode::Right => Some(true),
        KeyCode::Left => Some(false),
        _ => None,
    };
    if let Some(expanded) = despliegue {
        if app.set_node_expanded(expanded) {
            return;
        }
    }
    if code == KeyCode::Enter && app.toggle_node_expanded() {
        return;
    }
    // Espacio o Enter pliegan y despliegan el bloque activo si es largo; si no, siguen su camino
    // (el espacio pagina).
    if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) && app.toggle_block_fold() {
        return;
    }
    // Con el chrome `Flat` (Crush), flechas y j/k avanzan por bloques; Ctrl+flecha y
    // Ctrl+j siguen desplazando por líneas. Con Classic, todo igual que siempre.
    let by_block = app.theme.look.chrome == loomcoding_cloth::widgets::ChromeStyle::Flat
        && !mods.contains(KeyModifiers::CONTROL);
    match code {
        KeyCode::Up | KeyCode::Char('k') if by_block => {
            app.block_prev();
            return;
        }
        KeyCode::Down | KeyCode::Char('j') if by_block => {
            app.block_next();
            return;
        }
        _ => {}
    }
    // Navegación y copia por bloques: no alteran el desplazamiento por líneas.
    match code {
        KeyCode::Char('[') => app.block_prev(),
        KeyCode::Char(']') => app.block_next(),
        KeyCode::Char('y') | KeyCode::Char('c') => {
            app.copy_block();
        }
        _ => {}
    }
    let vp = &mut app.viewport;
    match code {
        KeyCode::Up | KeyCode::Char('k') => vp.scroll_up(1),
        KeyCode::Down | KeyCode::Char('j') => vp.scroll_down(1),
        KeyCode::Char('u') => vp.half_page_up(),
        KeyCode::Char('d') => vp.half_page_down(),
        KeyCode::PageUp | KeyCode::Char('b') => vp.page_up(),
        KeyCode::PageDown | KeyCode::Char('f') | KeyCode::Char(' ') => vp.page_down(),
        KeyCode::Home | KeyCode::Char('g') => vp.to_top(),
        KeyCode::End | KeyCode::Char('G') => vp.follow_bottom(),
        _ => {}
    }
}

pub(super) fn scroll_dashboard_widget(
    app: &mut App,
    focus: Focus,
    down: bool,
    amount: u16,
) -> bool {
    let offset = match focus {
        Focus::Activity => &mut app.activity_scroll,
        Focus::Mcp => &mut app.capability_scroll[0],
        Focus::Lsp => &mut app.capability_scroll[1],
        Focus::Skills => &mut app.capability_scroll[2],
        _ => return false,
    };
    *offset = if down {
        offset.saturating_add(amount)
    } else {
        offset.saturating_sub(amount)
    };
    true
}

/// Traduce una tecla a una accion sobre la app. Devuelve el prompt a enviar
/// cuando el usuario pulsa Enter con texto.
/// Traduce una tecla de crossterm a la del diálogo de preguntas.
pub(super) fn question_key_of(code: KeyCode) -> Option<loomcoding_cloth::state::QuestionKey> {
    use loomcoding_cloth::state::QuestionKey;
    Some(match code {
        KeyCode::Up => QuestionKey::Up,
        KeyCode::Down => QuestionKey::Down,
        KeyCode::Left => QuestionKey::Left,
        KeyCode::Right => QuestionKey::Right,
        KeyCode::Enter => QuestionKey::Enter,
        KeyCode::Esc => QuestionKey::Esc,
        KeyCode::Backspace => QuestionKey::Backspace,
        KeyCode::Char(c) => QuestionKey::Char(c),
        _ => return None,
    })
}

/// Cabe señalar que `?` solo abre la ayuda si el compositor está vacío, no hay ningún panel,
/// popup ni diálogo abierto, y no llega con Ctrl o Alt. En cualquier otro caso es un carácter más.
fn help_key_is_free(app: &App, mods: KeyModifiers) -> bool {
    !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && app.composer.is_empty()
        && app.permission.is_none()
        && app.question.is_none()
        && !(app.settings_open
            || app.temas_open
            || app.a2a_open
            || app.agents_open
            || app.tools_open
            || app.queue_open
            || app.session_picker_open)
}

pub fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) -> Option<String> {
    if app.slash_query().is_none() {
        app.slash_feedback = None;
    }
    if app
        .slash_dismissed
        .as_deref()
        .is_some_and(|text| text != app.composer.text())
    {
        app.slash_dismissed = None;
    }
    // Ctrl+C con una selección del composer la copia y no sale. Sin selección,
    // conserva su salida de siempre.
    if code == KeyCode::Char('c')
        && mods.contains(KeyModifiers::CONTROL)
        && app.composer.selection().is_some()
    {
        app.copy_composer_selection();
        return None;
    }
    // Igual con una selección de texto del transcript: la copia y no sale.
    if code == KeyCode::Char('c')
        && mods.contains(KeyModifiers::CONTROL)
        && app.text_sel.is_some_and(|sel| !sel.is_empty())
    {
        app.copy_transcript_selection();
        return None;
    }
    if matches!(code, KeyCode::Char('c' | 'q')) && mods.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return None;
    }
    if app.settings_open {
        handle_settings_key(app, code, mods);
        return None;
    }
    if app.temas_open {
        handle_temas_key(app, code);
        return None;
    }
    if app.help_open {
        if matches!(code, KeyCode::F(1) | KeyCode::Esc | KeyCode::Char('?')) {
            app.help_open = false;
        }
        return None;
    }
    if app.a2a_open {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => app.a2a_open = false,
            KeyCode::Tab | KeyCode::Right => app.a2a_focus_next(),
            KeyCode::Up => app.a2a_move(-1),
            KeyCode::Down => app.a2a_move(1),
            KeyCode::PageUp => app.a2a_detail_scroll = app.a2a_detail_scroll.saturating_sub(5),
            KeyCode::PageDown => app.a2a_detail_scroll = app.a2a_detail_scroll.saturating_add(5),
            KeyCode::Home => app.a2a_detail_scroll = 0,
            _ => {}
        }
        return None;
    }
    if app.agents_open && app.agent_edit.is_some() {
        let editing_value = app
            .agent_edit
            .as_ref()
            .is_some_and(|edit| edit.editing_value);
        if editing_value {
            match code {
                KeyCode::Esc => app.finish_agent_edit_value(false),
                KeyCode::Enter => app.finish_agent_edit_value(true),
                KeyCode::Char('j') if mods.contains(KeyModifiers::CONTROL) => {
                    app.agent_editor.newline()
                }
                KeyCode::Char('a') if mods.contains(KeyModifiers::CONTROL) => {
                    app.agent_editor.set_text("")
                }
                KeyCode::Backspace => app.agent_editor.backspace(),
                KeyCode::Delete => app.agent_editor.delete(),
                KeyCode::Left => app.agent_editor.move_left(),
                KeyCode::Right => app.agent_editor.move_right(),
                KeyCode::Up => app.agent_editor.move_up(),
                KeyCode::Down => app.agent_editor.move_down(),
                KeyCode::Home => app.agent_editor.move_home(),
                KeyCode::End => app.agent_editor.move_end(),
                KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    if app.agent_editor.text().len() + c.len_utf8() <= 16 * 1024 {
                        app.agent_editor.insert_char(c);
                    }
                }
                _ => {}
            }
        } else {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => app.cancel_agent_edit(),
                KeyCode::Up => app.move_agent_edit_field(-1),
                KeyCode::Down => app.move_agent_edit_field(1),
                KeyCode::Home => app.move_agent_edit_field(isize::MIN / 2),
                KeyCode::End => app.move_agent_edit_field(isize::MAX / 2),
                KeyCode::Enter | KeyCode::Char('e') => app.start_agent_edit_value(),
                KeyCode::Char('s') => app.save_agent_edit(),
                _ => {}
            }
        }
        return None;
    }
    if app.agents_open {
        // Modo edición de filtros (Query o Tag)
        if app.agents_filter_edit.is_some() {
            match code {
                KeyCode::Esc => {
                    app.agents_filter_edit = None;
                    app.agents_focus = AgentsFocus::List;
                    app.agents_tag_input.clear();
                }
                KeyCode::Enter => {
                    match app.agents_filter_edit.unwrap() {
                        AgentFilterEdit::Query => {
                            app.agents_filter_edit = None;
                            app.agents_focus = AgentsFocus::List;
                        }
                        AgentFilterEdit::Tag => {
                            // Añadir el tag escrito y limpiar buffer
                            let tag = app.agents_tag_input.trim().to_string();
                            if !tag.is_empty() {
                                app.add_agent_tag_filter(tag);
                            }
                            app.agents_tag_input.clear();
                        }
                    }
                }
                KeyCode::Tab => app.cycle_agents_focus(true),
                KeyCode::BackTab => app.cycle_agents_focus(false),
                KeyCode::Char('a') if mods.contains(KeyModifiers::CONTROL) => {
                    match app.agents_filter_edit.unwrap() {
                        AgentFilterEdit::Query => app.set_agents_query(String::new()),
                        AgentFilterEdit::Tag => {
                            app.set_agents_tag_filter(None);
                            app.agents_tag_input.clear();
                        }
                    }
                }
                KeyCode::Backspace => match app.agents_filter_edit.unwrap() {
                    AgentFilterEdit::Query => {
                        let mut query = app.agents_query.clone();
                        query.pop();
                        app.set_agents_query(query);
                    }
                    AgentFilterEdit::Tag => {
                        if !app.agents_tag_input.is_empty() {
                            app.agents_tag_input.pop();
                        } else if let Some(tags) = app.agents_tag_filter.clone() {
                            if let Some(last) = tags.last().cloned() {
                                app.remove_agent_tag_filter(&last);
                            }
                        }
                    }
                },
                KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    match app.agents_filter_edit.unwrap() {
                        AgentFilterEdit::Query if app.agents_query.len() + c.len_utf8() <= 128 => {
                            let mut query = app.agents_query.clone();
                            query.push(c);
                            app.set_agents_query(query);
                        }
                        AgentFilterEdit::Tag if app.agents_tag_input.len() + c.len_utf8() <= 64 => {
                            app.agents_tag_input.push(c);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return None;
        }

        // Modo navegación lista
        match code {
            KeyCode::Esc | KeyCode::Char('q') => app.agents_open = false,
            KeyCode::Up => app.agents_move(-1),
            KeyCode::Down => app.agents_move(1),
            KeyCode::Tab => app.cycle_agents_focus(true),
            KeyCode::BackTab => app.cycle_agents_focus(false),
            KeyCode::PageUp => app.agents_move(-5),
            KeyCode::PageDown => app.agents_move(5),
            KeyCode::Home => app.agents_move(isize::MIN / 2),
            KeyCode::Char('a') => app.agents_decide(true),
            KeyCode::Char('r') => app.agents_decide(false),
            KeyCode::Char('e') => app.begin_agent_edit(),
            KeyCode::Char('/') => {
                app.agents_filter_edit = Some(AgentFilterEdit::Query);
                app.agents_focus = AgentsFocus::Query;
            }
            KeyCode::Char('t') => {
                app.agents_filter_edit = Some(AgentFilterEdit::Tag);
                app.agents_focus = AgentsFocus::Tag;
            }
            KeyCode::Char('x') => app.clear_agents_filters(),
            _ => {}
        }
        return None;
    }
    if app.tools_open {
        match code {
            KeyCode::Esc => {
                app.tools_open = false;
                app.last_escape = None;
            }
            KeyCode::Char('t') if mods.contains(KeyModifiers::CONTROL) => app.tools_open = false,
            KeyCode::Up | KeyCode::Down => {
                app.tool_selected = if code == KeyCode::Up {
                    app.tool_selected.saturating_sub(1)
                } else {
                    (app.tool_selected + 1).min(app.tools.len().saturating_sub(1))
                };
                app.tool_scroll = (0, 0);
            }
            KeyCode::PageUp => app.tool_scroll.0 = app.tool_scroll.0.saturating_sub(10),
            KeyCode::PageDown => app.tool_scroll.0 = app.tool_scroll.0.saturating_add(10),
            KeyCode::Left => app.tool_scroll.1 = app.tool_scroll.1.saturating_sub(5),
            KeyCode::Right => app.tool_scroll.1 = app.tool_scroll.1.saturating_add(5),
            KeyCode::Home => app.tool_scroll = (0, 0),
            KeyCode::End => app.tool_scroll.0 = u16::MAX,
            KeyCode::Char('y') => {
                app.clipboard = app
                    .tools
                    .get(app.tool_selected)
                    .and_then(|tool| tool.output.clone());
                if app.clipboard.is_some() {
                    app.status = "tool output copied".into();
                }
            }
            _ => {}
        }
        return None;
    }
    if app.queue_open && app.queue_editing {
        handle_queue_editor_key(app, code, mods);
        return None;
    }
    if app.slash_query().is_some() {
        app.slash_feedback = None;
        let matches = app.slash_matches();
        app.slash_selected = app.slash_selected.min(matches.len().saturating_sub(1));
        match code {
            KeyCode::Esc => {
                app.slash_dismissed = Some(app.composer.text().to_string());
                app.last_escape = None;
                return None;
            }
            KeyCode::Up | KeyCode::Down => {
                app.slash_selected = if code == KeyCode::Up {
                    app.slash_selected.saturating_sub(1)
                } else {
                    (app.slash_selected + 1).min(matches.len().saturating_sub(1))
                };
                return None;
            }
            KeyCode::Tab => {
                if let Some((_, name, _)) = matches.get(app.slash_selected) {
                    app.composer.set_text(name);
                    app.slash_selected = 0;
                }
                return None;
            }
            KeyCode::Enter => {
                let Some((command, _, _)) = matches.get(app.slash_selected) else {
                    app.slash_feedback =
                        Some("Unknown command · /help for commands · Esc to send as text".into());
                    return None;
                };
                return execute_slash_command(app, *command);
            }
            _ => app.slash_selected = 0,
        }
    }
    if app.session_picker_open {
        if code == KeyCode::Esc {
            if app.session_searching {
                app.session_searching = false;
                app.session_query.clear();
                app.session_selected = 0;
            } else if app.session_picker_return_to_chat {
                app.session_picker_open = false;
                app.session_picker_return_to_chat = false;
                app.last_escape = None;
            } else {
                app.should_quit = true;
            }
            return None;
        }
        if code == KeyCode::Char('f') && mods.contains(KeyModifiers::CONTROL) {
            app.session_searching = !app.session_searching;
            app.session_selected = 0;
            return None;
        }
        if code == KeyCode::F(3) && !app.turn_in_flight {
            app.session_picker_open = false;
            app.session_picker_return_to_chat = true;
            app.session_list_refresh_requested = true;
            app.session_loading = true;
            app.session_query.clear();
            app.session_searching = false;
            return None;
        }
        if app.session_searching {
            match code {
                KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    app.session_query.push(c);
                    app.session_selected = 0;
                }
                KeyCode::Backspace => {
                    app.session_query.pop();
                    app.session_selected = 0;
                }
                _ => {}
            }
        }
        let matches = app.matching_sessions().cloned().collect::<Vec<_>>();
        let choices = matches.len() + 1;
        match code {
            KeyCode::Up | KeyCode::Char('k') if !app.session_searching => {
                app.session_selected = app.session_selected.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') if !app.session_searching => {
                app.session_selected = (app.session_selected + 1).min(choices - 1)
            }
            KeyCode::Home => app.session_selected = 0,
            KeyCode::End => app.session_selected = choices - 1,
            KeyCode::Char('n' | 'N') if !app.session_searching => {
                app.session_choice = Some(SessionChoice::New);
                app.session_picker_open = false;
                app.session_picker_return_to_chat = false;
                app.session_loading = true;
            }
            KeyCode::Enter => {
                app.session_choice = Some(if app.session_selected == matches.len() {
                    SessionChoice::New
                } else {
                    SessionChoice::Resume(matches[app.session_selected].id)
                });
                app.session_picker_open = false;
                app.session_picker_return_to_chat = false;
                app.session_loading = true;
            }
            _ => {}
        }
        app.session_selected = app.session_selected.min(choices - 1);
        return None;
    }
    // Con un diálogo de preguntas abierto (y sin permiso encima), Esc cancela la
    // pregunta en vez de contar para la salida por doble Esc.
    let question_has_keys = app.question.is_some() && app.permission.is_none();
    if code == KeyCode::Esc && !question_has_keys {
        // Con una selección de texto del transcript, Esc la limpia sin contar para
        // la doble Esc, igual que la del composer.
        if app.text_sel.is_some() && app.permission.is_none() {
            app.text_sel = None;
            app.last_escape = None;
            return None;
        }
        // Con una selección del composer, Esc solo la limpia y no cuenta para la
        // doble Esc que sale de la aplicación.
        if app.composer.selection().is_some() && app.permission.is_none() {
            app.composer.clear_selection();
            app.last_escape = None;
            return None;
        }
        if app.queue_open {
            app.queue_open = false;
            return None;
        }
        // Con foco en un panel lateral, Esc solo devuelve el foco al composer.
        // No cuenta para la doble Esc que sale de la aplicación.
        if app.focus.is_side_pane() && app.permission.is_none() {
            app.focus = Focus::Composer;
            app.last_escape = None;
            return None;
        }
        let now = Instant::now();
        if app
            .last_escape
            .is_some_and(|last| now.saturating_duration_since(last) <= ESC_DOUBLE_PRESS)
        {
            app.should_quit = true;
            app.last_escape = None;
        } else {
            app.last_escape = Some(now);
            app.cancel_requested = app.turn_in_flight;
            if app.cancel_requested {
                app.wake_pending = false;
            }
        }
        return None;
    }
    app.last_escape = None;
    if app.session_loading {
        return None;
    }
    if let Some(p) = app.permission.as_mut() {
        if let Some(d) = crate::permission_flow::permission_key(&mut p.dialog, code) {
            app.resolve_permission(d);
        }
        return None;
    }
    if app.question.is_some() {
        // Las teclas no reconocidas se consumen: no llegan al composer ni a los atajos.
        if let Some(key) = question_key_of(code) {
            app.question_key(key);
        }
        return None;
    }
    // `?` abre la ayuda solo con el compositor vacío y sin otro panel abierto; con texto es un
    // carácter más. Los diálogos de permiso y de pregunta ya han consumido la tecla arriba.
    if code == KeyCode::F(1) || (code == KeyCode::Char('?') && help_key_is_free(app, mods)) {
        app.help_open = true;
        return None;
    }
    if code == KeyCode::Char('t') && mods.contains(KeyModifiers::CONTROL) {
        app.tools_open = true;
        app.tool_selected = app.tools.len().saturating_sub(1);
        app.tool_scroll = (0, 0);
        return None;
    }
    if code == KeyCode::Char('g') && mods.contains(KeyModifiers::CONTROL) {
        app.cycle_composer_target();
        return None;
    }
    if code == KeyCode::F(2) {
        app.open_settings();
        return None;
    }
    if code == KeyCode::F(3) && !app.turn_in_flight {
        app.session_picker_open = false;
        app.session_picker_return_to_chat = true;
        app.session_list_refresh_requested = true;
        app.session_loading = true;
        app.session_query.clear();
        app.session_searching = false;
        return None;
    }
    if code == KeyCode::Char('k') && mods.contains(KeyModifiers::CONTROL) {
        app.queue_open = !app.queue_open;
        app.queue_selected = app
            .queue_selected
            .min(app.pending_prompts.len().saturating_sub(1));
        return None;
    }
    if app.queue_open {
        let len = app.pending_prompts.len();
        match (code, mods.contains(KeyModifiers::CONTROL)) {
            (KeyCode::Char(' '), false) => app.queue_paused = !app.queue_paused,
            (KeyCode::Char('x'), true) if app.turn_in_flight => {
                app.queue_paused = true;
                app.request_cancel();
            }
            (KeyCode::Enter, false) if len > 0 => {
                app.queue_editor
                    .set_text(&app.pending_prompts[app.queue_selected].text);
                app.queue_editing = true;
            }
            (KeyCode::Up, false) => app.queue_selected = app.queue_selected.saturating_sub(1),
            (KeyCode::Down, false) => {
                app.queue_selected = (app.queue_selected + 1).min(len.saturating_sub(1))
            }
            (KeyCode::Up, true) if app.queue_selected > 0 => {
                app.pending_prompts
                    .swap(app.queue_selected, app.queue_selected - 1);
                app.queue_selected -= 1;
            }
            (KeyCode::Down, true) if app.queue_selected + 1 < len => {
                app.pending_prompts
                    .swap(app.queue_selected, app.queue_selected + 1);
                app.queue_selected += 1;
            }
            (KeyCode::Delete, _) if len > 0 => {
                app.pending_prompts.remove(app.queue_selected);
                app.queue_selected = app.queue_selected.min(len.saturating_sub(2));
                app.queued = app.pending_prompts.len();
            }
            _ => {}
        }
        return None;
    }
    // Parada de emergencia: cancela el turno (si lo hay), los hijos y los jobs. Dentro del panel de la
    // cola, `Ctrl+X` conserva su significado de pausar la cola (rama de arriba).
    if code == KeyCode::Char('x') && mods.contains(KeyModifiers::CONTROL) {
        app.kill_all_requested = true;
        if app.turn_in_flight {
            app.request_cancel();
        }
        return None;
    }
    if sidebar_key(app, code) {
        return None;
    }
    if code == KeyCode::Tab {
        // Solo al pulsar Tab (nunca por pulsación de texto) y con la lista de agentes ya en memoria.
        let completado = crate::agent_command::complete_agent_name(
            app.composer.text(),
            &app.usable_agent_names(),
        );
        if let Some(texto) = completado {
            app.composer.set_text(&texto);
            return None;
        }
    }
    if matches!(code, KeyCode::Tab | KeyCode::BackTab) {
        app.toggle_focus();
        return None;
    }
    if code == KeyCode::Char('b') && mods.contains(KeyModifiers::CONTROL) {
        app.toggle_sidebar();
        return None;
    }
    if code == KeyCode::Char('o') && mods.contains(KeyModifiers::CONTROL) {
        let call_id = app.tools.back_mut().map(|tool| {
            tool.toggle_expanded();
            tool.call_id.clone()
        });
        if let Some(call_id) = call_id {
            app.sync_tool(&call_id);
        }
        return None;
    }
    // Una tecla imprimible con foco en un panel lateral devuelve el foco al
    // composer, y el carácter se escribe en él a continuación.
    if matches!(code, KeyCode::Char(_))
        && !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && app.focus.is_side_pane()
        && !(app.focus == Focus::Rail && code == KeyCode::Char('x'))
    {
        app.focus = Focus::Composer;
    }
    // Alt+← vuelve a la sesión padre con cualquier foco.
    if code == KeyCode::Left && mods.contains(KeyModifiers::ALT) {
        app.go_parent();
        return None;
    }
    // Con el foco en el rail y al menos un subagente, ↑/↓ mueven la selección y Enter
    // abre el subagente marcado. Sin subagentes, las teclas siguen en el composer.
    if app.focus == Focus::Rail && !app.subagents.is_empty() {
        match code {
            KeyCode::Char('x') => {
                if let Some(child) = app.rail_selected.and_then(|id| app.subagents.get(&id)) {
                    if !child.status.state.is_terminal() {
                        app.cancel_subagent_requested = Some(child.child);
                        app.status = format!("stopping subagent {}…", child.child.0);
                    }
                }
                return None;
            }
            KeyCode::Up => {
                app.rail_move(-1);
                return None;
            }
            KeyCode::Down => {
                app.rail_move(1);
                return None;
            }
            KeyCode::Enter => {
                if let Some(child) = app.rail_selected {
                    app.open_child(child);
                }
                return None;
            }
            _ => {}
        }
    }
    if matches!(
        code,
        KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
    ) {
        let down = matches!(code, KeyCode::PageDown | KeyCode::End);
        let amount = if matches!(code, KeyCode::Home | KeyCode::End) {
            u16::MAX
        } else {
            5
        };
        if scroll_dashboard_widget(app, app.focus, down, amount) {
            return None;
        }
    }
    if app.focus == Focus::Transcript {
        transcript_key(app, code, mods);
        return None;
    }
    if app.focus == Focus::Composer
        && matches!(code, KeyCode::Char('v' | 'V'))
        && mods.contains(KeyModifiers::CONTROL)
    {
        paste_from_clipboard(app);
        return None;
    }
    // Escribir, borrar o ampliar la selección en el composer anula la del transcript.
    if matches!(
        code,
        KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter
    ) || mods.contains(KeyModifiers::SHIFT)
    {
        app.text_sel = None;
    }
    match code {
        KeyCode::Enter => {
            // `/agent` y `@agente` no son mensajes: se analizan aquí, al enviar, y nunca por pulsación.
            // `/focus` fija el destinatario; con un hijo de destinatario el texto no va al padre.
            if app.try_agent_command() || app.try_focus_command() || app.submit_to_child() {
                return None;
            }
            let sent = app.submit();
            if sent.is_some() {
                app.viewport.follow_bottom();
            }
            sent
        }
        KeyCode::PageUp => {
            app.viewport.scroll_up(10);
            None
        }
        KeyCode::PageDown => {
            app.viewport.scroll_down(10);
            None
        }
        KeyCode::Char('j') if mods.contains(KeyModifiers::CONTROL) => {
            app.newline();
            None
        }
        KeyCode::Delete => {
            app.delete();
            None
        }
        // Shift+flechas y Shift+Inicio/Fin amplían la selección del composer.
        KeyCode::Left if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_left();
            None
        }
        KeyCode::Right if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_right();
            None
        }
        KeyCode::Up if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_up();
            None
        }
        KeyCode::Down if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_down();
            None
        }
        KeyCode::Home if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_home();
            None
        }
        KeyCode::End if mods.contains(KeyModifiers::SHIFT) => {
            app.composer.extend_end();
            None
        }
        KeyCode::Left => {
            app.move_left();
            None
        }
        KeyCode::Right => {
            app.move_right();
            None
        }
        KeyCode::Home => {
            app.move_home();
            None
        }
        KeyCode::End => {
            app.move_end();
            None
        }
        KeyCode::Up => {
            app.up();
            None
        }
        KeyCode::Down => {
            app.down();
            None
        }
        KeyCode::Backspace => {
            if app.composer.is_empty() && app.detach_last_image() {
                app.slash_feedback = Some("Removed last image attachment".into());
            } else {
                app.backspace();
            }
            None
        }
        KeyCode::Char(c) => {
            app.push_char(c);
            None
        }
        _ => None,
    }
}
