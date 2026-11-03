use super::*;

#[test]
fn esc_clears_text_selection_without_double_esc() {
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    drag_select(&mut app, (x, y), (x + 3, y));
    assert!(app.text_sel.is_some());
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.text_sel.is_none());
    assert_eq!(app.last_escape, None);
    assert!(!app.should_quit);
    // La Esc que limpió no cuenta: hacen falta dos más para salir.
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_quit);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.should_quit);
}

#[test]
fn ctrl_c_with_text_selection_copies_and_does_not_quit() {
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    drag_select(&mut app, (x, y), (x + 3, y));
    app.clipboard = None;
    handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!app.should_quit);
    assert_eq!(app.clipboard.as_deref(), Some("hola"));
}

#[test]
fn width_change_clears_text_selection() {
    let mut app = transcript_app(&["hola mundo"]);
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "hola");
    drag_select(&mut app, (x, y), (x + 3, y));
    assert!(app.text_sel.is_some());
    paint_sized(&mut app, 60, 30);
    assert!(app.text_sel.is_none());
}

#[test]
fn text_selection_excludes_line_selection() {
    let mut app = transcript_app(&["uno", "dos", "tres"]);
    app.focus = Focus::Transcript;
    let buf = paint_sized(&mut app, 100, 30);
    handle_key(&mut app, KeyCode::Char('v'), KeyModifiers::NONE);
    assert!(app.selection.is_some());
    let (x, y) = screen_pos_of(&buf, "dos");
    drag_select(&mut app, (x, y), (x + 1, y));
    assert!(
        app.selection.is_none(),
        "la selección de texto anula la de líneas"
    );
    assert!(app.text_sel.is_some());
    handle_key(&mut app, KeyCode::Char('v'), KeyModifiers::NONE);
    assert!(app.text_sel.is_none(), "la de líneas anula la de texto");
    // Y la del composer, también excluyente.
    app.composer.set_text("hola");
    app.composer.select_range(0, 2);
    drag_select(&mut app, (x, y), (x + 1, y));
    assert_eq!(app.composer.selection(), None);
}

#[test]
fn dialog_open_ignores_transcript_mouse() {
    let mut app = transcript_app(&["hola mundo"]);
    app.focus = Focus::Composer;
    let buf = paint_sized(&mut app, 100, 30);
    let _reply = open_dialog(&mut app);
    let (x, y) = screen_pos_of(&buf, "hola");
    drag_select(&mut app, (x, y), (x + 3, y));
    assert_eq!(app.focus, Focus::Composer);
    assert!(app.text_sel.is_none());
    assert!(!app.text_drag);
    assert!(app.clipboard.is_none());
}

#[test]
fn drag_on_empty_transcript_does_not_panic() {
    let mut app = App::new();
    paint_sized(&mut app, 100, 30);
    drag_select(&mut app, (10, 10), (20, 5));
    assert!(app.text_sel.is_none());
    // Un terminal de una sola fila de transcript tampoco debe romper.
    let mut app = transcript_app(&["uno", "dos", "tres"]);
    paint_sized(&mut app, 100, 6);
    drag_select(&mut app, (10, 3), (12, 0));
    mouse_event_drag_left(&mut app, 5, u16::MAX);
}

#[test]
fn shift_arrows_extend_the_selection() {
    let mut app = App::new();
    app.composer.set_text("abc");
    handle_key(&mut app, KeyCode::Left, KeyModifiers::SHIFT);
    handle_key(&mut app, KeyCode::Left, KeyModifiers::SHIFT);
    assert_eq!(app.composer.selected_text(), Some("bc"));
    handle_key(&mut app, KeyCode::Home, KeyModifiers::SHIFT);
    assert_eq!(app.composer.selected_text(), Some("abc"));

    let mut app = App::new();
    app.composer.set_text("abc");
    app.composer.set_cursor(0);
    handle_key(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    assert_eq!(app.composer.selected_text(), Some("abc"));
    handle_key(&mut app, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(
        app.composer.selection(),
        None,
        "sin Shift, el movimiento limpia"
    );
}

#[test]
fn ctrl_c_with_selection_copies_and_does_not_quit() {
    let mut app = App::new();
    app.composer.set_text("abc");
    app.composer.select_range(0, 2);
    handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!app.should_quit);
    assert_eq!(app.clipboard.as_deref(), Some("ab"));
}

#[test]
fn ctrl_c_without_selection_keeps_its_behavior() {
    let mut app = App::new();
    app.composer.set_text("abc");
    handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(app.should_quit);
    assert!(app.clipboard.is_none());
}

#[test]
fn esc_with_selection_clears_it_and_does_not_count_for_double_esc() {
    let mut app = App::new();
    app.composer.set_text("abc");
    app.composer.select_range(0, 2);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.composer.selection(), None);
    assert_eq!(app.last_escape, None);
    assert!(!app.should_quit);
    // La primera Esc sin selección cuenta, pero no basta para salir.
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_quit);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.should_quit);
}

#[test]
fn mouse_is_ignored_while_a_dialog_is_open() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    app.composer.set_cursor(2);
    app.focus = Focus::Transcript;
    paint(&mut app);
    let _reply = open_dialog(&mut app);
    let (x, y) = composer_cell(&app, 6);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), x + 3, y);
    mouse_event(&mut app, MouseEventKind::Up(MouseButton::Left), x + 3, y);
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(app.composer.cursor(), 2);
    assert_eq!(app.composer.selection(), None);
    assert!(app.clipboard.is_none());
}

#[test]
fn typing_after_selecting_replaces_the_selected_text() {
    let mut app = App::new();
    app.composer.set_text("hello world");
    app.composer.select_range(0, 5);
    handle_key(&mut app, KeyCode::Char('X'), KeyModifiers::NONE);
    assert_eq!(app.composer.text(), "X world");
    assert_eq!(app.composer.selection(), None);
}

#[test]
fn typing_during_a_drag_cancels_it() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = App::new();
    app.composer.set_text("hello world");
    paint(&mut app);
    let (x0, y) = composer_cell(&app, 0);
    let (x5, _) = composer_cell(&app, 5);
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x0, y);
    handle_event(
        &mut app,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )),
    );
    mouse_event(&mut app, MouseEventKind::Drag(MouseButton::Left), x5, y);
    assert_eq!(app.composer.selection(), None);
}

#[test]
fn ctrl_c_quits() {
    let mut app = App::new();
    assert_eq!(
        handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL),
        None
    );
    assert!(app.should_quit);
}
