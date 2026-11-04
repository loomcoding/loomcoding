use super::*;

pub(super) fn open_dialog(
    app: &mut App,
) -> tokio::sync::oneshot::Receiver<loomcoding_core::permission::Decision> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.enqueue_permission(
        loomcoding_core::permission::PermissionRequest {
            tool: "bash".into(),
            action: "run".into(),
            detail: r#"{"command":"ls"}"#.into(),
        },
        tx,
    );
    rx
}

/// Pinta la aplicación una vez en un terminal de prueba para que registre
/// sus ventanas (`pane_rects`) y el área del composer.
pub(super) fn paint(app: &mut App) {
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
}

pub(super) fn mouse_event(app: &mut App, kind: crossterm::event::MouseEventKind, x: u16, y: u16) {
    use crossterm::event::MouseEvent;
    handle_event(
        app,
        Event::Mouse(MouseEvent::new(kind, x, y, KeyModifiers::NONE)),
    );
}

/// Celda (x, y) de la primera línea del composer Classic, en la columna `col`
/// del texto (el prefijo ocupa dos celdas tras el borde).
pub(super) fn composer_cell(app: &App, col: u16) -> (u16, u16) {
    let area = app
        .composer_area
        .expect("el composer se registra al pintar");
    (area.x + 1 + 2 + col, area.y + 1)
}

/// Pinta a `w`×`h` y devuelve el búfer resultante, para localizar texto en pantalla.
pub(super) fn paint_sized(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
    term.draw(|f| render(f, app)).unwrap();
    term.backend().buffer().clone()
}

/// Celda (x, y) donde aparece por primera vez `needle` en la pantalla pintada.
pub(super) fn screen_pos_of(buf: &ratatui::buffer::Buffer, needle: &str) -> (u16, u16) {
    let area = buf.area;
    for y in area.y..area.bottom() {
        let row: String = (area.x..area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect();
        if let Some(byte) = row.find(needle) {
            return (area.x + row[..byte].chars().count() as u16, y);
        }
    }
    panic!("«{needle}» no aparece en pantalla");
}

/// Aplicación con un mensaje de usuario por cada texto.
pub(super) fn transcript_app(texts: &[&str]) -> App {
    let mut app = App::new();
    for text in texts {
        app.push_user(text);
    }
    app
}

/// Arrastra con el botón izquierdo desde `from` hasta `to` y suelta.
pub(super) fn drag_select(app: &mut App, from: (u16, u16), to: (u16, u16)) {
    use crossterm::event::{MouseButton, MouseEventKind};
    mouse_event(app, MouseEventKind::Down(MouseButton::Left), from.0, from.1);
    mouse_event(app, MouseEventKind::Drag(MouseButton::Left), to.0, to.1);
    mouse_event(app, MouseEventKind::Up(MouseButton::Left), to.0, to.1);
}

/// Arrastre del botón izquierdo hasta (x, y).
pub(super) fn mouse_event_drag_left(app: &mut App, x: u16, y: u16) {
    use crossterm::event::{MouseButton, MouseEventKind};
    mouse_event(app, MouseEventKind::Drag(MouseButton::Left), x, y);
}
