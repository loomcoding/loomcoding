//! Tests: selection.

use super::support::*;
use super::*;

#[test]
fn rail_shows_the_agent_of_each_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    let mut con_agente = running(9, "review");
    con_agente.agent = Some("front".into());
    app.subagents.insert(9, con_agente);
    app.subagents.insert(10, running(10, "tests"));

    let rows = rows_of(&app, 40, 20);
    let review = rows
        .iter()
        .find(|row| row.contains("review"))
        .expect("el rail lista el subagente review");
    let tests = rows
        .iter()
        .find(|row| row.contains("tests"))
        .expect("el rail lista el subagente tests");
    assert!(
        review.contains("[front] review"),
        "falta la etiqueta del agente: {review:?}"
    );
    assert!(!tests.contains('['), "etiqueta de más: {tests:?}");

    // Con el foco en el rail, el hijo elegido lleva `›` en lugar del glifo, con la misma etiqueta.
    app.focus = Focus::Rail;
    app.rail_selected = Some(9);
    let rows = rows_of(&app, 40, 20);
    let review = rows
        .iter()
        .find(|row| row.contains("review"))
        .expect("el rail lista el subagente review seleccionado");
    assert!(
        review.contains("› [front] review"),
        "falta la etiqueta con `›`: {review:?}"
    );
}

#[test]
fn selected_subagent_shows_marker_only_when_rail_focused() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.rail_selected = Some(9);

    app.focus = crate::app::Focus::Composer;
    let sin_foco = rows_of(&app, 40, 24);
    let fila_sin_foco = sin_foco
        .iter()
        .find(|row| row.contains("review"))
        .expect("el rail lista review");
    assert!(
        !fila_sin_foco.contains('›'),
        "marcador de selección sin foco en el rail: {fila_sin_foco:?}"
    );
    assert!(fila_sin_foco.contains("● review"));

    app.focus = crate::app::Focus::Rail;
    let con_foco = rows_of(&app, 40, 24);
    let fila_review = con_foco
        .iter()
        .find(|row| row.contains("review"))
        .expect("el rail lista review");
    let fila_tests = con_foco
        .iter()
        .find(|row| row.contains("tests"))
        .expect("el rail lista tests");
    assert!(
        fila_review.contains("› review"),
        "falta el marcador con foco en el rail: {fila_review:?}"
    );
    assert!(
        fila_tests.contains("● tests") && !fila_tests.contains('›'),
        "marcador en un subagente no seleccionado: {fila_tests:?}"
    );
    assert_eq!(
        columna(fila_review, "review"),
        columna(fila_sin_foco, "review"),
        "el marcador cambia la anchura de la línea"
    );
}

#[test]
fn rail_selected_child_keeps_arrow_marker() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.rail_selected = Some(9);
    app.subagents.get_mut(&9).unwrap().status = TaskStatus::new(TaskState::InputRequired);
    app.focus = crate::app::Focus::Rail;

    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("› review")),
        "el hijo elegido con foco en el rail debe llevar ›: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains("? review")),
        "el › sustituye al glifo del hijo elegido: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("● tests")),
        "el hijo no elegido conserva su glifo: {rows:?}"
    );
}

#[test]
fn selected_child_marker_is_tinted_when_waiting_for_input() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.focus = crate::app::Focus::Rail;
    app.rail_selected = Some(9);
    app.subagents.get_mut(&9).unwrap().status = TaskStatus::new(TaskState::InputRequired);
    let marca = |app: &App, etiqueta: &str| {
        rail_lines(app, 40)
            .into_iter()
            .find(|line| line.spans.iter().any(|s| s.content == etiqueta))
            .and_then(|line| line.spans.first().map(|s| s.style))
            .expect("la línea del hijo existe")
    };
    assert_eq!(marca(&app, "review"), app.theme.styles.warning);

    // Sin espera, el marcador vuelve al acento.
    app.subagents.get_mut(&9).unwrap().status = TaskStatus::new(TaskState::Working);
    assert_eq!(marca(&app, "review"), app.theme.styles.accent);
}
