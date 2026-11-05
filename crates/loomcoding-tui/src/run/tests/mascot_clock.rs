//! El reloj de 50 ms sigue mientras el muñeco tenga postura: `Done` y `Fail` deben desaparecer
//! solos al vencer, aunque no haya turno en curso.

use super::*;
use std::time::Duration;

#[test]
fn the_clock_runs_while_done_is_shown_and_stops_after_it() {
    let mut app = App::new();
    let ended = Instant::now();
    app.last_turn_end = Some((ended, true));
    assert!(wants_clock(&app, false, ended + Duration::from_secs(2)));
    assert!(!wants_clock(&app, false, ended + Duration::from_secs(4)));
}

#[test]
fn the_clock_runs_while_fail_is_shown_and_stops_after_it() {
    let mut app = App::new();
    let ended = Instant::now();
    app.last_turn_end = Some((ended, false));
    assert!(wants_clock(&app, false, ended + Duration::from_secs(9)));
    assert!(!wants_clock(&app, false, ended + Duration::from_secs(11)));
}

#[test]
fn an_idle_app_does_not_run_the_clock() {
    let app = App::new();
    assert!(!wants_clock(&app, false, Instant::now()));
}
