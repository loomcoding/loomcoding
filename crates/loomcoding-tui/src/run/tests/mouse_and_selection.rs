use super::*;

#[test]
fn pasted_newlines_stay_together_until_enter_submits() {
    let mut app = App::new();
    let sent = handle_event(&mut app, Event::Paste("uno\ndos\ntres".into()));
    assert_eq!(sent, None);
    assert_eq!(app.composer.text(), "uno\ndos\ntres");
    assert_eq!(
        handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE),
        Some("uno\ndos\ntres".into())
    );
}

#[test]
fn mouse_wheel_scrolls_transcript_but_not_overlays() {
    use crossterm::event::{MouseEvent, MouseEventKind};
    let mut app = App::new();
    app.viewport.set_geometry(20, 5);
    handle_event(
        &mut app,
        Event::Mouse(MouseEvent::new(
            MouseEventKind::ScrollUp,
            0,
            0,
            KeyModifiers::NONE,
        )),
    );
    assert!(!app.viewport.is_following());

    app.queue_open = true;
    app.viewport.follow_bottom();
    handle_event(
        &mut app,
        Event::Mouse(MouseEvent::new(
            MouseEventKind::ScrollUp,
            0,
            0,
            KeyModifiers::NONE,
        )),
    );
    assert!(app.viewport.is_following());
}

#[test]
fn mouse_wheel_over_composer_moves_from_middle_to_its_last_line() {
    use crossterm::event::{MouseEvent, MouseEventKind};
    let mut app = App::new();
    app.set_theme(loomcoding_cloth::widgets::ThemePreset::Crush);
    let text = "x".repeat(1000);
    app.composer.set_text(&text);
    app.composer.set_cursor(500);
    app.composer_area = Some(Rect::new(10, 10, 40, 12));
    app.viewport.set_geometry(100, 10);
    let transcript_top = app.viewport.top();
    let before = app.composer.cursor();

    for _ in 0..12 {
        handle_event(
            &mut app,
            Event::Mouse(MouseEvent::new(
                MouseEventKind::ScrollDown,
                20,
                15,
                KeyModifiers::NONE,
            )),
        );
    }

    let after = app.composer.cursor();
    assert!(
        after > 950,
        "el cursor terminó en {after}, partió en {before}"
    );
    assert_eq!(app.viewport.top(), transcript_top);
    assert_eq!(app.focus, Focus::Composer);
}

#[test]
fn a_burst_of_keys_is_applied_in_order() {
    use crossterm::event::KeyEvent;
    let mut app = App::new();
    for c in "x".repeat(2000).chars() {
        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)),
        );
    }
    assert_eq!(app.composer.text().len(), 2000);
}

#[test]
fn click_in_the_composer_moves_the_cursor_and_focuses_it() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    app.focus = Focus::Transcript;
    paint(&mut app);
    let (x, y) = composer_cell(&app, 4);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    assert_eq!(app.focus, Focus::Composer);
    assert_eq!(app.composer.cursor(), 4);
    assert_eq!(app.composer.selection(), None);
}

#[test]
fn click_in_another_pane_changes_focus_and_does_not_select() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    app.composer.set_cursor(3);
    paint(&mut app);
    let transcript = app
        .pane_rects
        .iter()
        .find(|(pane, _)| *pane == Focus::Transcript)
        .map(|(_, area)| *area)
        .expect("el transcript se registra");
    mouse_event(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        transcript.x + 2,
        transcript.y + 2,
    );
    mouse_event(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        transcript.x + 4,
        transcript.y + 2,
    );
    mouse_event(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        transcript.x + 4,
        transcript.y + 2,
    );
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(app.composer.cursor(), 3);
    assert_eq!(app.composer.selection(), None);
    assert!(app.clipboard.is_none());
}

#[test]
fn drag_in_the_composer_selects_text() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    paint(&mut app);
    let (x0, y) = composer_cell(&app, 0);
    let (x5, _) = composer_cell(&app, 5);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x0, y);
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), x5, y);
    assert_eq!(app.composer.selection(), Some((0, 5)));
    assert_eq!(app.composer.selected_text(), Some("hello"));
}

