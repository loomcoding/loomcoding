//! Tests: states.

use super::support::*;
use super::*;

#[test]
fn rail_marks_subagents_waiting_for_input() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.subagents.get_mut(&9).unwrap().status = TaskStatus::new(TaskState::InputRequired);

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
        review.contains("? review"),
        "falta el glifo de espera: {review:?}"
    );
    assert!(!tests.contains('?'), "glifo de espera de más: {tests:?}");
}

#[test]
fn rail_title_shows_hierarchy_totals() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    let now = Some(Instant::now());
    app.agent_stats
        .insert(9, stats(1, 0, None, 1_200, 300, now));
    app.agent_stats.insert(10, stats(1, 0, None, 800, 100, now));

    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter()
            .any(|row| row.contains("SUBAGENTS · 2 · 2k/400")),
        "el título no muestra los totales: {rows:?}"
    );
}

#[test]
fn rail_names_the_a2a_states_under_the_running_count() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));

    let rows = rows_of(&app, 40, 24);
    // Solo los estados que hay, con su cuenta exacta.
    assert!(
        rows.iter().any(|row| row.contains("(working 1)")),
        "falta el desglose por estado bajo el recuento: {rows:?}"
    );
    assert!(
        !rows.iter().any(|row| row.contains("auth-required")),
        "se nombra un estado que no hay: {rows:?}"
    );
}

#[test]
fn rail_shows_input_required_glyph_for_waiting_running_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(9, running(9, "review"));
    app.subagents.insert(10, running(10, "tests"));
    app.subagents.get_mut(&9).unwrap().status = TaskStatus::new(TaskState::InputRequired);

    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("? review")),
        "falta el glifo input-required del hijo en espera: {rows:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("● tests")),
        "el hijo sin espera debe pintarse como working: {rows:?}"
    );
}

#[test]
fn rail_shows_failed_glyph_for_a_failed_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(
        9,
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            status: TaskStatus::new(TaskState::Failed),
            ..running(9, "review")
        },
    );

    let rows = rows_of(&app, 40, 24);
    assert!(
        rows.iter().any(|row| row.contains("✗ review")),
        "un hijo sin terminar debe verse como failed: {rows:?}"
    );
}

#[test]
fn rail_shows_completed_glyph_for_finished_child() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(
        9,
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            status: TaskStatus::new(TaskState::Completed),
            ..running(9, "review")
        },
    );

    let rows = rows_of(&app, 40, 24);
    let fila = rows
        .iter()
        .find(|row| row.contains("review"))
        .expect("el rail lista review");
    assert!(
        fila.contains("✓ review"),
        "un hijo terminado debe verse como completed: {fila:?}"
    );
    assert!(
        !fila.contains('?'),
        "espera residual en un hijo terminado: {fila:?}"
    );
}

#[test]
fn agent_state_covers_all_eight_states() {
    // Submitted, AuthRequired y Rejected no tienen productor hoy: se construyen a mano.
    let pares = [
        (TaskState::Submitted, AgentState::Submitted),
        (TaskState::Working, AgentState::Working),
        (TaskState::InputRequired, AgentState::InputRequired),
        (TaskState::AuthRequired, AgentState::AuthRequired),
        (TaskState::Completed, AgentState::Completed),
        (TaskState::Failed, AgentState::Failed),
        (TaskState::Canceled, AgentState::Canceled),
        (TaskState::Rejected, AgentState::Rejected),
    ];
    for (tarea, esperado) in pares {
        assert_eq!(agent_state(tarea), esperado, "proyección de {tarea:?}");
    }
}

#[test]
fn rail_derives_input_and_auth_from_the_task_state() {
    let mut app = App::new();
    app.session_id = Some(SessionId(1));
    app.subagents.insert(
        9,
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            status: TaskStatus::new(TaskState::InputRequired),
            ..running(9, "review")
        },
    );
    app.subagents.insert(
        10,
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            status: TaskStatus::new(TaskState::AuthRequired),
            ..running(10, "tests")
        },
    );

    let rows = rows_of(&app, 40, 24);
    let glifo_espera = agent_state(TaskState::InputRequired).glyph();
    let glifo_permiso = agent_state(TaskState::AuthRequired).glyph();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("{glifo_espera} review"))),
        "el glifo de input-required sale del estado de la tarea: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("{glifo_permiso} tests"))),
        "el glifo de auth-required sale del estado de la tarea: {rows:?}"
    );
}
