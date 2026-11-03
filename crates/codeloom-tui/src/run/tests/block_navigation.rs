use super::*;

/// Aplicación con `n` bloques (mensajes de usuario), pintada y con el foco en
/// el transcript.
fn block_app(n: usize) -> App {
    let texts: Vec<String> = (0..n).map(|i| format!("bloque-{i:02}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut app = transcript_app(&refs);
    paint_sized(&mut app, 100, 30);
    app.focus = Focus::Transcript;
    app
}

/// Aplicación con look Crush (chrome `Flat`), un mensaje de usuario por texto y
/// pintada. El foco queda en el composer.
fn crush_app(texts: &[&str]) -> App {
    use batten::widgets::{Theme, ThemePreset};
    let mut app = transcript_app(texts);
    app.theme_preset = ThemePreset::Crush;
    app.theme = Theme::preset(ThemePreset::Crush);
    paint_sized(&mut app, 100, 30);
    app
}

/// Como `crush_app`, con `n` bloques de texto para poder desplazarse.
fn crush_app_n(n: usize) -> App {
    let texts: Vec<String> = (0..n).map(|i| format!("bloque-{i:02}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    crush_app(&refs)
}

#[test]
fn bracket_keys_move_the_active_block() {
    let mut app = block_app(3);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    assert_eq!(app.block, Some(2), "sin bloque activo, [ activa el último");
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    assert_eq!(app.block, Some(1));
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    assert_eq!(app.block, Some(0), "acotado al primero");
    handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    assert_eq!(app.block, Some(1));
    handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    assert_eq!(app.block, Some(2), "acotado al último");
}

#[test]
fn block_nav_is_clamped_and_noop_on_empty_transcript() {
    let mut app = App::new();
    app.focus = Focus::Transcript;
    paint_sized(&mut app, 100, 30);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
    assert_eq!(app.block, None);
    assert!(app.clipboard.is_none());
}

#[test]
fn block_nav_scrolls_block_into_view() {
    let mut app = block_app(60);
    // La primera pulsación activa el último; 54 más llevan al bloque 5.
    for _ in 0..55 {
        handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    }
    assert_eq!(app.block, Some(5));
    assert!(
        !app.viewport.is_following(),
        "navegar hacia atrás suelta el final"
    );
    let start = app.layout.row_of(app.transcript.entry_start(5));
    assert!(app.viewport.window().contains(&start), "el bloque 5 se ve");
    // Hacia el final, el último bloque vuelve a verse.
    for _ in 0..60 {
        handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    }
    assert_eq!(app.block, Some(59));
    let last = app.layout.row_of(app.transcript.entry_start(59));
    assert!(app.viewport.window().contains(&last), "el último se ve");
}

#[test]
fn copy_block_copies_entry_text_with_notice() {
    let mut app = block_app(2);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
    let clip = app.clipboard.clone().expect("y copia el bloque activo");
    assert!(clip.contains("bloque-01"));
    assert!(!clip.contains("bloque-00"));
    let chars = clip.chars().count();
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some(format!("Copied {chars} chars").as_str())
    );
    app.clipboard = None;
    handle_key(&mut app, KeyCode::Char('c'), KeyModifiers::NONE);
    assert!(app.clipboard.is_some(), "c copia igual que y");
}

#[test]
fn click_on_transcript_activates_the_clicked_block() {
    use crossterm::event::{MouseButton, MouseEventKind};
    let mut app = block_app(3);
    app.focus = Focus::Composer;
    let buf = paint_sized(&mut app, 100, 30);
    let (x, y) = screen_pos_of(&buf, "bloque-01");
    mouse_event(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(app.block, Some(1));
    assert!(app.text_sel.is_some(), "el clic sigue creando la selección");
}

#[test]
fn j_k_still_scroll_by_lines() {
    let mut app = block_app(60);
    let before = app.viewport.top();
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.viewport.top(), before - 1, "k sube una fila");
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.viewport.top(), before, "j baja una fila");
    assert_eq!(app.block, None, "j/k no mueven el bloque activo");
}

#[test]
fn arrows_and_jk_move_blocks_in_crush() {
    let mut app = crush_app(&["uno", "dos", "tres"]);
    app.focus = Focus::Transcript;
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(
        app.block,
        Some(2),
        "sin bloque activo, la flecha activa el último"
    );
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.block, Some(1));
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.block, Some(0));
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(app.block, Some(0), "acotado al primero");
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.block, Some(1));
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.block, Some(2));
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(app.block, Some(2), "acotado al último");
}