#[test]
fn drag_outside_the_composer_is_clamped_to_it() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    paint(&mut app);
    let (x6, y) = composer_cell(&app, 6);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x6, y);
    // Hacia arriba y a la izquierda: se acota al inicio del texto.
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), 0, 0);
    assert_eq!(app.composer.selected_text(), Some("hello "));
    // Hacia abajo y a la derecha, muy lejos: se acota al final del texto.
    mouse_event(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        u16::MAX,
        u16::MAX,
    );
    assert_eq!(app.composer.selected_text(), Some("world"));
}

#[test]
fn release_copies_the_selection_to_the_clipboard() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    paint(&mut app);
    let (x0, y) = composer_cell(&app, 0);
    let (x5, _) = composer_cell(&app, 5);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x0, y);
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), x5, y);
    assert!(app.clipboard.is_none(), "copiar solo al soltar");
    mouse_event(&mut app, MouseEventKind::Up(MouseButton::Left), x5, y);
    assert_eq!(app.clipboard.as_deref(), Some("hello"));
    assert_eq!(app.slash_feedback.as_deref(), Some("Copied 5 chars"));
}

#[test]
fn click_without_drag_copies_nothing() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    paint(&mut app);
    let (x, y) = composer_cell(&app, 3);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    mouse_event(&mut app, MouseEventKind::Up(MouseButton::Left), x, y);
    assert!(app.clipboard.is_none());
    assert_eq!(app.slash_feedback, None);
}

#[test]
fn click_in_transcript_focuses_and_starts_selection() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = transcript_app(&["hola mundo"]);
    app.focus = Focus::Composer;
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    assert_eq!(app.focus, Focus::Transcript);
    let sel = app.text_sel.expect("el clic empieza una selección");
    assert!(sel.is_empty());
    assert!(app.text_drag);
}

#[test]
fn drag_selects_text_and_mouse_up_copies_with_notice() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), x + 3, y);
    assert!(app.clipboard.is_none(), "copiar solo al soltar");
    mouse_event(&mut app, MouseEventKind::Up(MouseButton::Left), x + 3, y);
    assert_eq!(app.clipboard.as_deref(), Some("hola"));
    assert_eq!(app.slash_feedback.as_deref(), Some("Copied 4 chars"));
    assert!(app.text_sel.is_some(), "la selección sigue resaltada");
}

#[test]
fn dragging_backwards_selects_the_same_text() {
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (hx, y) = screen_pos_of(&buf, "hola");
    let (mx, _) = screen_pos_of(&buf, "mundo");
    // Del final hacia el principio: el rango se normaliza.
    drag_select(&mut app, (mx + 4, y), (hx, y));
    assert_eq!(app.clipboard.as_deref(), Some("hola mundo"));
}

#[test]
fn simple_click_clears_selection_and_copies_nothing() {
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    drag_select(&mut app, (x, y), (x + 3, y));
    assert!(app.text_sel.is_some());
    app.clipboard = None;
    // Un clic simple, sin arrastre, descarta la selección anterior.
    drag_select(&mut app, (x, y), (x, y));
    assert!(app.text_sel.is_none());
    assert!(app.clipboard.is_none());
}

#[test]
fn drag_above_area_autoscrolls_and_clamps() {
    let texts: Vec<String> = (0..60).map(|i| format!("msg-{i:02}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut app = transcript_app(&refs);
    paint_sized(&mut app, 100, 30);
    assert!(app.viewport.is_following());
    let area = app.transcript_area.expect("el transcript fija su área");
    let (x, y) = (area.x, area.y + 2);
    let top = app.viewport.top();
    mouse_event_down_left(&mut app, x, y);
    for _ in 0..3 {
        mouse_event_drag_left(&mut app, x, 0);
    }
    assert_eq!(app.viewport.top(), top - 3, "una fila por evento");
    assert!(!app.viewport.is_following());
    let (start, _) = app.text_sel.expect("hay selección").ordered();
    assert_eq!(start.row, app.viewport.top(), "la cabeza se acota al área");
    // Por debajo del área también desplaza, sin salirse del transcript.
    let before = app.viewport.top();
    mouse_event_drag_left(&mut app, x, u16::MAX);
    assert_eq!(app.viewport.top(), before + 1);
}

/// Botón izquierdo pulsado en (x, y), para los tests que arrastran por pasos.
fn mouse_event_down_left(app: &mut App, x: u16, y: u16) {
    use crossterm::event::{MouseButton, MouseEventKind};
    mouse_event(app, MouseEventKind::Down(MouseButton::Left), x, y);
}
