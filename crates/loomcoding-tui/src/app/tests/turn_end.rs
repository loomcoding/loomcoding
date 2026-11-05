//! Registro del final del último turno (`last_turn_end`), que usa el muñeco de estado.

use super::*;

#[test]
fn turn_ended_records_a_successful_end() {
    let mut app = App::new();
    app.apply(LiveEvent::TurnEnded);
    assert!(matches!(app.last_turn_end, Some((_, true))));
}

#[test]
fn an_error_records_a_failed_end() {
    let mut app = App::new();
    app.apply(LiveEvent::Error {
        message: "provider unavailable".into(),
    });
    assert!(matches!(app.last_turn_end, Some((_, false))));
}

#[test]
fn starting_a_turn_clears_the_last_end() {
    let mut app = App::new();
    app.apply(LiveEvent::TurnEnded);
    app.apply(LiveEvent::TurnStarted);
    assert!(app.last_turn_end.is_none());
}