#[test]
fn ctrl_arrows_and_ctrl_j_scroll_one_line_in_crush() {
    let mut app = crush_app_n(60);
    app.focus = Focus::Transcript;
    let before = app.viewport.top();
    handle_key(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
    assert_eq!(
        app.viewport.top(),
        before - 1,
        "Ctrl+flecha arriba sube una fila"
    );
    handle_key(&mut app, KeyCode::Down, KeyModifiers::CONTROL);
    assert_eq!(
        app.viewport.top(),
        before,
        "Ctrl+flecha abajo baja una fila"
    );
    // Al volver al final el seguimiento se activa: subimos dos para tener margen.
    handle_key(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::CONTROL);
    assert_eq!(app.viewport.top(), before - 2);
    handle_key(&mut app, KeyCode::Char('j'), KeyModifiers::CONTROL);
    assert_eq!(app.viewport.top(), before - 1, "Ctrl+j baja una fila");
    assert_eq!(
        app.block, None,
        "el desplazamiento no mueve el bloque activo"
    );
}

#[test]
fn line_selection_mode_jk_still_extends_lines() {
    let mut app = crush_app_n(10);
    app.focus = Focus::Transcript;
    handle_key(&mut app, KeyCode::Char('v'), KeyModifiers::NONE);
    let lines = |app: &App| app.selection.map(|s| s.range().count());
    assert_eq!(lines(&app), Some(1));
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(
        lines(&app),
        Some(2),
        "con selección activa, k extiende por líneas"
    );
    handle_key(&mut app, KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(lines(&app), Some(3));
    assert_eq!(app.block, None, "la selección de líneas no mueve bloques");
}

#[test]
fn tab_into_transcript_selects_last_block_in_crush() {
    let mut app = crush_app(&["uno", "dos", "tres"]);
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(
        app.block,
        Some(2),
        "al entrar, el último bloque queda marcado"
    );
}

#[test]
fn tab_into_transcript_keeps_classic_without_block() {
    let mut app = transcript_app(&["uno", "dos", "tres"]);
    paint_sized(&mut app, 100, 30);
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(app.block, None, "Classic no marca bloque al entrar");
}

#[test]
fn focus_with_empty_transcript_does_not_panic() {
    let mut app = crush_app(&[]);
    app.focus = Focus::Composer;
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.focus, Focus::Transcript);
    assert_eq!(app.block, None);
    handle_key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char(']'), KeyModifiers::NONE);
    assert_eq!(app.block, None);
}

#[test]
fn block_cleared_on_session_load() {
    let mut app = block_app(3);
    handle_key(&mut app, KeyCode::Char('['), KeyModifiers::NONE);
    assert!(app.block.is_some());
    app.load_session(codeloom_core::session::SessionId(9), Vec::new());
    assert_eq!(app.block, None);
}

#[test]
fn space_folds_the_active_long_block_and_still_pages_otherwise() {
    let mut app = App::new();
    app.push_user(
        &(1..=25)
            .map(|i| format!("l{i}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app.focus = Focus::Transcript;
    app.block = Some(0);

    handle_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(app.transcript.entry_lines(0).len(), 25, "espacio despliega");
    handle_key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        app.transcript.entry_lines(0).len(),
        11,
        "Enter vuelve a plegar"
    );

    // Sin nada que plegar, el espacio sigue paginando (no toca el transcript).
    app.block = None;
    handle_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(app.transcript.entry_lines(0).len(), 11);
}
